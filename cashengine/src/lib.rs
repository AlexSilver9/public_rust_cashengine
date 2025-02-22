mod time_util;
mod rest_client;
mod symbol;
mod websocket;
pub mod mmap_queue;

use std::{str};
use std::io::{Read};
use std::net::TcpStream;
use tungstenite::{Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use crate::mmap_queue::SharedMemoryQueue;
use crate::time_util::print_systemtime;

const MMAP_FILE_SIZE: usize = 1024/*b*/ * 1024/*kb*/; // * 1024/*mb*/; // * 10/*gb*/; // TODO: Make this configurable

pub fn run() {
    print_systemtime();

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
    symbols.print_compact();

    if let Err(err) = symbols.get_error() {
        panic!("Requested symbols contained an error. Exchange error: {err}")
    }

    let websocket_url = "wss://api-aws.huobi.pro/ws";

    let mut websocket = websocket::CeWebSocket::connect(websocket_url)
        .expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());

    // WebSocket message handling

    let (socket, response) = tungstenite::connect(
        websocket_url
    ).expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");

    for (header, _value) in response.headers() {
        println!("* {header}");
    }

    let mut subscribe_request = String::new();
    subscribe_request.push_str("{\"sub\": [");
    symbols.get_symbols()[0..150].iter().for_each(|symbol| {
        if let Some(symbol_name) = &symbol.symbol {
            subscribe_request.push_str(format!("\"market.{}.bbo\",", symbol_name).as_str());
        }
    });
    if subscribe_request.ends_with(',') {
        subscribe_request.pop(); // Remove the last comma
    }
    subscribe_request.push_str("\n],\n\"id\": \"id1\"\n}");

    println!("Subscribing to symbols: {}", subscribe_request);
    websocket.subscribe(subscribe_request.as_str());

    let websocket_count = 1;

    let mmap_file_path = "/tmp/tick.mmap";
    let log_file_path = "/tmp/rust_cashengine.log"; // TODO: Make this configurable
    let mut shared_memory_queue = SharedMemoryQueue::create(
        mmap_file_path,
        MMAP_FILE_SIZE,
        log_file_path,
        websocket_count,
        websocket::CHUNK_SIZE
    );

    std::thread::scope(|s| {
        println!("Starting IPC Writer Threads for {} Ids", websocket_count);
        for id in 0..websocket_count {
            s.spawn(move || {
                println!("Starting IPC Writer Thread for Id {}", id);
                shared_memory_queue.write(id);
            });
        }
        let main_thread = s.spawn(move || {
            println!("Starting IPC Reader Thread for {} Ids", websocket_count);
            loop {
                let message = shared_memory_queue.read_next_mesage();
            }
        });
        main_thread.join().unwrap();
        shared_memory_queue.close();
    });

    std::thread::scope(|s| {
        s.spawn(move || {
            websocket.run(shareable_ptr, websocket::CHUNK_SIZE, websocket_count, 0)
        });

        s.spawn(move || {
            loop {
                /*let shared_message = unsafe {
                    synchronizer.read::<SharedMessage>(false)
                };
                match shared_message {
                    Ok(shared_message) => {
                        let msg = String::from_utf8(shared_message.message.to_owned()).expect("Invalid UTF-8 in shared message");
                        println!("Received shared message: {}", msg);
                        let write_result = log_file.write_all(msg.as_bytes());
                        match write_result {
                            Ok(_) => {
                                log_file.write_all(b"\n").expect("Failed to write newline to log file");
                                log_file.flush().expect("Failed to flush log file");
                            },
                            Err(e) => println!("Failed to write to log file: {}", e),
                        }
                    }
                    Err(e) => {
                        println!("Failed to read from mmap file: {}", file_path);
                        break;
                    }
                }
                 */

                /*let shared_tick_result = unsafe {
                    synchronizer.read::<SharedTick>(false)
                };
                match shared_tick_result {
                    Ok(tick) => {
                        println!("Received shared tick message: {:?}", *tick);
                    }
                    Err(e) => {
                        println!("Failed to read from mmap file: {}", file_path);
                        break;
                    }
                }*/
            }
        });
        std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
        //mmap_queue.flush().expect("Failed to flush");
    });
}
