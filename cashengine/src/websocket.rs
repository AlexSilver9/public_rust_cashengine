use std::net::TcpStream;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use crate::{gz_inflate_to_buffer, send_message, send_pong};

// non-blocking: https://github.com/haxpor/bybit-shiprekt/blob/6c3c5693d675fc997ce5e76df27e571f2aaaf291/src/main.rs

// TODO: Make non-static
static mut BUFFER: [u8; 4096] = [0; 4096];

pub struct CeWebSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
    max_size: usize,
}

pub struct SharedMessage {
    message: Vec<String>,
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

    pub fn run(&mut self) -> Option<String> {
        let msg = self.socket.read().expect("Error reading message");
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
                            }
                            if size > self.max_size {
                                self.max_size = size;
                                println!("Max size: {}", self.max_size);
                            }
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
}
