use std::{
    collections::HashMap,
    fs::{self},
    io::Write,
};

use tera::Context;

use crate::{
    consts::{OUTPUT_DIR, TERA_ENGINE},
    models::{file_index::FileIndex, markdown_file::MarkdownFile},
};

mod consts;
mod error;
mod models;
mod preprocess;
mod process;
mod types;

fn main() {
    //* Start tracing subscriber */
    tracing_subscriber::fmt::init();
    let root_dir = std::env::var("ROOT_DIR").expect("NO ROOT_DIR SET!");

    let start = std::time::Instant::now();
    tracing::info!("🚀 STARTED PROCESSING");

    //* Create required directories */
    let _ = fs::create_dir(&root_dir);
    let output_path = OUTPUT_DIR.to_string();
    let _ = fs::create_dir(output_path);

    //* Index files */
    let mut index = FileIndex::default();
    index.create_index(&root_dir, &root_dir);
    index.remove_unpublished_images();
    index.copy_images();

    let index_file_create = fs::File::create(format!("{}/index.html", *OUTPUT_DIR));

    if let Ok(mut index_file) = index_file_create {
        let mut index_context = Context::new();

        let mut grouped_index: HashMap<char, Vec<MarkdownFile>> = HashMap::new();

        for item in index.markdown_files {
            if let Some(letter) = item.title.chars().next() {
                grouped_index.entry(letter).or_default().push(item);
            };
        }
        index_context.insert("links", &grouped_index);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }

    tracing::info!("🏁 FINISHED PROCESSING IN {:?}", start.elapsed());
}
