use std::{
    collections::HashMap,
    fs::{self},
    io::Write,
};

use tera::Context;

use crate::{
    consts::{OUTPUT_DIR, TERA_ENGINE},
    models::{file_index::FileIndex, markdown_file::MarkdownFile},
    process::render_file,
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

    //* Create required directories */
    let _ = fs::create_dir_all(&root_dir);
    let output_path = OUTPUT_DIR.to_string();
    let _ = fs::create_dir_all(&output_path);
    let _ = fs::copy("static/output.css", format!("{}/output.css", output_path));

    let start = std::time::Instant::now();
    tracing::info!("🚀 STARTED PROCESSING");

    //* Index files */
    let mut index = FileIndex::default();
    index.create_index(&root_dir, &root_dir);
    tracing::info!("🏁 FINISHED INDEXING IN {:?}", start.elapsed());

    index.remove_unpublished_images();
    tracing::info!("🏁 FINISHED REMOVING IN {:?}", start.elapsed());

    index.copy_images();
    tracing::info!("🏁 FINISHED COPYING IMAGES IN {:?}", start.elapsed());

    for file in &index.markdown_files {
        if let Err(err) = render_file(file, &index) {
            println!("{err}");
            continue;
        }
    }

    let index_file_create = fs::File::create(format!("{}/index.html", *OUTPUT_DIR));

    if let Ok(mut index_file) = index_file_create {
        let mut index_context = Context::new();
        index_context.insert("output_dir", &*OUTPUT_DIR);
        let mut grouped_index: HashMap<char, Vec<MarkdownFile>> = HashMap::new();

        for item in index.markdown_files {
            if let Some(letter) = item.title.chars().next() {
                grouped_index.entry(letter).or_default().push(item);
            };
        }
        let mut pairs: Vec<(char, Vec<MarkdownFile>)> = grouped_index.into_iter().collect();
        pairs.sort_by_key(|a| a.0);
        index_context.insert("links", &pairs);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }

    tracing::info!("🏁 FINISHED PROCESSING IN {:?}", start.elapsed());
}
