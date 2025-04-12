use anyhow::{Context, Result};
use common::{deserialize, serialize, ClientToServer, FaceKey, ServerToClient};
use log::{error, info};
use std::net::{TcpListener, TcpStream};
use tungstenite::{accept, Message};

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
            websocket.flush()?;
        }
    }
}

struct ClientSession {}

impl ClientSession {
    pub fn new() -> Self {
        Self {}
    }

    pub fn handle_response(&mut self, msg: ClientToServer) -> Option<ServerToClient> {
        match msg {
            ClientToServer::LoadFolder(path) => {
                std::fs::read_dir(path).ok().map(|files| {
                    ServerToClient::FolderContents(
                        files
                            .filter_map(|f| f.ok())
                            .filter_map(|f| f.file_name().into_string().ok())
                            .filter(|f| f.ends_with(".png"))
                            .map(|f| FaceKey {
                                prefix: f,
                                is_narrow: false,
                            })
                            .collect(),
                    )
                })
            }
            _ => None,
        }
    }
}
