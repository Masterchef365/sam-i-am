use anyhow::{Context, Result};
use common::{ClientToServer, ServerToClient, deserialize, serialize};
use log::{error, info};
use std::net::{TcpListener, TcpStream};
use tungstenite::{Message, accept};

/// A WebSocket echo server
fn main() {
    env_logger::init();

    let server = TcpListener::bind("127.0.0.1:9001").unwrap();
    for stream in server.incoming() {
        match stream {
            Err(e) => {
                error!("{e:#}");
                continue;
            }
            Ok(stream) => {
                std::thread::spawn(move || {
                    if let Err(e) = client_handler(stream) {
                        error!("{e:#}");
                    }
                });
            }
        }
    }
}

fn client_handler(stream: TcpStream) -> Result<()> {
    let mut websocket = accept(stream)?;
    let mut session = ClientSession::new();
    loop {
        let msg = websocket.read()?;
        let Message::Binary(msg) = msg else {
            continue;
        };
        let msg: ClientToServer =
            deserialize(&mut std::io::Cursor::new(msg)).context("Deserialization")?;

        if let Some(resp) = session.handle_response(msg) {
            let mut buf = vec![];
            serialize(&mut buf, &resp)?;
            websocket.write(Message::Binary(buf.into()))?;
        }
    }
}

struct ClientSession {}

impl ClientSession {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle_response(&mut self, msg: ClientToServer) -> Option<ServerToClient> {
        todo!()
    }
}
