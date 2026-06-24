use std::{
    fs::{self},
    io::Read,
    ops::Not,
};

use anyhow::Result;

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
                    let parser = pulldown_cmark::Parser::new(&file_content);

                    let mut html_output = String::new();

                    pulldown_cmark::html::push_html(&mut html_output, parser);

                    tracing::info!("{}", html_output);
                }
            }
        }
    });

    Ok(())
}
