use anyhow::{anyhow, Result};
use common::{AnnotationData, ClientToServer, FaceKey, ImageData, ServerToClient};
use egui::{ahash::HashMap, CentralPanel, Color32, Context, RichText, TextureId, TextureOptions};
use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "eframe template",
        native_options,
        Box::new(|cc| Ok(Box::new(TemplateApp::new(cc)))),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(TemplateApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}

pub struct SocketSession {
    sender: WsSender,
    receiver: WsReceiver,
    is_open: bool,

    data: ClientSession,
}

#[derive(Default)]
pub struct ClientSession {
    /// If the user has chosen a folder, lists the prefixes of the files in that folder
    folder_contents: Option<Vec<FaceKey>>,
    annotation_sess: Option<AnnotationSession>,
}

pub struct AnnotationSession {
    key: FaceKey,
    annotations: AnnotationData,
    image: TextureId,
}

pub struct TemplateApp {
    session: Result<SocketSession>,
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        /*
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }
        */

        let session = SocketSession::new();

        Self { session }
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        //eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let session = match &mut self.session {
            Ok(s) => s,
            Err(e) => {
                let error = RichText::new(format!("Error: {e:#}")).color(Color32::RED);
                CentralPanel::default().show(ctx, |ui| {
                    ui.label(error);
                    if ui.button("Reconnect").clicked() {
                        self.session = SocketSession::new();
                    }
                });
                return;
            }
        };

        if let Err(e) = session.receive(ctx) {
            self.session = Err(e);
            return;
        }

        if !session.is_open {
            CentralPanel::default().show(ctx, |ui| {
                ui.label("Connecting, please wait...");
            });
            return;
        }

        session_gui(ctx, session);
    }
}

impl SocketSession {
    pub fn new() -> Result<Self> {
        let options = ewebsock::Options::default();
        let (sender, receiver) =
            ewebsock::connect("ws://127.0.0.1:9001", options).map_err(|e| anyhow!("{e}"))?;
        Ok(Self {
            sender,
            receiver,
            is_open: false,
            data: ClientSession::default(),
        })
    }

    pub fn receive(&mut self, ctx: &Context) -> Result<()> {
        if let Some(msg) = self.receiver.try_recv() {
            match msg {
                WsEvent::Closed => return Err(anyhow!("Remote session was closed!")),
                WsEvent::Opened => self.is_open = true,
                WsEvent::Error(e) => return Err(anyhow!("{e}")),
                WsEvent::Message(msg) => self.handle_ws_msg(msg, ctx)?,
            };
        }

        Ok(())
    }

    fn handle_ws_msg(&mut self, msg: WsMessage, ctx: &Context) -> Result<()> {
        let WsMessage::Binary(msg) = msg else {
            return Ok(());
        };

        let decoded = common::deserialize(&mut std::io::Cursor::new(msg))?;

        for msg in self.data.handle_msg(decoded, ctx)? {
            self.send_ws_message(msg)?;
        }

        Ok(())
    }

    fn send_ws_message(&mut self, msg: ClientToServer) -> Result<()> {
        let mut buf = vec![];
        common::serialize(&mut buf, &msg)?;
        self.sender.send(WsMessage::Binary(buf));
        Ok(())
    }
}

fn upload_image(ctx: &Context, image: ImageData) -> TextureId {
    let image =
        egui::ColorImage::from_rgb([image.width as usize, image.height as usize], &image.rgb);
    ctx.tex_manager().write().alloc(
        "board texture".to_string(),
        image.into(),
        TextureOptions::NEAREST,
    )
}

impl ClientSession {
    fn handle_msg(&mut self, msg: ServerToClient, ctx: &Context) -> Result<Vec<ClientToServer>> {
        let mut responses = vec![];

        match msg {
            ServerToClient::FolderContents(keys) => self.folder_contents = Some(keys),
            ServerToClient::InitialLoad(key, image, annotations) => {
                let image = upload_image(ctx, image);
                self.annotation_sess = Some(AnnotationSession {
                    key,
                    annotations,
                    image,
                });
            }
            ServerToClient::ServerUpdated(ann) => {
                if let Some(sess) = &mut self.annotation_sess {
                    sess.annotations = ann;
                }
            }
        }

        Ok(responses)
    }
}

fn session_gui(ctx: &Context, sess: &mut SocketSession) {
    CentralPanel::default().show(ctx, |ui| {
        ui.label("Connected!");
    });
}
