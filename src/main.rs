use std::{
    fs::{self, DirEntry, File},
    io::{Read, Write},
    ops::Not,
    path::{Display, Path},
    sync::LazyLock,
};

use anyhow::Result;
use gray_matter::{Matter, engine::YAML};
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

const ROOT_DIR: &str = "content";

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

            let parser = pulldown_cmark::Parser::new(&frontmatter.content);

            let mut html_output = String::new();

            pulldown_cmark::html::push_html(&mut html_output, parser);

            let mut context = Context::new();
            context.insert("title", &title);
            context.insert("content", &html_output);

            let rendered = TERA_ENGINE.render("base.html", &context);

            if let Err(err) = rendered {
                return Err(FileRenderError::Render(err));
            }
            let rendered = rendered.unwrap();
            let current_path = path.to_string().replace(".md", ".html");

            let new_path = &format!("dist/{}", current_path).replace("/content", "");

            let render_path = Path::new(new_path);

            let render_file = fs::File::create(render_path);

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

fn get_valid_entries_from_dir(dir: &str) -> Vec<Result<DirEntry, std::io::Error>> {
    if let Ok(directory) = fs::read_dir(dir) {
        directory
            .filter(|f| f.as_ref().is_ok_and(is_valid_entry))
            .collect::<Vec<Result<DirEntry, std::io::Error>>>()
    } else {
        vec![]
    }
}

fn main() {
    //* Start tracing subscriber */
    tracing_subscriber::fmt::init();

    let _ = fs::create_dir(ROOT_DIR);
    let _ = fs::create_dir("dist");

    //* Filter out invalid content */
    let content = get_valid_entries_from_dir(ROOT_DIR);

    //* Create index page */
    let mut index_page_links: Vec<String> = vec![];

    let mut errored_files: Vec<String> = vec![];

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
            && let Ok(mut file) = fs::File::open(path)
        {
            let render_result = render_file(&mut file, &title, &entry.path().display());

            if let Ok(new_path) = render_result {
                index_page_links.push(new_path);
                tracing::info!("🟢 SUCCESSFULLY RENDERED FILE \"{}\"", title);
            } else if let Err(err) = render_result {
                match err {
                    FileRenderError::ContentEmpty => {
                        tracing::info!("⏭️ FILE CONTENT EMPTY FOR FILE \"{}\" | SKIPPING", title)
                    }
                    FileRenderError::NotPublished => {
                        tracing::info!("⏭️ FILE NOT PUBLISHED FOR FILE \"{}\" | SKIPPING", title)
                    }
                    FileRenderError::Create(err) | FileRenderError::Write(err) => {
                        tracing::error!("🔴 ERROR WITH FILE \"{}\" | ERROR: {}", title, err)
                    }
                    FileRenderError::Render(err) => {
                        tracing::error!("🔴 ERROR WITH FILE \"{}\" | ERROR: {}", title, err)
                    }
                    FileRenderError::FrontmatterParsing(err) => {
                        tracing::error!("🔴 ERROR WITH FILE \"{}\" | ERROR: {}", title, err)
                    }
                }
                errored_files.push(title);
            }
        } else if file_type.is_dir() {
            tracing::info!("THIS IS A DIRECTORY")
        }
    }
    let mut index_context = Context::new();

    index_context.insert("links", &index_page_links);
}
