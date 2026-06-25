use std::{
    fs::{self, DirEntry, File},
    io::{Read, Write},
    ops::Not,
    path::Display,
    sync::LazyLock,
};

use anyhow::Result;
use gray_matter::{Matter, engine::YAML};
use regex::Regex;
use tera::{Context, Tera};
use thiserror::Error;

static TERA_ENGINE: LazyLock<Tera> = LazyLock::new(|| {
    //* Load templates */
    let mut tera = match Tera::new("templates/**/*.html") {
        Ok(t) => t,
        Err(e) => {
            println!("Parsing error(s): {}", e);
            ::std::process::exit(1);
        }
    };
    tera.autoescape_on(vec![".html"]);
    tera
});
static WIKILINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\[(?P<link>[^#|\]]+)(?:#(?P<heading>[^|\]]+))?(?:\|(?P<text>[^\]]+))?\]\]")
        .unwrap()
});

const ROOT_DIR: &str = "content";
const OUTPUT_DIR: &str = "dist";

#[derive(Debug, Error)]
enum FileRenderError {
    #[error("Failed to render file | ERROR: {0}")]
    Render(#[from] tera::Error),
    #[error("Failed to create file | ERROR: {0}")]
    Create(#[from] std::io::Error),
    #[error("Failed to write file | ERROR: {0}")]
    Write(std::io::Error),
    #[error("Failed to parse frontmatter | ERROR: {0}")]
    FrontmatterParsing(#[from] gray_matter::value::error::Error),
    #[error("Content of file is empty")]
    ContentEmpty,
    #[error("File is NOT publish = true")]
    NotPublished,
}
#[derive(serde::Deserialize, Debug)]
struct Frontmatter {
    publish: Option<bool>,
}

#[derive(Default)]
struct ContentProcessResult {
    success: u64,
    skipped: u64,
    errored: u64,
}

type Content = Vec<Result<DirEntry, std::io::Error>>;
struct Wikilink {
    alias: Option<String>,
    heading: Option<String>,
    link: String,
}
fn extract_wikilinks(text: &str) -> Vec<Wikilink> {
    let mut results: Vec<Wikilink> = vec![];
    for capture in WIKILINK_RE.captures_iter(text) {
        let link = capture
            .name("link")
            .map(|s| s.as_str().to_string())
            .unwrap_or_default();

        if link.is_empty() {
            continue;
        }

        let heading = capture.name("heading").map(|s| s.as_str().to_string());

        let alias = capture.name("text").map(|s| s.as_str().to_string());

        results.push(Wikilink {
            alias,
            heading,
            link,
        });
    }
    results
}

fn render_file(file: &mut File, title: &String, path: &Display) -> Result<String, FileRenderError> {
    let mut file_content = String::new();

    file.read_to_string(&mut file_content).unwrap_or_default();

    //* Parse only non-empty files  */
    if file_content.is_empty().not() {
        //* Extract the frontmatter  */
        let matter = Matter::<YAML>::new();

        let parsed_matter = matter.parse::<Frontmatter>(&file_content);

        if let Ok(frontmatter) = parsed_matter {
            if frontmatter
                .data
                .is_none_or(|fm| fm.publish.is_none_or(|publish| !publish))
            {
                return Err(FileRenderError::NotPublished);
            }

            let _ = extract_wikilinks(&frontmatter.content);

            let parser = pulldown_cmark::Parser::new(&frontmatter.content);

            let mut html_output = String::new();

            pulldown_cmark::html::push_html(&mut html_output, parser);

            let mut context = Context::new();
            context.insert("title", &title);
            context.insert("content", &html_output);

            let rendered = TERA_ENGINE.render("article.html", &context);

            if let Err(err) = rendered {
                return Err(FileRenderError::Render(err));
            }
            let rendered = rendered.unwrap();
            let current_path = path.to_string().replace(".md", ".html");
            let new_path = &format!("{}/{}", OUTPUT_DIR, current_path).replace(ROOT_DIR, "");

            let mut path_segments = new_path.split("/").collect::<Vec<&str>>();

            let render_path = path_segments.pop().unwrap_or_default();
            let dirs_path = path_segments.join("/");

            let _ = fs::create_dir_all(&dirs_path);
            let render_file = fs::File::create(format!("{}/{}", dirs_path, render_path));

            if let Err(err) = render_file {
                return Err(FileRenderError::Create(err));
            }
            let mut render_file = render_file.unwrap();
            let write_result = render_file.write_all(&rendered.into_bytes());

            if let Err(err) = write_result {
                Err(FileRenderError::Write(err))
            } else {
                Ok(new_path.to_owned())
            }
        } else if let Err(err) = parsed_matter {
            Err(FileRenderError::FrontmatterParsing(err))
        } else {
            Err(FileRenderError::FrontmatterParsing(
                gray_matter::Error::value_missing(),
            ))
        }
    } else {
        Err(FileRenderError::ContentEmpty)
    }
}

fn is_valid_entry(entry: &DirEntry) -> bool {
    let Ok(file_type) = entry.file_type() else {
        return false;
    };
    if entry.file_name().is_empty() {
        return false;
    };
    file_type.is_dir()
        || (file_type.is_file() && entry.path().extension().is_some_and(|ext| ext == "md"))
}

fn get_valid_entries_from_dir(dir: &str) -> Content {
    if let Ok(directory) = fs::read_dir(dir) {
        directory
            .filter(|f| f.as_ref().is_ok_and(is_valid_entry))
            .collect::<Vec<Result<DirEntry, std::io::Error>>>()
    } else {
        vec![]
    }
}

fn process_content(
    content: Content,
    index_page_links: &mut Vec<String>,
    process_result_count: &mut ContentProcessResult,
) {
    for item in content {
        //* Checked previously when filtering invalid content */
        let entry = item.unwrap();
        let file_type = entry.file_type().unwrap();
        //* Likewise title is validated to NOT be empty */
        let title = entry
            .file_name()
            .to_str()
            .map(|s| s.to_string().replace(".md", ""))
            .unwrap_or_default();

        let path = entry.path();

        if file_type.is_file()
            && let Ok(mut file) = fs::File::open(&path)
        {
            let render_result = render_file(&mut file, &title, &entry.path().display());

            if let Ok(new_path) = render_result {
                index_page_links.push(new_path);
                tracing::info!("🟢 SUCCESSFULLY RENDERED FILE \"{}\"", title);
                process_result_count.success += 1;
            } else if let Err(err) = render_result {
                match err {
                    FileRenderError::ContentEmpty => {
                        tracing::info!("⏭️ FILE CONTENT EMPTY FOR FILE \"{}\" | SKIPPING", title);
                        process_result_count.skipped += 1;
                    }
                    FileRenderError::NotPublished => {
                        tracing::info!("⏭️ FILE NOT PUBLISHED FOR FILE \"{}\" | SKIPPING", title);
                        process_result_count.skipped += 1;
                    }
                    FileRenderError::Create(err) => {
                        tracing::error!(
                            "🔴 ERROR WITH CREATING FILE \"{}\" | ERROR: {}",
                            title,
                            err
                        );
                        process_result_count.errored += 1;
                    }
                    FileRenderError::Write(err) => {
                        tracing::error!(
                            "🔴 ERROR WITH WRITING FILE \"{}\" | ERROR: {}",
                            title,
                            err
                        );
                        process_result_count.errored += 1;
                    }
                    FileRenderError::Render(err) => {
                        tracing::error!(
                            "🔴 ERROR WITH RENDERING FILE \"{}\" | ERROR: {}",
                            title,
                            err
                        );
                        process_result_count.errored += 1;
                    }
                    FileRenderError::FrontmatterParsing(err) => {
                        tracing::error!(
                            "🔴 ERROR WITH FRONTMATTER PARSING FILE \"{}\" | ERROR: {}",
                            title,
                            err
                        );
                        process_result_count.errored += 1;
                    }
                }
            }
        } else if file_type.is_dir()
            && let Some(dir_path) = path.to_str()
        {
            let content = get_valid_entries_from_dir(dir_path);
            process_content(content, index_page_links, process_result_count);
        }
    }
}

fn main() {
    //* Start tracing subscriber */
    tracing_subscriber::fmt::init();

    let _ = fs::create_dir(ROOT_DIR);
    let _ = fs::create_dir(OUTPUT_DIR);

    //* Get valid content in root directory */
    let content = get_valid_entries_from_dir(ROOT_DIR);

    let mut process_result_count = ContentProcessResult::default();

    let mut index_page_links: Vec<String> = vec![];

    let start = std::time::Instant::now();
    //* Process content starting with root directory */
    process_content(content, &mut index_page_links, &mut process_result_count);

    println!("\n");
    println!("\n");
    tracing::info!(
        "🟢 NUMBER OF RENDERED FILES: {}",
        process_result_count.success
    );
    tracing::info!(
        "⏭️ NUMBER OF SKIPPED FILES: {}",
        process_result_count.skipped
    );
    tracing::info!(
        "🔴 NUMBER OF ERRORED FILES: {}",
        process_result_count.errored
    );
    tracing::info!(
        "📊 TOTAL FILES PROCESSED: {} IN {} milliseconds.",
        process_result_count.success + process_result_count.skipped + process_result_count.errored,
        start.elapsed().as_millis()
    );

    let index_file_create = fs::File::create(format!("{}/index.html", OUTPUT_DIR));

    if let Ok(mut index_file) = index_file_create {
        let mut index_context = Context::new();
        index_context.insert("links", &index_page_links);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }
}
