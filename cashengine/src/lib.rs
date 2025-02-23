mod time_util;
mod rest_client;
mod symbol;
mod websocket;
pub mod mmap_queue;

use crate::mmap_queue::SharedMemoryQueue;
use crate::time_util::print_systemtime;
use std::io::Read;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

const MMAP_FILE_SIZE: usize = 1024/*b*/ * 1024/*kb*/; // * 1024/*mb*/; // * 10/*gb*/; // TODO: Make this configurable

pub fn run() {
    print_systemtime();

    let websocket_url = "wss://api-aws.huobi.pro/ws";
    let websocket_count = 1;

    let mmap_file_path = "/tmp/tick.mmap";
    let log_file_path = "/tmp/rust_cashengine.log"; // TODO: Make this configurable
    let mut shared_memory_queue = Arc::new(Mutex::new(SharedMemoryQueue::create(
        mmap_file_path,
        MMAP_FILE_SIZE,
        log_file_path,
        websocket_count,
        websocket::CHUNK_SIZE
    )));

    std::thread::scope(|s| {
        println!("Starting IPC Writer Threads for {} Ids", websocket_count);
        for id in 0..websocket_count {
            let mut shared_memory_queue = Arc::clone(&shared_memory_queue);
            s.spawn(move || {
                println!("Starting IPC Writer Thread for Id {}", id);

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

                let mut websocket = websocket::CeWebSocket::connect(1, websocket_url)
                    .expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());
                let (_, response) = tungstenite::connect(websocket_url)
                    .expect(format!("Failed to connect websocket url: {}", websocket_url).as_str());
                println!("Connected to the server");
                println!("Response HTTP code: {}", response.status());
                println!("Response contains the following headers:");
                for (header, _value) in response.headers() {
                    println!("* {header}");
                }
                println!("Subscribing to symbols: {}", subscribe_request);
                websocket.subscribe(subscribe_request.as_str());
                websocket.run(&mut shared_memory_queue);

            });
        }

        let main_thread_queue = Arc::clone(&shared_memory_queue);
        let main_thread = s.spawn(move || {
            println!("Starting IPC Reader Thread for {} Ids", websocket_count);
            for id in 0..websocket_count {
                let queue = main_thread_queue.lock().unwrap();
                let message = queue.get_read_buffer(id);
                println!("Received message: {}", message);
            }
        });
        main_thread.join().unwrap();
        let mut queue = shared_memory_queue.lock().unwrap();
        queue.close();
    });
}
