use anyhow::{anyhow, ensure, Context, Result};
use common::{deserialize, serialize, ClientToServer, FaceKey, ImageData, ServerToClient};
use log::{error, info};
use std::{
    fmt::Display,
    fs::File,
    net::{TcpListener, TcpStream},
    path::PathBuf,
};
use tiff::{decoder::DecodingResult, ColorType};
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

struct ClientSession {
    selected_folder: Option<PathBuf>,
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            selected_folder: None,
        }
    }

    pub fn handle_response(&mut self, msg: ClientToServer) -> Option<ServerToClient> {
        match msg {
            ClientToServer::LoadFolder(path) => {
                self.selected_folder = Some(path.clone().into());

                std::fs::read_dir(path).ok().map(|files| {
                    ServerToClient::FolderContents(
                        files
                            .filter_map(|f| f.ok())
                            .filter_map(|f| f.file_name().into_string().ok())
                            .filter_map(|f| {
                                f.strip_suffix(".tiff").map(|f| FaceKey {
                                    prefix: f.to_string(),
                                    is_narrow: false,
                                })
                            })
                            .collect(),
                    )
                })
            }
            ClientToServer::LoadKey(key) => self.selected_folder.as_ref().and_then(|folder| {
                let file_path = folder.join(format!("{}.tiff", key.prefix));
                ok_or_log_error(load_image(&file_path)).map(|image_data| {
                    ServerToClient::InitialLoad(key, image_data, Default::default())
                })
            }),
            _ => None,
        }
    }
}

fn ok_or_log_error<T, E: Display>(r: Result<T, E>) -> Option<T> {
    r.inspect_err(|e| log::error!("{e}")).ok()
}

fn load_image(path: &PathBuf) -> Result<ImageData> {
    let mut decoder = tiff::decoder::Decoder::new(File::open(path)?)?;
    let (width, height) = decoder.dimensions()?;
    ensure!(decoder.colortype()? == ColorType::RGB(8));
    let DecodingResult::U8(rgb) = decoder.read_image()? else {
        return Err(anyhow!("Incorrect image type"));
    };

    Ok(ImageData { width, height, rgb })
}
