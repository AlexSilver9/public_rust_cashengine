use std::net::TcpStream;
use std::str;
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
                let msg = socket.read().expect("Error reading message");
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
                                        let message = str::from_utf8_unchecked(&BUFFER[..size]);
                                        println!("Received message: {}", message);
                                        let shared_message = SharedMessage {
                                            message: BUFFER[..size].to_vec(),
                                        };

                                        let (written, reset) = synchronizer
                                            .write(&shared_message, Duration::from_secs(1))
                                            .expect(format!("Failed to write to mmap file: {}", file_path).as_str());
                                        println!("Wrote {} bytes to mmap file, reset: {}", written, reset);
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
                        println!("Received unknown message from server");
                    }
                }
            }
        });
        handle.join().unwrap();
    }
}
