use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileRenderError {
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
}
