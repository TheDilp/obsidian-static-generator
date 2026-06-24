use std::{
    fs::{self},
    io::Read,
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
                let mut buffer = String::new();

                file.read_to_string(&mut buffer).unwrap_or_default();

                tracing::info!("{}", buffer);
            }
        }
    });

    Ok(())
}
