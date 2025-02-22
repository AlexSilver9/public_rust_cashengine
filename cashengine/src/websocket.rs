use std::fmt::{Debug, Display};
use std::net::TcpStream;
use std::str;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use mmap_sync::synchronizer::Synchronizer;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use crate::{gz_inflate_to_buffer, send_message, send_pong};
use bytecheck::CheckBytes;
use rkyv::{Archive, Serialize, Deserialize};

// non-blocking: https://github.com/haxpor/bybit-shiprekt/blob/6c3c5693d675fc997ce5e76df27e571f2aaaf291/src/main.rs

// TODO: Make non-static
static mut BUFFER: [u8; 4096] = [0; 4096];

pub struct CeWebSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    max_size: usize,
}

#[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes))]
pub struct SharedMessage {
    pub message: Vec<u8>,
}

#[derive(Archive, Serialize, Deserialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes))]
pub struct SharedTick {
    pub channel: String,
    pub timestamp: i32,
    pub sequence: i32,
    pub ask: f64,
    pub ask_size: f64,
    pub bid: f64,
    pub bid_size: f64,
    pub symbol: String,
}
impl Display for SharedTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,
            "Channel: {}, Timestamp: {}, Sequence: {}, Ask: {}, Ask Size: {}, Bid: {}, Bid Size: {}, Symbol: {}",
            self.channel,
            self.timestamp,
            self.sequence,
            self.ask,
            self.ask_size,
            self.bid,
            self.bid_size,
            self.symbol,
        )
    }
}

impl Debug for ArchivedSharedTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchivedSharedTick")
            .field("channel", &self.channel)
            .field("timestamp", &self.timestamp)
            .field("sequence", &self.sequence)
            .field("ask", &self.ask)
            .field("ask_size", &self.ask_size)
            .field("bid", &self.bid)
            .field("bid_size", &self.bid_size)
            .field("symbol", &self.symbol)
            .finish()
    }
}

impl From<&[u8]> for SharedTick {
    fn from(data: &[u8]) -> Self {
        let mut tick = SharedTick {
            channel: String::new(),
            timestamp: 0,
            sequence: 0,
            ask: 0.0,
            ask_size: 0.0,
            bid: 0.0,
            bid_size: 0.0,
            symbol: String::new(),
        };

        let mut length: usize = 0;
        for (i, byte) in data.iter().enumerate() {
            if *byte == 0 {
                length = i;
                break;
            }
            length = i;
        }

        tick.channel = String::from_utf8_lossy(&data[0..length]).to_string();

        tick
    }
}

impl CeWebSocket {
    pub fn connect(url: &str) -> Result<CeWebSocket, tungstenite::Error> {
        let result = tungstenite::connect(url);
        match result {
            Ok((sock, response)) => {
                // Uncomment these lines if you need to debug the response
                //println!("Connected to the server");
                //println!("Response HTTP code: {}", response.status());
                //println!("Response contains the following headers:");
                //for (header, _value) in response.headers() {
                //    println!("* {header}");
                //}

                Ok(CeWebSocket {
                    socket: sock,
                    max_size: 0,
                })
            },
            Err(e) => Err(e)
        }
    }

    pub fn subscribe(&mut self, request: &str) {
        send_message(&mut self.socket, request);
    }

    pub fn run(self, file_path: &str) {
        let file_path = String::from(file_path);
        let mut socket = self.socket;
        let max_size = self.max_size;
        let handle = thread::spawn( move || {
            // TODO: On Linux use tmpfs shared memory: let mut synchronizer = Synchronizer::new("/dev/shm/hello_world".as_ref());
            let mut synchronizer = Synchronizer::new(file_path.as_str().as_ref());
            let mut local_max_size = max_size;
            loop {
                let msg = match socket.read() {
                    Ok(msg) => msg,
                    Err(e) => {
                        eprintln!("Error reading message: {}", e);
                        break;
                    }
                };
                match msg {
                    Message::Text(message) => {
                        println!("Received text message from websocket server: {}", message);
                    },
                    Message::Binary(bytes) => {
                        let vec = bytes.as_ref().to_vec();

                        unsafe {
                            match gz_inflate_to_buffer(&vec, &mut BUFFER) {
                                Ok(size) => {
                                    if size >= 6 && BUFFER.get_unchecked(..6) == b"{\"ping" {
                                        let message = str::from_utf8_unchecked(&BUFFER[..size]);
                                        println!("Received ping from websocket server: {}", message);
                                        send_pong(&mut socket, message);
                                    } else {
                                        /*let message = str::from_utf8_unchecked(&BUFFER[..size]);
                                        //println!("Received message: {}", message);
                                        let shared_message = SharedMessage {
                                            message: BUFFER[..size].to_vec(),
                                        };

                                        let (written, reset) = synchronizer
                                            .write(&shared_message, Duration::from_secs(1))
                                            .expect(format!("Failed to write to mmap file: {}", file_path).as_str());
                                        println!("Wrote {} bytes to mmap file, reset: {}", written, reset);
                                        */

                                        let tick = SharedTick::from(&BUFFER[..size]);
                                        let (written, reset) = synchronizer
                                            .write(&tick, Duration::from_secs(1))
                                            .expect(format!("Failed to write tick to mmap file: {}", file_path).as_str());
                                        println!("Wrote {} bytes of tick to mmap file, reset: {}, tick: {}", written, reset, tick);
                                    }
                                    if size > local_max_size {
                                        local_max_size = size;
                                    }
                                    println!("Max size in bytes: {}", local_max_size);
                                }
                                Err(e) => eprintln!("Failed to parse message: {:?}: {:?}", e, String::from_utf8_lossy(&vec)),
                            }
                        }
                    },
                    Message::Close(close_frame) => {
                        match close_frame {
                            Some(reason) => {
                                println!("Connection closed by server with reason: {}", reason);
                                match socket.close(None) {
                                    Ok(()) => println!("Closed connection to server"),
                                    Err(e) => println!("Failed to close connection to sever: {}", e),
                                }
                            },
                            None => {
                                println!("Connection closed by server without reason");
                                match socket.close(None) {
                                    Ok(()) => println!("Closed connection to server"),
                                    Err(e) => println!("Failed to close connection to server: {}", e),
                                }
                            },
                        }
                        break;
                    },
                    _ => {
                        eprintln!("Received unknown message from server");
                        break;
                    }
                }
            }
        });
        //handle.join().unwrap();
    }
}
