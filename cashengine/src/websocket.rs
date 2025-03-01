use crate::shm_block_writer::SharedMemoryWriter;
use flate2::read::MultiGzDecoder;
use std::io::Read;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::{io, str};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
// non-blocking: https://github.com/haxpor/bybit-shiprekt/blob/6c3c5693d675fc997ce5e76df27e571f2aaaf291/src/main.rs

pub const CHUNK_SIZE: usize = 1024;

// TODO: Make non-static
static mut BUFFER: [u8; CHUNK_SIZE] = [0; CHUNK_SIZE];

pub struct CeWebSocket {
    id: usize,
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    max_size: usize,
}

impl CeWebSocket {
    pub fn connect(id: usize, url: &str) -> Result<CeWebSocket, tungstenite::Error> {
        let result = tungstenite::connect(url);
        match result {
            Ok((sock, response)) => {
                // Uncomment these lines to debug the response
                //println!("Connected to the server");
                //println!("Response HTTP code: {}", response.status());
                //println!("Response contains the following headers:");
                //for (header, _value) in response.headers() {
                //    println!("* {header}");
                //}

                Ok(CeWebSocket {
                    id,
                    socket: sock,
                    max_size: 0,
                })
            },
            Err(e) => Err(e)
        }
    }

    pub fn subscribe(&mut self, request: &str) {
        self.send_message(request);
    }

    pub fn run<F>(&mut self, mut on_message: F)
    where
        F: FnMut(&[u8]),
    {
        loop {
            let msg = match self.socket.read() {
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
                                    self.send_pong(message);
                                } else {
                                    on_message( &BUFFER[..size]);
                                }
                                if size > self.max_size {
                                    self.max_size = size;
                                }
                                println!("Max size in bytes: {}", self.max_size);
                            }
                            Err(e) => eprintln!("Failed to parse message: {:?}: {:?}", e, String::from_utf8_lossy(&vec)),
                        }
                    }
                },
                Message::Close(close_frame) => {
                    match close_frame {
                        Some(reason) => {
                            println!("Connection closed by server with reason: {}", reason);
                            match self.socket.close(None) {
                                Ok(()) => println!("Closed connection to server"),
                                Err(e) => println!("Failed to close connection to sever: {}", e),
                            }
                        },
                        None => {
                            println!("Connection closed by server without reason");
                            match self.socket.close(None) {
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
    }

    fn send_pong(&mut self, s: &str) {
        let mut pong = String::with_capacity(s.len());
        pong.push_str(&s[..3]);
        pong.push('o');
        pong.push_str(&s[4..]);
        self.send_message(pong.as_str());
    }

    fn send_message(&mut self, s: &str) {
        let sent = String::from(s);
        let msg = Message::text(s);
        match self.socket.send(msg) {
            Ok(()) => {
                println!("Sent {}", sent);
            },
            Err(e) => {
                println!("Error sending message: {}", e);
            }
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