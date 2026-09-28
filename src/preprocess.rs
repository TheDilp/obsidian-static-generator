use std::fs::{self, DirEntry};

use crate::{consts::VALID_EXTENSIONS, types::Content};

fn is_valid_entry(entry: &DirEntry) -> bool {
    let Ok(file_type) = entry.file_type() else {
        return false;
    };
    if entry.file_name().is_empty() {
        return false;
    };
    if file_type.is_dir() && entry.file_name().to_str().is_some_and(|name| name.starts_with('.')) {
        return false;
    }

    file_type.is_dir()
        || (file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| VALID_EXTENSIONS.contains(&ext.to_str().unwrap_or_default())))
}

pub fn get_valid_entries_from_dir(dir: &str) -> Content {
    if let Ok(directory) = fs::read_dir(dir) {
        directory
            .filter_map(Result::ok)
            .filter(is_valid_entry)
            .collect::<Vec<DirEntry>>()
    } else {
        vec![]
    }
}
