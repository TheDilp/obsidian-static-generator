use std::{collections::HashSet, fs, io::Write, path::Path};

use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};
use tera::Context;
use unidecode::unidecode;

use crate::{
    consts::{MARKDOWN_PARSER_OPTIONS, OUTPUT_DIR, TERA_ENGINE},
    error::FileRenderError,
    models::{
        file_index::FileIndex,
        markdown_file::{MarkdownFile, Property},
    },
};

pub(crate) fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Renders `[[WikiLink]]` references in a property value: a link to a
/// published note, or plain text when the target isn't published.
fn render_property_value(value: &str, file_index: &FileIndex) -> String {
    let parser = pulldown_cmark::Parser::new_ext(value, *MARKDOWN_PARSER_OPTIONS);
    let mut output = String::new();
    let mut link_open = false;

    for event in parser {
        match event {
            Event::Start(Tag::Link {
                link_type: LinkType::WikiLink { .. },
                ref dest_url,
                ..
            }) => {
                let basename = dest_url.rsplit('/').next().unwrap_or(dest_url);
                if let Some(&idx) = file_index.link_lookup.get(&unidecode(basename)) {
                    let output_path = &file_index.link_summaries[idx].output_path;
                    output.push_str(&format!(
                        r#"<a class="text-blue-400 font-semibold hover:underline" href="/{}">"#,
                        output_path
                    ));
                    link_open = true;
                }
            }
            Event::End(TagEnd::Link) if link_open => {
                output.push_str("</a>");
                link_open = false;
            }
            Event::Text(text) | Event::Code(text) => output.push_str(&escape_html(&text)),
            Event::SoftBreak | Event::HardBreak => output.push(' '),
            _ => {}
        }
    }

    output
}

pub fn render_file(
    markdown_file: &MarkdownFile,
    file_index: &FileIndex,
) -> Result<(String, Vec<usize>), FileRenderError> {
    let (html_output, linked_indices) = render_markdown(&markdown_file.content, file_index);
    let frontmatter = &markdown_file.frontmatter;

    let filtered_properties: Vec<Property> = markdown_file
        .properties
        .iter()
        .filter(|p| p.key.to_lowercase() != "tags")
        .map(|p| Property {
            key: p.key.clone(),
            value: render_property_value(&p.value, file_index),
        })
        .collect();

    let mut context = Context::new();
    context.insert("title", &markdown_file.title);
    context.insert("content", &html_output);
    context.insert("links", &file_index.link_summaries);
    context.insert("output_dir", &*OUTPUT_DIR);
    context.insert("properties", &filtered_properties);
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
    let unique_tags: HashSet<String> = frontmatter
        .tags
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|tag| tag.split('/').next_back().map(String::from))
        .collect();
    context.insert("tags", &unique_tags);
    let new_path = &markdown_file.output_path;
    if let Some(parent) = Path::new(new_path).parent() {
        fs::create_dir_all(parent).map_err(FileRenderError::Create)?;
    }
    let rendered = TERA_ENGINE.render("article.html", &context)?;
    let mut file = fs::File::create(new_path)?;
    file.write_all(rendered.as_bytes())
        .map_err(FileRenderError::Write)?;
    Ok((new_path.to_owned(), linked_indices))
}

/// Shared by articles, canvas text cards, and note previews.
pub(crate) fn render_markdown(content: &str, file_index: &FileIndex) -> (String, Vec<usize>) {
    let parser = pulldown_cmark::Parser::new_ext(content, *MARKDOWN_PARSER_OPTIONS);

    let mut in_unpublished_link = false;
    let mut in_code_block = false;
    let mut in_missing_image = false;
    let mut linked_indices: Vec<usize> = Vec::new();
    let events = parser.filter_map(|event| match event {
        Event::Start(Tag::CodeBlock(_)) => {
            in_code_block = true;
            None
        }
        Event::End(TagEnd::CodeBlock) => {
            in_code_block = false;
            None
        }
        _ if in_code_block => None,
        Event::Start(Tag::Link {
            link_type: LinkType::WikiLink { .. },
            ref dest_url,
            ref title,
            ref id,
        }) => {
            let resolved = crate::canvas::resolve_page(file_index, dest_url);

            match resolved {
                Some(idx) => {
                    linked_indices.push(idx);
                    let resolved_url = format!("/{}", file_index.link_summaries[idx].output_path);
                    Some(Event::Start(Tag::Link {
                        link_type: LinkType::WikiLink { has_pothole: false },
                        dest_url: CowStr::from(resolved_url),
                        title: title.clone(),
                        id: id.clone(),
                    }))
                }
                None => {
                    in_unpublished_link = true;
                    None
                }
            }
        }
        Event::End(TagEnd::Link) if in_unpublished_link => {
            in_unpublished_link = false;
            None
        }
        // Raw HTML cannot execute inside generated cards or articles.
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let image = crate::canvas::resolve_image(file_index, &dest_url);
            in_missing_image = image.is_none();
            image.map(|image| {
                Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::from(format!("/{}", image.link)),
                    title,
                    id,
                })
            })
        }
        Event::End(TagEnd::Image) if in_missing_image => {
            in_missing_image = false;
            None
        }
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let url = if dest_url.ends_with(".md") || dest_url.ends_with(".canvas") {
                crate::canvas::resolve_page(file_index, &dest_url)
                    .map(|idx| format!("/{}", file_index.link_summaries[idx].output_path))
            } else {
                let lower = dest_url.to_ascii_lowercase();
                (!lower.contains(':')
                    || lower.starts_with("https://")
                    || lower.starts_with("http://")
                    || lower.starts_with("mailto:"))
                .then(|| dest_url.to_string())
            };
            if let Some(url) = url {
                Some(Event::Start(Tag::Link {
                    link_type,
                    dest_url: CowStr::from(url),
                    title,
                    id,
                }))
            } else {
                in_unpublished_link = true;
                None
            }
        }
        other => Some(other),
    });

    let mut html_output = String::new();

    pulldown_cmark::html::push_html(&mut html_output, events);
    (html_output, linked_indices)
}
