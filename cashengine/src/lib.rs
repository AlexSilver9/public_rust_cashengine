mod rest_client;
mod htx_symbol;
mod htx_currency;
mod htx_market;
mod time_util;
mod websocket;
pub mod shm_block_writer;
pub mod shm_reader;

mod string_u8_util;
mod util;
mod metrics;
mod compression;
mod ring;
mod pair;
mod precision;
mod limits;
mod currency;

use crate::metrics::P95Tracker;
use crate::shm_block_writer::SharedMemoryWriter;
use crate::shm_reader::SharedMemoryReader;
use crate::time_util::print_systemtime;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

static STATUS: &[u8] = b"status";
static MARKET_DOT: &[u8] = b"market.";
const MARKETS_PER_WEBSOCKET: usize = 150;

pub fn run() {

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    print_systemtime();

    let websocket_url = "wss://api-aws.huobi.pro/ws";

    // TODO: On Linux use tmpfs shared memory: /dev/shm/ticks.shm;
    let mmap_file_path = "/tmp/ticks.mmap";

    let rest_url = "https://api-aws.huobi.pro";

    let symbols_url = format!("{rest_url}{path}",path = htx_symbol::PATH);
    let body = rest_client::send_request(&symbols_url).expect("Failed to get symbols");
    let mut symbols = htx_symbol::HtxSymbols::from(&body).expect("Failed to parse symbols");
    symbols = symbols
        .with_online_symbols()
        .with_trade_enabled_symbols()
        .with_cancel_enabled_symbols()
        .with_visible_symbols()
        .with_listed_symbols()
        .with_country_enabled();
    if let Err(err) = symbols.get_error() {
        panic!("Requested symbols contained an error. Exchange error: {err}")
    } else if symbols.len() == 0 {
        panic!("Requested symbols are empty");
    } else {
        symbols.log_compact();
    }


    let currencies_url = format!("{rest_url}{path}", path = htx_currency::PATH);
    let body = rest_client::send_request(&currencies_url).expect("Failed to get currencies");
    let mut currencies = htx_currency::HtxCurrencies::from(&body).expect("Failed to parse currencies");
    currencies = currencies
        .with_online_currencies()
        .with_country_enabled();
    if let Err(err) = currencies.get_error() {
        panic!("Requested currencies contained an error. Exchange error: {err}")
    } else if currencies.len() == 0 {
        panic!("Requested currencies are empty");
    } else {
        //currencies.print_compact();
    }

    let markets_url = format!("{rest_url}{path}", path = htx_market::PATH);
    let body = rest_client::send_request(&markets_url).expect("Failed to get markets");
    let mut markets = htx_market::HtxMarkets::from(&body).expect("Failed to parse markets");
    markets = markets
        .with_online_markets();
    if let Err(err) = markets.get_error() {
        panic!("Requested markets contained an error. Exchange error: {err}")
    } else if markets.len() == 0 {
        panic!("Requested markets are empty");
    } else {
        //markets.print_compact();
    }


    let symbols = Arc::new(symbols);


    let websocket_count = (symbols.len() / MARKETS_PER_WEBSOCKET) + 1;

    let shm_file = create_shm_file(mmap_file_path);
    resize_shm_file(&shm_file, websocket::CHUNK_SIZE * websocket_count * MARKETS_PER_WEBSOCKET);

    let shm_file = Arc::new(shm_file);

    std::thread::scope(|s| {
        tracing::info!("Starting Feed Threads for {} Ids", websocket_count);
        for id in 0..websocket_count {
            let symbols = Arc::clone(&symbols);
            let shm_file = Arc::clone(&shm_file);
            s.spawn(move || {
                tracing::info!("Starting Feed Thread for Id {}", id);
                let mut shm_writer = SharedMemoryWriter::create(
                    &&shm_file,
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
                tracing::debug!("Connected to websocket server with id {}", id);
                tracing::debug!("Websocket server id {} response HTTP code: {}", id, response.status());
                tracing::debug!("Websocket server id {} response contains the following headers:", id);
                for (header, _value) in response.headers() {
                    tracing::debug!("* {header}");
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

                tracing::info!("Subscribing to symbols: {}", subscribe_request);
                websocket.subscribe(subscribe_request.as_str());
                websocket.run(on_websocket_message);

                shm_writer.close();
            });
        }

        let main_thread = s.spawn(move || {
            tracing::info!("Starting Feeds Reader Thread");
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
                    tracing::trace!("Read message: '{}'", message);
                    match message.find(':') {
                        Some(index) => {
                            let writer_id = &message[..index];
                            let message = &message[index+1..];
                            match message.find(':') {
                                Some(start_index) => {
                                    let sequence = &message[..start_index];
                                    let message = &message[start_index+1..];
                                    match message.find(':') {
                                        Some(end_index) => {
                                            let start_timestamp_micros = &message[..end_index];
                                            let start_timestamp_micros: u128 = start_timestamp_micros.parse().unwrap_or_else(|e| {
                                                tracing::error!("Failed to parse start_timestamp_micros: {}, error: {}", start_timestamp_micros, e);
                                                0
                                            });

                                            let message = &message[end_index+1..];

                                            match message.find(':') {
                                                Some(end_index) => {
                                                    let offset = &message[..end_index];
                                                    let message = &message[end_index+1..];

                                                    // TODO: Process message with business logic here

                                                    let current_system_time = SystemTime::now();
                                                    match current_system_time.duration_since(UNIX_EPOCH) {
                                                        Ok(duration_since_epoch) => {
                                                            let end_timestamp_micros = duration_since_epoch.as_micros();
                                                            let latency = end_timestamp_micros - start_timestamp_micros;

                                                            p95_tracker.push(latency);

                                                            if p95_tracker.has_enough_samples() {
                                                                if let Some(p95) = p95_tracker.p95() {
                                                                    tracing::debug!("P95 Latency: {} μs", p95);
                                                                    tracing::trace!("Read message from writer_id: {}, sequence: {}, start_timestamp_micros: {}, offset: {}, message: {}",
                                                                        writer_id, sequence, start_timestamp_micros, offset, message);
                                                                }
                                                            }
                                                        },
                                                        Err(e) => tracing::error!("Failed getting duration for UNIX epoch: {}", e),
                                                    }
                                                },
                                                _ => ()
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
    });
}

fn create_shm_file(file_path: &str) -> File {
    tracing::info!("Creating SHM file: {}", file_path);
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
    tracing::info!("Resizing SHM file to {} bytes", file_size);
    match file.set_len(file_size as u64) {
        Ok(_) => (),
        Err(e) => {
            panic!("Failed to resize SHM file: {}", e);
        }
    }
}