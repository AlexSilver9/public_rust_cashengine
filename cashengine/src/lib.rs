mod rest_client;
mod symbol;
mod time_util;
mod websocket;
//pub mod shm_interlaced_queue;
pub mod shm_block_writer;
pub mod shm_reader;

mod string_u8_util;
mod util;
mod metrics;
mod compression;

use crate::metrics::P95Tracker;
use crate::shm_block_writer::SharedMemoryWriter;
use crate::shm_reader::SharedMemoryReader;
use crate::time_util::print_systemtime;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{event, Level};

static STATUS: &[u8] = b"status";
static MARKET_DOT: &[u8] = b"market.";
const MARKETS_PER_WEBSOCKET: usize = 150;

pub fn run() {

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    print_systemtime();

    let websocket_url = "wss://api-aws.huobi.pro/ws";

    // TODO: On Linux use tmpfs shared memory: let mut synchronizer = Synchronizer::new("/dev/shm/hello_world".as_ref());
    let mmap_file_path = "/tmp/ticks.mmap";
    let log_file_path = "/tmp/rust_cashengine.log"; // TODO: Make this configurable

    let symbols_file_path = "/tmp/symbols.json"; // TODO: Make this configurable
    let request_url = format!(
        "https://api-aws.huobi.pro{path}",
        path = "/v1/settings/common/symbols"
    );
    let body = rest_client::send_request(&request_url).expect("Failed to get symbols");
    let mut symbols = symbol::Symbols::from(&body).expect("Failed to parse symbols");
    symbols = symbols
        .with_online_symbols()
        .with_trade_enabled_symbols()
        .with_cancel_enabled_symbols()
        .with_visible_symbols()
        .with_listed_symbols()
        .with_country_enabled_symbols();

    if let Err(err) = symbols.get_error() {
        panic!("Requested symbols contained an error. Exchange error: {err}")
    } else {
        symbols.print_compact();
    }
    let symbols = Arc::new(symbols);

    let websocket_count = (symbols.len() / MARKETS_PER_WEBSOCKET) + 1;

    let shm_file = create_shm_file(mmap_file_path);
    resize_shm_file(&shm_file, websocket::CHUNK_SIZE * websocket_count * MARKETS_PER_WEBSOCKET);

    let shm_file = Arc::new(shm_file);

    std::thread::scope(|s| {
        println!("Starting Feed Threads for {} Ids", websocket_count);
        for id in 0..websocket_count {
            let symbols = Arc::clone(&symbols);
            let shm_file = Arc::clone(&shm_file);
            s.spawn(move || {
                println!("Starting Feed Thread for Id {}", id);
                let mut shm_writer = SharedMemoryWriter::create(
                    &&shm_file,
                    log_file_path,
                    id,
                    websocket::CHUNK_SIZE,
                    MARKETS_PER_WEBSOCKET,
                );

                let mut subscribe_request = String::new();
                subscribe_request.push_str("{\"sub\": [");
                let symbols_start_index = id * MARKETS_PER_WEBSOCKET; // TODO: Make 150 configurable
                let mut symbols_length = (id * MARKETS_PER_WEBSOCKET) + MARKETS_PER_WEBSOCKET; // TODO: Debug that this doesnt overlap with the other threads
                if symbols_length > symbols.len() {
                    symbols_length = symbols.len();
                }

                let symbols_to_subscribe = &symbols.get_symbols()[symbols_start_index..symbols_length];
                let mut indexed_symbols: HashMap<&str, usize> = HashMap::new();
                symbols_to_subscribe.iter().enumerate().for_each(|(index, symbol)| {
                    if let Some(symbol_name) = &symbol.symbol {
                        subscribe_request.push_str(format!("\"market.{}.bbo\",", symbol_name).as_str());
                        indexed_symbols.insert(symbol.symbol.as_ref().unwrap(), index);
                    } else {
                        panic!("Missing symbol name for: {:?}", symbol);
                    }
                });

                if subscribe_request.ends_with(',') {
                    subscribe_request.pop(); // Remove the last comma
                }
                subscribe_request.push_str("\n],\n\"id\": \"id");
                subscribe_request.push_str(id.to_string().as_str());
                subscribe_request.push_str("\"\n}");

                let mut websocket = websocket::CeWebSocket::connect(websocket_url)
                    .expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());
                let (_, response) = tungstenite::connect(websocket_url)
                    .expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());
                println!("Connected to the server");
                println!("Response HTTP code: {}", response.status());
                println!("Response contains the following headers:");
                for (header, _value) in response.headers() {
                    println!("* {header}");
                }

                let on_websocket_message = |message: &[u8]| {
                    if let Some(_) = message.windows(STATUS.len()).position(|window| window == STATUS) {
                        return;
                    }

                    if let Some(market_index_start) = message.windows(MARKET_DOT.len()).position(|window| window == MARKET_DOT) {
                        let market_index_start = market_index_start + "market.".len();
                        if let Some(market_index_end) = message[market_index_start..].windows(4).position(|window| window == b".bbo") {
                            let market_index_str = std::str::from_utf8(&message[market_index_start..market_index_start + market_index_end])
                                .expect("Invalid UTF-8 sequence");
                            if let Some(index) = indexed_symbols.get(market_index_str) {
                                shm_writer.write(*index, message);
                            } else {
                                panic!("Failed to lookup index for market {} from websocket {}, message: {}",
                                       market_index_str, id, std::str::from_utf8(message).unwrap_or("Invalid UTF-8"));
                            }
                        } else {
                            panic!("Failed to parse market from websocket {}, message: {}",
                                   id, std::str::from_utf8(message).unwrap_or("Invalid UTF-8"));
                        }
                    } else {
                        panic!("Failed to parse market from websocket {}, message: {}",
                               id, std::str::from_utf8(message).unwrap_or("Invalid UTF-8"));
                    }
                };

                println!("Subscribing to symbols: {}", subscribe_request);
                websocket.subscribe(subscribe_request.as_str());
                websocket.run(on_websocket_message);

                shm_writer.close();
            });
        }

        let main_thread = s.spawn(move || {
            println!("Starting Feeds Reader Thread");
            let shm_file = Arc::clone(&shm_file);
            let mut shm_reader = SharedMemoryReader::create(
                &&shm_file,
                websocket::CHUNK_SIZE,
                websocket_count * MARKETS_PER_WEBSOCKET,
            );

            let mut p95_tracker = P95Tracker::new(128);

            loop {
                let message = shm_reader.read_next_message();
                let message =
                    unsafe { string_u8_util::null_terminated_u8_to_utf8_str_unchecked(message) };
                if !message.is_empty() {
                    //println!("Read message: '{}'", message);
                    match message.find(':') {
                        Some(index) => {
                            let message = &message[index+1..];
                            match message.find(':') {
                                Some(start_index) => {
                                    let message = &message[start_index+1..];
                                    match message.find(':') {
                                        Some(end_index) => {
                                            let timestamp = &message[..end_index];
                                            let timestamp: u128 = timestamp.parse().unwrap_or_else(|e| {
                                                eprintln!("Failed to parse timestamp: {}, error: {}", timestamp, e);
                                                0
                                            });

                                            let current_system_time = SystemTime::now();
                                            match current_system_time.duration_since(UNIX_EPOCH) {
                                                Ok(duration_since_epoch) => {
                                                    let micro_seconds_timestamp = duration_since_epoch.as_micros();
                                                    let latency = micro_seconds_timestamp - timestamp;

                                                    p95_tracker.push(latency);

                                                    if p95_tracker.has_enough_samples() {
                                                        if let Some(p95) = p95_tracker.p95() {
                                                            event!(Level::INFO, "P95 Latency: {} μs", p95);
                                                        }
                                                    }
                                                },
                                                Err(e) => println!("Failed getting duration for UNIX epoch: {}", e),
                                            }
                                        }
                                        _ => ()
                                    }

                                },
                                _ => ()
                            }
                        }
                        _ => ()
                    }
                }
            }
        });
        main_thread.join().unwrap();
        //close(&shm_file);
    });
}

fn create_shm_file(file_path: &str) -> File {
    println!("Creating SHM file: {}", file_path);
    let path_buf = PathBuf::from(file_path);
    let open_result = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path_buf);
    let file: File = match open_result {
        Ok(file) => file,
        Err(e) => {
            panic!("Failed to create SHM file: {}", e);
        }
    };
    file
}

fn resize_shm_file(file: &File, file_size: usize) {
    println!("Resizing SHM file to {} bytes", file_size);
    match file.set_len(file_size as u64) {
        Ok(_) => (),
        Err(e) => {
            panic!("Failed to resize SHM file: {}", e);
        }
    }
}

/*pub fn close(mut file: &Arc<File>) {
    // Make writes visible for main thread
    // It is not necessary when using `std::thread::scope` but may be necessary in your case.
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
    let result = file.flush();
    match result {
        Ok(()) => println!("Flushed SHM file"),
        Err(e) => println!(
            "Failed to flush SHM file, error: {}", e),
    }
}*/