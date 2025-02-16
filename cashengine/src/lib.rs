mod time_util;
mod rest_client;
mod symbol;

use std::io;
use std::io::Read;
use std::net::TcpStream;
use std::ops::Deref;
use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use crate::time_util::print_systemtime;

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


    // WebSocket message handling
    let (mut socket, response) = connect(
        "wss://api-aws.huobi.pro/ws"
    ).expect("Failed to connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");

    for (header, _value) in response.headers() {
        println!("* {header}");
    }

    let mut subscribe_request = String::new();
    subscribe_request.push_str("{\"sub\": [");
    symbols.get_symbols().iter().for_each(|symbol| {
        if let Some(symbol_name) = &symbol.symbol {
            subscribe_request.push_str(format!("\"market.{}.bbo\",", symbol_name).as_str());
            // TODO: Omit the last comma
        }
    });
    subscribe_request.push_str("\n],\n\"id\": \"id1\"\n}");

    println!("Subscribing to symbols: {}", subscribe_request);

//    let subscribe_request = r#"{
//  "sub": [
//    "market.btcusdt.bbo",
//    "market.ethusdt.bbo",
//    "market.htxusdt.bbo"
//  ],
//  "id": "id1"
//}"#;
    send_message(&mut socket, subscribe_request.to_string());

    loop {
        let msg = socket.read().expect("Error reading message");
        let vec = msg.into_data().to_vec();
        match decode_reader(vec) {
            Ok(decoded_message) => {
                if decoded_message.starts_with("{\"ping") {
                    println!("Received message: {}", decoded_message);
                    send_pong(&mut socket, decoded_message);
                } else {
                    println!("Received message: {}", decoded_message);
                }
            },
            Err(e) => println!("Failed to parse message: {:?}", e),
        }
    }
}

fn send_pong(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, mut s: String) {
    s.replace_range(3..4, "o");
    send_message(socket, s);
}

fn send_message(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, mut s: String) {
    let sent = String::from(s.as_str());
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

fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
    let mut gz = MultiGzDecoder::new(&bytes[..]);
    let mut s = String::new();
    gz. read_to_string(&mut s)?;
    Ok(s)
}

