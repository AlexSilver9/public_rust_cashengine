use std::io;
use std::io::Read;
use std::net::TcpStream;
use flate2::read::{GzDecoder, MultiGzDecoder};
use serde::{Deserialize, Serialize};
use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;
use std::time::{ SystemTime, SystemTimeError, UNIX_EPOCH };

fn main() {
    let current_system_time = SystemTime::now();
    let duration_since_epoch = current_system_time.duration_since(UNIX_EPOCH).expect("Failed to get duration for UNIX epoch");
    let milliseconds_timestamp = duration_since_epoch.as_millis();
    println!("Startup timestamp: {}", milliseconds_timestamp);

    // Rest connection
    let request_url = format!("https://api-aws.huobi.pro{path}", path = "/v1/settings/common/symbols");

    let mut response = reqwest::blocking::get(request_url).expect("Failed to send request");
    let mut body = String::new();
    response.read_to_string(&mut body);

    println!("Status: {}", response.status());
    println!("Headers:\n{:#?}", response.headers());
    //println!("Body:\n{}", body);

    let parse_result: serde_json::Result<Symbols> = serde_json::from_str(body.as_str());
    match parse_result {
        Ok(symbols) => {
            //println!("{:?}", symbols);
            symbols.data.iter().for_each(|symbol| {
                print!("Symbol: {:?}", symbol);
            });
            println!("\n");
            println!("Symbols received: {}", symbols.data.len());
        }
        Err(e) => {
            println!("Error parsing symbols: {}, {}", e, body);
            panic!("Failed to parse symbols");
        }
    }

    // Websocket connection
    let (mut socket, response) = connect(
        "wss://api-aws.huobi.pro/ws"
    ).expect("Failed to connect");

    println!("Connected to the server");
    println!("Response HTTP code: {}", response.status());
    println!("Response contains the following headers:");

    for (header, _value) in response.headers() {
        println!("* {header}");
    }

    let subscribe_request = r#"{
  "sub": [
    "market.btcusdt.bbo",
    "market.ethusdt.bbo",
    "market.htxusdt.bbo"
  ],
  "id": "id1"
}"#;
    send_message(&mut socket, subscribe_request.to_string());

    loop {
        let msg = socket.read().expect("Error reading message");
        let vec = msg.into_data().to_vec();
        let mut result = decode_reader(vec);
        let mut s = result.expect("Failed to parse message");

        if s.starts_with("{\"ping") {
            println!("Received message: {}", s);
            send_pong(&mut socket, s);
        } else {
            println!("Received message: {}", s);
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
        Ok(x) => {
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

#[derive(Serialize, Deserialize, Debug)]
struct Symbol {
    // field, type, required, description
    symbol: Option<String>, // false	symbol(outside)
    sn: Option<String>,     // false	symbol name
    bc: Option<String>,     // false	base currency
    qc: Option<String>,     // false	quote currency
    state: Option<String>,	// false	symbol status. unknown，not-online，pre-online，online，suspend，offline，transfer-board，fuse
    ve: Option<bool>,       // false	visible
    we:	Option<bool>,       // false	white enabled
    dl:	Option<bool>,	    // false	delist
    cd:	Option<bool>,	    // false	country disabled
    te:	Option<bool>,       // false	trade enabled
    ce:	Option<bool>,       // false	cancel enabled
    tet: Option<u64>,       // false	trade enable timestamp
    toa: Option<u64>,       // false	the time trade open at
    tca: Option<u64>,       // false	the time trade close at
    voa: Option<u64>,       // false	visible open at
    vca: Option<u64>,       // false	visible close at
    sp: Option<String>,     // false	symbol partition
    tm: Option<String>,     // false	symbol partition
    w:	Option<u64>,        // false	weight sort
    ttp: Option<f64>,       // false	trade total precision -> decimal(10,6)
    tap: Option<f64>,       // false	trade amount precision -> decimal(10,6)
    tpp: Option<f64>,       // false	trade price precision -> decimal(10,6)
    fp: Option<f64>,        // false	fee precision -> decimal(10,6)
    tags: Option<String>,   // false	Tags, multiple tags are separated by commas, such as: st, hadax
    d: Option<String>,		// false
    bcdn: Option<String>,	// false	base currency display name
    qcdn: Option<String>,	// false	quote currency display name
    elr: Option<String>,	// false	etp leverage ratio
    castate: Option<String>,// false	Not required. The state of the call auction; it will only be displayed when it is in the 1st and 2nd stage of the call auction. Enumeration values: "ca_1", "ca_2"
    ca1oa: Option<u64>,     // false	not Required. the open time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    ca1ca: Option<u64>,     // false	not Required. the close time of call auction phase 1, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    ca2oa: Option<u64>,     // false	not Required. the open time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
    ca2ca: Option<u64>,     // false	not Required. the close time of call auction phase 2, total milliseconds since January 1, 1970 0:0:0:00ms UTC
}

#[derive(Serialize, Deserialize, Debug)]
struct Symbols {
    status: String,             // false    status
    data: Vec<Symbol>,          // false    data
    ts: String,                 // false    timestamp of incremental data
    full: i8,                   // false    full data flag: 0 for no and 1 for yes
    err_code: Option<String>,   // false	error code(returned when the interface reports an error)  -> err-code -> TODO: parse this manually because it is no underscore
    err_msg: Option<String>,    // false	error msg(returned when the interface reports an error)  -> err-code -> TODO: parse this manually because it is no underscore
}