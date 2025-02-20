mod time_util;
mod rest_client;
mod symbol;
mod websocket;

use std::{io, str};
use std::io::Read;
use std::net::TcpStream;
use flate2::read::MultiGzDecoder;
use mmap_sync::synchronizer::Synchronizer;
use tungstenite::{Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use crate::time_util::print_systemtime;
use crate::websocket::SharedMessage;

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

    let (mut socket, response) = tungstenite::connect(
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

    let file_path = "/tmp/tick.mmap";
    websocket.run(file_path);

    let mut synchronizer = Synchronizer::new(file_path.as_ref());
    loop {
        let shared_message = unsafe {
            synchronizer.read::<SharedMessage>(false)
        };
        match shared_message {
            Ok(shared_message) => {
                println!("Received shares message: {:?}", shared_message.message);
            }
            Err(e) => {
                println!("Failed to read from mmap file: {}", file_path);
                break;
            }
        }
    }
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

fn gz_inflate_to_string(bytes: &Vec<u8>) -> io::Result<String> {
    let mut gz = MultiGzDecoder::new(&bytes[..]);
    let mut s = String::new();
    gz.read_to_string(&mut s)?;
    Ok(s)
}

fn gz_inflate_to_buffer(bytes: &Vec<u8>, buffer: &mut [u8]) -> io::Result<usize> {
    let mut gz = MultiGzDecoder::new(&bytes[..]);
    gz.read(buffer)
}
