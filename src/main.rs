use std::{
    collections::{HashMap, HashSet},
    fs::{self, DirEntry, File},
    io::{Read, Write},
    ops::Not,
    path::Display,
    sync::LazyLock,
};

use anyhow::Result;
use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::Options;
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

static MARKDOWN_PARSER_OPTIONS: LazyLock<Options> = LazyLock::new(|| {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_WIKILINKS);
    options.insert(Options::ENABLE_MATH);

    options
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
    errored: u64,
}

type Content = Vec<Result<DirEntry, std::io::Error>>;
type FileIndex = HashSet<String>;

fn render_file(
    file: &mut File,
    title: &String,
    path: &Display,
    file_index: &FileIndex,
) -> Result<String, FileRenderError> {
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

            let parser =
                pulldown_cmark::Parser::new_ext(&frontmatter.content, *MARKDOWN_PARSER_OPTIONS);

            let mut html_output = String::new();

            pulldown_cmark::html::push_html(&mut html_output, parser);

            let mut context = Context::new();
            context.insert("title", &title);
            context.insert("content", &html_output);
            context.insert("links", file_index);

            let rendered = TERA_ENGINE.render("article.html", &context);

            if let Err(err) = rendered {
                return Err(FileRenderError::Render(err));
            }
            let rendered = rendered.unwrap();
            let current_path = path.to_string().replace(".md", ".html");
            let new_path = &format!("{}/{}", OUTPUT_DIR, current_path);

            let mut path_segments = new_path.split("/").collect::<Vec<&str>>();

            path_segments.pop().unwrap_or_default();
            let dirs_path = path_segments.join("/");

            let _ = fs::create_dir_all(&dirs_path);
            let render_file = fs::File::create(new_path);

            if let Err(err) = render_file {
                return Err(FileRenderError::Create(err));
            }
            let mut render_file = render_file.unwrap();
            let write_result = render_file.write_all(&rendered.into_bytes());

            if let Err(err) = write_result {
                Err(FileRenderError::Write(err))
            } else {
                let mut link_path = current_path.replace(ROOT_DIR, "");
                link_path.remove(0);
                Ok(link_path)
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
        || (file_type.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "canvas" || ext == "base"))
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

fn index_files(content: &Content, file_index: &mut FileIndex) {
    for item in content.iter().filter(|entry| entry.is_ok()) {
        //* Checked previously when filtering invalid content */
        let entry = item.as_ref().unwrap();
        let file_type = entry.file_type().unwrap();

        let path = entry.path();

        if file_type.is_file()
            && let Ok(mut file) = fs::File::open(&path)
        {
            let mut file_content = String::new();

            file.read_to_string(&mut file_content).unwrap_or_default();

            //* Parse only non-empty files  */
            if file_content.is_empty().not() {
                //* Extract the frontmatter  */
                let matter = Matter::<YAML>::new();

                let parsed_matter = matter.parse::<Frontmatter>(&file_content);

                if let Ok(frontmatter) = parsed_matter
                    && frontmatter
                        .data
                        .is_some_and(|fm| fm.publish.is_some_and(|publish| publish))
                {
                    let key = path.to_str().map(|s| s.to_string()).unwrap_or_default();
                    file_index.insert(key);
                }
            }
        } else if file_type.is_dir()
            && let Some(dir_path) = path.to_str()
        {
            let content = get_valid_entries_from_dir(dir_path);
            index_files(&content, file_index);
        }
    }
}

fn process_content(
    content: Content,
    process_result_count: &mut ContentProcessResult,
    index: &FileIndex,
) {
    let filtered = content.iter().filter(|item| {
        if item.is_err() {
            return false;
        };
        let item = item.as_ref().unwrap();
        if item.metadata().is_err() {
            return false;
        }
        let metadata = item.metadata().unwrap();
        if metadata.is_dir() {
            true
        } else {
            metadata.is_file()
                && index.contains(
                    &item
                        .path()
                        .to_str()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                )
        }
    });

    for item in filtered {
        //* Checked previously when filtering invalid content */
        let entry = item.as_ref().unwrap();
        let file_type = entry.file_type().unwrap();
        let title = entry
            .file_name()
            .to_str()
            //Todo: CLEAR OTHER FILE EXTENSIONS USING REGEX
            .map(|s| s.to_string().replace(".md", ""))
            .unwrap_or_default();

        //* Skip if file title string is empty */
        if title.is_empty() {
            continue;
        }

        let path = entry.path();

        if file_type.is_file()
            && let Ok(mut file) = fs::File::open(&path)
        {
            let render_result = render_file(&mut file, &title, &entry.path().display(), index);

            if render_result.is_ok() {
                tracing::info!("🟢 SUCCESSFULLY RENDERED FILE \"{}\"", title);
                process_result_count.success += 1;
            } else if let Err(err) = render_result {
                match err {
                    FileRenderError::ContentEmpty | FileRenderError::NotPublished => {}
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
            process_content(content, process_result_count, index);
        }
    }
}

fn main() {
    //* Start tracing subscriber */
    // tracing_subscriber::fmt::init();

    //* Create required directories */
    let _ = fs::create_dir(ROOT_DIR);
    let _ = fs::create_dir(OUTPUT_DIR);

    //* Get valid content in root directory */
    let content = get_valid_entries_from_dir(ROOT_DIR);

    //* Index files */
    let mut index = HashSet::new();
    index_files(&content, &mut index);

    let mut process_result_count = ContentProcessResult::default();

    let start = std::time::Instant::now();
    //* Process content starting with root directory */
    process_content(content, &mut process_result_count, &index);
    println!("\n");
    println!("\n");
    tracing::info!(
        "🟢 NUMBER OF RENDERED FILES: {}",
        process_result_count.success
    );

    tracing::info!(
        "🔴 NUMBER OF ERRORED FILES: {}",
        process_result_count.errored
    );
    tracing::info!(
        "📊 TOTAL FILES PROCESSED: {} IN {} milliseconds.",
        process_result_count.success + process_result_count.errored,
        start.elapsed().as_millis()
    );

    let index_file_create = fs::File::create(format!("{}/index.html", OUTPUT_DIR));

    if let Ok(mut index_file) = index_file_create {
        let mut index_context = Context::new();

        let mut grouped_index: HashMap<char, Vec<String>> = HashMap::new();

        for item in index {
            if let Some(file_name) = item.split("/").last()
                && let Some(letter) = file_name.chars().next()
            {
                grouped_index.entry(letter).or_default().push(item);
            };
        }
        index_context.insert("links", &grouped_index);

        let content = TERA_ENGINE.render("index.html", &index_context).unwrap();

        index_file.write_all(&content.into_bytes()).unwrap();
    } else if let Err(err) = index_file_create {
        tracing::error!("{}", err);
    }
}
