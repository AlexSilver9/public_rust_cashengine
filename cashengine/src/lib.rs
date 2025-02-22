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

    let mmap_file_path = "/tmp/tick.mmap";
    let log_file_path = "/tmp/rust_cashengine.log"; // TODO: Make this configurable

    let writer_threads = 2;

    let (shareable_ptr, mmap, mmap_file) = mmap_queue::initialize(mmap_file_path, MMAP_FILE_SIZE);
    let mut log_file = mmap_queue::create_log_file(log_file_path);

    std::thread::scope(|s| {
        println!("Starting IPC Writer Threads for {} Ids", writer_threads);
        for id in 0..writer_threads {
            s.spawn(move || {
                mmap_queue::write(id, writer_threads, websocket::CHUNK_SIZE, MMAP_FILE_SIZE, &shareable_ptr);
            });
        }
        let main_thread = s.spawn(move || {
            println!("Starting IPC Reader Thread for {} Ids", writer_threads);
            mmap_queue::read(writer_threads, websocket::CHUNK_SIZE, MMAP_FILE_SIZE, &shareable_ptr, &mut log_file);
        });
        main_thread.join().unwrap();
        mmap_queue::close(mmap, mmap_file);
    });

    std::thread::scope(|s| {
        s.spawn(move || {
            websocket.run(shareable_ptr, websocket::CHUNK_SIZE, writer_threads, 0)
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

fn send_pong(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, s: &str) {
    let mut pong = String::with_capacity(s.len());
    pong.push_str(&s[..3]);
    pong.push('o');
    pong.push_str(&s[4..]);
    send_message(socket, pong.as_str());
}

fn send_message(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, s: &str) {
    let sent = String::from(s);
    let msg = Message::text(s);
    match socket.send(msg) {
        Ok(()) => {
            println!("Sent {}", sent);
        },
        Err(e) => {
            println!("Error sending message: {}", e);
        }
    }
}

