mod time_util;
mod rest_client;
mod symbol;
mod websocket;
//pub mod shm_interlaced_queue;
pub mod shm_block_writer;
pub mod shm_reader;

mod string_u8_util;
mod util;

use std::collections::HashMap;
use std::fs::File;
use crate::shm_block_writer::SharedMemoryWriter;
use crate::shm_reader::SharedMemoryReader;
use crate::time_util::print_systemtime;
use std::io::Read;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;


pub fn run() {
    print_systemtime();

    let throttle: bool = false; // TODO: Make this configurable



    let websocket_url = "wss://api-aws.huobi.pro/ws";
    let websocket_count = 1;


    // TODO: On Linux use tmpfs shared memory: let mut synchronizer = Synchronizer::new("/dev/shm/hello_world".as_ref());
    let mmap_file_path = "/tmp/ticks.mmap";
    let log_file_path = "/tmp/rust_cashengine.log"; // TODO: Make this configurable

    let symbols_file_path = "/tmp/symbols.json"; // TODO: Make this configurable
    let request_url = format!("https://api-aws.huobi.pro{path}", path = "/v1/settings/common/symbols");
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


    let shm_file = create_shm_file(mmap_file_path);
    resize_shm_file(&shm_file, websocket::CHUNK_SIZE * websocket_count);

    std::thread::scope(|s| {
        println!("Starting Feed Threads for {} Ids", websocket_count);
        for id in 0..websocket_count {
            let symbols = Arc::clone(&symbols);
            s.spawn(move || {
                println!("Starting Feed Thread for Id {}", id);
                let mut shm_writer = SharedMemoryWriter::create(
                    mmap_file_path,
                    log_file_path,
                    id,
                    websocket::CHUNK_SIZE,
                    websocket_count,
                );

                let mut subscribe_request = String::new();
                subscribe_request.push_str("{\"sub\": [");
                let symbols_start_index = id * 150; // TODO: Make 150 configurable
                let symbols_length = (id * 150) + 150; // TODO: Debug that this doesnt overlap with the other threads

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

                let mut websocket = websocket::CeWebSocket::connect(id, websocket_url)
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
                    let market: Vec<u8> = "market.".as_bytes().to_vec();
                    if let Some(market_index_start) = message.windows(market.len()).position(|window| window == market) {
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
                };;

                println!("Subscribing to symbols: {}", subscribe_request);
                websocket.subscribe(subscribe_request.as_str());
                websocket.run(on_websocket_message);

                shm_writer.close();
            });
        }

        let main_thread = s.spawn(move || {
            println!("Starting Feeds Reader Thread");
            let mut shm_reader = SharedMemoryReader::create(
                mmap_file_path,
                log_file_path,
                websocket::CHUNK_SIZE,
                websocket_count,
            );
            loop {
                if throttle {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }

                let message = shm_reader.read_next_message();
                let message = unsafe { string_u8_util::null_terminated_u8_to_utf8_str_unchecked(message) };
                if ! message.is_empty() {
                    println!("Received message: '{}'", message);
                }
            }
            shm_reader.close()
        });
        main_thread.join().unwrap();
    });
}

fn create_shm_file(file_path: &str) -> File {
    println!("Creating IPC file: {}", file_path);
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
            panic!("Failed to create IPC file: {}", e);
        }
    };
    file
}


fn resize_shm_file(file: &File, file_size: usize) {
    println!("Resizing IPC file to {} bytes", file_size);
    match file.set_len(file_size as u64) {
        Ok(_) => (),
        Err(e) => {
            panic!("Failed to resize IPC file: {}", e);
        }
    }
}