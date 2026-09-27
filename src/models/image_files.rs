use serde::Serialize;

#[derive(Serialize, Debug, Hash, PartialEq, Eq)]
pub struct ImageFileLink {
    pub title: String,
    pub link: String,
    pub original_path: String,
    pub extension: String,
}
