use anyhow::{anyhow, Result};
use common::ServerToClient;
use egui::{CentralPanel, Color32, RichText};
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

pub struct Session {
    sender: WsSender,
    receiver: WsReceiver,
    is_open: bool,
}

pub struct TemplateApp {
    session: Result<Session>,
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        /*
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }
        */

        let session = Session::new();

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
                        self.session = Session::new();
                    }
                });
                return;
            }
        };

        if let Err(e) = session.receive() {
            self.session = Err(e);
            return;
        }
        
        if !session.is_open {
            CentralPanel::default().show(ctx, |ui| {
                ui.label("Connecting, please wait...");
            });
            return;
        }

        CentralPanel::default().show(ctx, |ui| {
            ui.label("Connected!");
        });
    }
}

impl Session {
    pub fn new() -> Result<Self> {
        let options = ewebsock::Options::default();
        let (sender, receiver) =
            ewebsock::connect("ws://127.0.0.1:9001", options).map_err(|e| anyhow!("{e}"))?;
        Ok(Self {
            sender,
            receiver,
            is_open: false,
        })
    }

    pub fn receive(&mut self) -> Result<()> {
        if let Some(msg) = self.receiver.try_recv() {
            match msg {
                WsEvent::Closed => return Err(anyhow!("Remote session was closed!")),
                WsEvent::Opened => self.is_open = true,
                WsEvent::Error(e) => return Err(anyhow!("{e}")),
                WsEvent::Message(msg) => self.handle_ws_msg(msg)?,
            };
        }

        Ok(())
    }

    fn handle_ws_msg(&mut self, msg: WsMessage) -> Result<()> {
        let WsMessage::Binary(msg) = msg else {
            return Ok(());
        };

        let decoded = common::deserialize(&mut std::io::Cursor::new(msg))?;

        self.handle_msg(decoded)
    }

    fn handle_msg(&mut self, msg: ServerToClient) -> Result<()> {

        Ok(())
    }
}
