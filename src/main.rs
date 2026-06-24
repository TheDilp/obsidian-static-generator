use std::{
    fs::{self},
    io::Read,
    ops::Not,
};

use anyhow::Result;
use gray_matter::{Matter, engine::YAML};

#[derive(serde::Deserialize, Debug)]
struct Frontmatter {
    publish: Option<bool>,
}

fn main() -> Result<()> {
    //* Start tracing subscriber */
    tracing_subscriber::fmt::init();

    //* Create parser */
    let content = fs::read_dir("content")?;

    content.for_each(|item| {
        if let Ok(entry) = item
            && entry.file_type().is_ok()
        {
            let file_type = entry.file_type().unwrap();
            if file_type.is_file()
                && let Ok(mut file) = fs::File::open(entry.path())
            {
                let mut file_content = String::new();

                file.read_to_string(&mut file_content).unwrap_or_default();

                if file_content.is_empty().not() {
                    //* Extract the frontmatter first */
                    let matter = Matter::<YAML>::new();

                    let parsed_matter = matter.parse::<Frontmatter>(&file_content).unwrap();

                    if parsed_matter
                        .data
                        .is_some_and(|matter| matter.publish.is_some_and(|published| published))
                    {
                        let parser = pulldown_cmark::Parser::new(&parsed_matter.content);

                        let mut html_output = String::new();

                        pulldown_cmark::html::push_html(&mut html_output, parser);

                        tracing::info!("{:?}", html_output);
                    }
                }
            }
        }
    });

    Ok(())
}
