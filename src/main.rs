use std::{
    collections::HashMap,
    fs::{self},
    io::{BufReader, Write},
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

    //* Create required directories */
    let _ = fs::create_dir_all(&root_dir);
    let output_path = OUTPUT_DIR.to_string();
    let _ = fs::create_dir_all(&output_path);
    let _ = fs::copy(
        "static/output.css",
        format!("{}/css/output.css", output_path),
    );

    //* Cache */
    let mut cache = HashMap::new();
    if let Ok(cache_json) = fs::File::open(format!("{}/cache.json", *OUTPUT_DIR)) {
        let reader = BufReader::new(cache_json);
        let cached = serde_json::from_reader::<_, HashMap<u64, u64>>(reader);
        if let Ok(cached_data) = cached {
            cache = cached_data;
        }
    }

    let start = std::time::Instant::now();
    tracing::info!("🚀 STARTED PROCESSING");

    //* Index files */
    let mut index = FileIndex::default();
    index.create_index(&root_dir, &root_dir, &mut cache);
    index.remove_unpublished_images();
    index.copy_images();

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
        index_context.insert("links", &grouped_index);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }

    tracing::info!("🏁 FINISHED PROCESSING IN {:?}", start.elapsed());
    if let Ok(cache_json) = serde_json::to_value(cache) {
        let cache_file = fs::File::create(format!("{}/cache.json", *OUTPUT_DIR));

        if let Ok(mut file) = cache_file {
            file.write_all(&cache_json.to_string().into_bytes())
                .unwrap();
        }
    }
}
