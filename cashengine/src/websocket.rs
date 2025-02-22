use crate::mmap_queue;
use crate::{send_message, send_pong};
use std::fmt::Write;
use std::net::TcpStream;
use std::{io, str};
use std::io::Read;
use flate2::read::MultiGzDecoder;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
// non-blocking: https://github.com/haxpor/bybit-shiprekt/blob/6c3c5693d675fc997ce5e76df27e571f2aaaf291/src/main.rs

pub const CHUNK_SIZE: usize = 1024;

// TODO: Make non-static
static mut BUFFER: [u8; CHUNK_SIZE] = [0; CHUNK_SIZE];

pub struct CeWebSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    max_size: usize,
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

    pub fn run<'a>(self, start_ptr: mmap_queue::ShareablePtr, chunk_size: usize, total_size: usize, id: usize) {
        let mut socket = self.socket;
        let max_size = self.max_size;

        let start_ptr = start_ptr;
        let start_ptr: *mut u8 = start_ptr.0;
        let mut value = String::with_capacity(chunk_size);
        let mut
        offset = chunk_size * id;
        // TODO: On Linux use tmpfs shared memory: let mut synchronizer = Synchronizer::new("/dev/shm/hello_world".as_ref());
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
                                    while offset + chunk_size <= total_size {
                                        value.clear();
                                        write!(&mut value, "{}", str::from_utf8_unchecked(&BUFFER[..size])).expect("TODO: panic message");
                                        assert_eq!(true, value.len() <= chunk_size);

                                        std::ptr::copy_nonoverlapping(
                                            value.as_ptr(),
                                            start_ptr.add(offset),
                                            chunk_size,
                                        );
                                        offset += chunk_size * id;
                                    }
                                    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
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