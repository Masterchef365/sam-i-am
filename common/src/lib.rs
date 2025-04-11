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

#[derive(Deserialize, Serialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Serialize)]
pub struct Defect {
    pub polygon: Vec<Point>,
    pub class: String,
}

#[derive(Deserialize, Serialize)]
pub struct AnnotationData {
    pub polygons: Vec<Defect>,
}

#[derive(Deserialize, Serialize)]
pub enum NarrowOrWide {
    Narrow,
    Wide,
}

/// Protocol messages sent from client to server
#[derive(Deserialize, Serialize)]
pub enum ClientToServer {
    /// Set the folder for the current session. Induces
    LoadFolder(PathBuf),
    /// Loads the file with the given prefix, and the given face
    LoadPath(String, NarrowOrWide),
    /// Annotation events
    Annotate(AnnotationEvent),
}

#[derive(Deserialize, Serialize)]
pub enum AnnotationEvent {
    Sam(SamEvent),
    NewDefect(Defect),
    Delete(usize),
    EditDefect(usize, Defect),
}

#[derive(Deserialize, Serialize)]
pub enum SamEvent {
    /// True if positive click, false if negative click
    Click(Point, bool),
    /// The user used the bounding box tool
    BoundingBox(Point, Point),
}

/// Protocol messages sent from server to client
#[derive(Deserialize, Serialize)]
pub enum ServerToClient {
    /// Returned contents of a folder
    FolderContents(Vec<String>),
    /// Image and annotations loaded from disk
    InitialLoad(ImageData, AnnotationData),
    /// An annotation event was fired
    ServerUpdated(AnnotationData),
}
