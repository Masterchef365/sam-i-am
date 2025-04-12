use anyhow::{anyhow, ensure, Context, Result};
use common::{
    deserialize, serialize, AnnotationData, AnnotationEvent, ClientToServer, Defect, FaceKey,
    ImageData, SamEvent, ServerToClient,
};
use image::{DynamicImage, ImageBuffer, Pixel, Rgb, RgbImage};
use log::{error, info};
use std::{
    fmt::Display,
    fs::File,
    net::{TcpListener, TcpStream},
    path::PathBuf,
};
use tiff::{decoder::DecodingResult, ColorType};
use tungstenite::{accept, Message};
use usls::{Options, SamPrompt, Xs, Ys, SAM};

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
    let mut session = ClientSession::new("cpu:0".into())?;
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
    model: SAM,
    board_face: Option<BoardFaceSession>,
}

struct BoardFaceSession {
    xs: Xs,
    ann: AnnotationData,
}

impl ClientSession {
    pub fn new(device: String) -> Result<Self> {
        let mut options_encoder = Options::sam2_base_plus_encoder();
        let mut options_decoder = Options::sam2_base_plus_decoder();
        options_decoder.model_num_dry_run = 0;
        options_encoder.model_num_dry_run = 0;

        let options_encoder = options_encoder
            .with_model_device(device.as_str().try_into()?)
            .commit()?;
        let options_decoder = options_decoder.commit()?;

        let model = SAM::new(options_encoder, options_decoder)?;

        Ok(Self {
            selected_folder: None,
            model,
            board_face: None,
        })
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
                let image_data = ok_or_log_error(load_image(&file_path))?;

                let mut ann = AnnotationData::default();

                self.board_face =
                    ok_or_log_error(BoardFaceSession::new(ann, &mut self.model, &image_data));

                Some(ServerToClient::InitialLoad(
                    key,
                    image_data,
                    Default::default(),
                ))
            }),
            ClientToServer::Annotate(AnnotationEvent::Sam(SamEvent::Click(pos, is_positive))) => {
                if let Some(face) = &mut self.board_face {
                    let prompts = vec![SamPrompt::default().with_postive_point(pos.x, pos.y)];

                    let ys = ok_or_log_error(self.model.decode(&face.xs, &prompts))?;
                    face.ann.polygons.push(ys_to_defect(&ys));

                    Some(ServerToClient::ServerUpdated(face.ann.clone()))
                } else {
                    None
                }
            }
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

impl BoardFaceSession {
    pub fn new(ann: AnnotationData, sam: &mut SAM, image: &ImageData) -> Result<Self> {
        let xs = sam.encode(&[image_data_to_dyn_image(image)])?;
        Ok(Self { xs, ann })
    }
}

fn image_data_to_dyn_image(image: &ImageData) -> DynamicImage {
    DynamicImage::ImageRgb8(
        RgbImage::from_raw(image.width, image.height, image.rgb.clone()).unwrap(),
    )
}

fn ys_to_defect(ys: &Ys) -> Defect {
    let polygon = &ys[0].polygons().unwrap()[0];
    let polygon = polygon.polygon(); // POLYGON POLYGON POLYGON POLYGON
    let points = polygon.exterior().points();
    let polygon = points.map(|point| common::Point { x: point.x() as f32, y: point.y() as f32 }).collect();

    Defect { polygon, class: "some class idk".to_string() }
}
