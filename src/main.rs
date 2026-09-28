use std::{
    collections::{HashMap, HashSet},
    fs::{self},
    io::Write,
};

use rayon::prelude::*;
use tera::Context;

use crate::{
    consts::{OUTPUT_DIR, TERA_ENGINE},
    models::{file_index::FileIndex, markdown_file::LinkSummary},
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

    let render_results: Vec<_> = index
        .markdown_files
        .par_iter()
        .map(|file| render_file(file, &index))
        .collect();

    let error_count = render_results
        .iter()
        .filter(|res| res.is_err())
        .inspect(|res| {
            if let Err(err) = res {
                tracing::error!("{err}");
            }
        })
        .count();

    tracing::info!(
        "🏁 FINISHED RENDERING {} FILES ({} errors) IN {:?}",
        render_results.len(),
        error_count,
        start.elapsed()
    );

    //* Build tag -> articles map (nested tags collapse to their last segment) */
    let mut tag_map: HashMap<String, Vec<LinkSummary>> = HashMap::new();
    for file in &index.markdown_files {
        let Some(tags) = file.frontmatter.tags.as_deref() else {
            continue;
        };
        let unique_tags: HashSet<String> = tags
            .iter()
            .filter_map(|t| t.split('/').last().map(String::from))
            .collect();
        for tag in unique_tags {
            tag_map.entry(tag).or_default().push(LinkSummary {
                title: file.title.clone(),
                output_path: file.output_path.clone(),
            });
        }
    }

    let tags_dir = format!("{}/tags", *OUTPUT_DIR);
    let _ = fs::create_dir_all(&tags_dir);

    let mut tag_pairs: Vec<(String, Vec<LinkSummary>)> = tag_map.into_iter().collect();
    tag_pairs.sort_by(|a, b| a.0.cmp(&b.0));

    for (tag, links) in &tag_pairs {
        let tag_file_create = fs::File::create(format!("{}/{}.html", tags_dir, tag));

        match tag_file_create {
            Ok(mut tag_file) => {
                let mut tag_context = Context::new();
                tag_context.insert("output_dir", &*OUTPUT_DIR);
                tag_context.insert("section", "tags");
                tag_context.insert("tag", tag);
                tag_context.insert("links", links);

                let content = TERA_ENGINE.render("tag.html", &tag_context).unwrap();
                let _ = tag_file.write_all(&content.into_bytes());
            }
            Err(err) => tracing::error!("{}", err),
        }
    }

    let tags_index_create = fs::File::create(format!("{}/index.html", tags_dir));

    if let Ok(mut tags_index_file) = tags_index_create {
        let mut tags_index_context = Context::new();
        tags_index_context.insert("output_dir", &*OUTPUT_DIR);
        tags_index_context.insert("section", "tags");
        let tag_names: Vec<&String> = tag_pairs.iter().map(|(tag, _)| tag).collect();
        tags_index_context.insert("tags", &tag_names);

        let content = TERA_ENGINE
            .render("tags_index.html", &tags_index_context)
            .unwrap();
        let _ = tags_index_file.write_all(&content.into_bytes());
    } else if let Err(err) = tags_index_create {
        tracing::error!("{}", err);
    }

    let index_file_create = fs::File::create(format!("{}/index.html", *OUTPUT_DIR));

    if let Ok(mut index_file) = index_file_create {
        let mut index_context = Context::new();
        index_context.insert("output_dir", &*OUTPUT_DIR);
        index_context.insert("section", "pages");
        let mut grouped_index: HashMap<char, Vec<LinkSummary>> = HashMap::new();

        for item in index.link_summaries {
            if let Some(letter) = item.title.chars().next() {
                grouped_index.entry(letter).or_default().push(item);
            };
        }
        let mut pairs: Vec<(char, Vec<LinkSummary>)> = grouped_index.into_iter().collect();
        pairs.sort_by_key(|a| a.0);
        index_context.insert("links", &pairs);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }

    tracing::info!("🏁 FINISHED PROCESSING IN {:?}", start.elapsed());
}
