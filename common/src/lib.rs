use std::path::PathBuf;

struct ImageData;

pub enum ClientToServer {
    LoadFolder(PathBuf),
}

pub enum ServerToClient {
    NewSessionData {
        image: ImageData,

    }
}
