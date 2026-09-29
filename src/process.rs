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

fn escape_html(text: &str) -> String {
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
    let parser = pulldown_cmark::Parser::new_ext(&markdown_file.content, *MARKDOWN_PARSER_OPTIONS);

    let mut in_unpublished_link = false;
    let mut in_code_block = false;
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
            let basename = dest_url.rsplit('/').next().unwrap_or(dest_url);
            let resolved = file_index.link_lookup.get(&unidecode(basename)).copied();

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
        other => Some(other),
    });

    let mut html_output = String::new();

    pulldown_cmark::html::push_html(&mut html_output, events);
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

    let tags_collection = frontmatter.tags.as_deref().map(|tags| {
        tags.iter()
            .filter_map(|tag| tag.split("/").last().map(|t| t.to_string()))
    });

    let unique_tags: HashSet<String> = match tags_collection {
        Some(tags) => tags.collect(),
        None => HashSet::default(),
    };

    context.insert("tags", &unique_tags);

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
        Ok((new_path.to_owned(), linked_indices))
    }
}
