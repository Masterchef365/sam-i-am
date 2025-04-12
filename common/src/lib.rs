use bincode::error::DecodeError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{io::Write, path::PathBuf};

pub fn serialize<T: Serialize, W: Write>(
    mut writer: W,
    val: &T,
) -> Result<(), bincode::error::EncodeError> {
    bincode::serde::encode_into_std_write(val, &mut writer, bincode::config::standard())?;
    Ok(())
}

pub fn deserialize<'r, D: DeserializeOwned, R: std::io::Read>(
    src: &'r mut R,
) -> Result<D, DecodeError> {
    bincode::serde::decode_from_std_read(src, bincode::config::standard())
}

/// An RGB image sent over the wire
#[derive(Deserialize, Serialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Defect {
    pub polygon: Vec<Point>,
    pub class: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AnnotationData {
    pub polygons: Vec<Defect>,
}

/// Refers to a specific face of a board
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FaceKey {
    /// Prefix to the file name, e.g. 20250306_054339_38x184_793738TR
    pub prefix: String,
    /// Whether this face is narrow or wide
    pub is_narrow: bool,
}

/// Protocol messages sent from client to server
#[derive(Deserialize, Serialize, Debug)]
pub enum ClientToServer {
    /// Set the folder for the current session.
    LoadFolder(String),
    /// Loads the file with the given prefix, and the given face
    LoadPath(FaceKey),
    /// Annotation events
    Annotate(AnnotationEvent),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum AnnotationEvent {
    Sam(SamEvent),
    NewDefect(Defect),
    Delete(usize),
    EditDefect(usize, Defect),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum SamEvent {
    /// True if positive click, false if negative click
    Click(Point, bool),
    /// The user used the bounding box tool
    BoundingBox(Point, Point),
}

/// Protocol messages sent from server to client
#[derive(Deserialize, Serialize, Debug)]
pub enum ServerToClient {
    /// Returned contents of a folder
    FolderContents(Vec<FaceKey>),
    /// Image and annotations loaded from disk
    InitialLoad(FaceKey, ImageData, AnnotationData),
    /// An annotation event was fired
    ServerUpdated(AnnotationData),
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RGB Image {}x{}", self.width, self.height)
    }
}
