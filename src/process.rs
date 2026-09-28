use std::{collections::HashSet, fs, io::Write, path::Path};

use pulldown_cmark::{Event, LinkType, Tag, TagEnd};
use tera::Context;
use unidecode::unidecode;

use crate::{
    consts::{MARKDOWN_PARSER_OPTIONS, OUTPUT_DIR, TERA_ENGINE},
    error::FileRenderError,
    models::{file_index::FileIndex, markdown_file::MarkdownFile},
};

fn is_wikilink_published(dest_url: &str, published_titles: &HashSet<String>) -> bool {
    let basename = dest_url.rsplit('/').next().unwrap_or(dest_url);
    published_titles.contains(&unidecode(basename))
}

pub fn render_file(
    markdown_file: &MarkdownFile,
    file_index: &FileIndex,
) -> Result<String, FileRenderError> {
    let parser = pulldown_cmark::Parser::new_ext(&markdown_file.content, *MARKDOWN_PARSER_OPTIONS);

    let mut in_unpublished_link = false;
    let events = parser.filter_map(|event| match event {
        Event::Start(Tag::Link {
            link_type: LinkType::WikiLink { .. },
            ref dest_url,
            ..
        }) if !is_wikilink_published(dest_url, &file_index.published_titles) => {
            in_unpublished_link = true;
            None
        }
        Event::End(TagEnd::Link) if in_unpublished_link => {
            in_unpublished_link = false;
            None
        }
        other => Some(other),
    });

    let mut html_output = String::new();

    pulldown_cmark::html::push_html(&mut html_output, events);

    let frontmatter = &markdown_file.frontmatter;

    let mut context = Context::new();
    context.insert("title", &markdown_file.title);
    context.insert("content", &html_output);
    context.insert("links", &file_index.link_summaries);
    context.insert("output_dir", &*OUTPUT_DIR);
    if let Some(image) = frontmatter.image.as_ref().and_then(|images| images.first()) {
        let image_title = image.replace("[[", "").replace("]]", "");

        let image_file = file_index
            .image_lookup
            .get(&unidecode(image_title.as_str()))
            .map(|&idx| &file_index.image_files[idx]);

        if let Some(image_file) = image_file {
            context.insert("image", &image_file.link);
        }
    }

    context.insert(
        "tags",
        frontmatter.tags.as_deref().unwrap_or_default(),
    );

    let new_path = &markdown_file.output_path;

    if let Some(dirs_path) = Path::new(new_path).parent() {
        let _ = fs::create_dir_all(dirs_path);
    }

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
