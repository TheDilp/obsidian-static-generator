use std::fs::DirEntry;

pub type Content = Vec<Result<DirEntry, std::io::Error>>;
