use std::{fs, io::Write};

use tera::Context;
use unidecode::unidecode;

use crate::{
    consts::{MARKDOWN_PARSER_OPTIONS, OUTPUT_DIR, TERA_ENGINE},
    error::FileRenderError,
    models::{file_index::FileIndex, markdown_file::MarkdownFile},
};

pub fn render_file(
    markdown_file: &MarkdownFile,
    file_index: &FileIndex,
) -> Result<String, FileRenderError> {
    let parser = pulldown_cmark::Parser::new_ext(&markdown_file.content, *MARKDOWN_PARSER_OPTIONS);

    let mut html_output = String::new();

    pulldown_cmark::html::push_html(&mut html_output, parser);

    let frontmatter = &markdown_file.frontmatter;

    let mut context = Context::new();
    context.insert("title", &markdown_file.title);
    context.insert("content", &html_output);
    context.insert("links", &file_index.markdown_files);
    context.insert("output_dir", &*OUTPUT_DIR);
    if let Some(image) = frontmatter.image.as_ref().and_then(|images| images.first()) {
        let image_title = image.replace("[[", "").replace("]]", "");

        let image_file = file_index
            .image_files
            .iter()
            .find(|img| unidecode(&img.title).ends_with(&unidecode(image_title.as_str())));

        if let Some(image_file) = image_file {
            let mut dirs_path = image_file.link.split("/").collect::<Vec<&str>>();
            dirs_path.pop();
            let dirs_path = dirs_path.join("/");
            let _ = fs::create_dir_all(&dirs_path).inspect(|_| {
                let res = fs::copy(&image_file.original_path, &image_file.link);
                if res.is_ok() {
                    context.insert("image", &image_file.link);
                } else {
                    eprintln!("{}", res.err().unwrap());
                }
            });
        }
    }

    let tags = frontmatter.tags.clone().unwrap_or_default();

    context.insert("tags", &tags);

    let new_path = &markdown_file.output_path;

    let mut path_segments = new_path.split("/").collect::<Vec<&str>>();

    path_segments.pop().unwrap_or_default();
    let dirs_path = path_segments.join("/");

    let _ = fs::create_dir_all(&dirs_path);

    let render_file = fs::File::create(new_path);

    if let Err(err) = render_file {
        return Err(FileRenderError::Create(err));
    }
    let rendered = TERA_ENGINE.render("article.html", &context);

    if let Err(err) = rendered {
        return Err(FileRenderError::Render(err));
    }
    let rendered = rendered.unwrap();

    let mut render_file = render_file.unwrap();
    let write_result = render_file.write_all(&rendered.into_bytes());

    if let Err(err) = write_result {
        Err(FileRenderError::Write(err))
    } else {
        Ok(new_path.to_owned())
    }
}
