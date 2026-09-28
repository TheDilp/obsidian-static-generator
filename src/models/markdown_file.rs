use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone)]
pub struct LinkSummary {
    pub title: String,
    pub output_path: String,
}

#[derive(Serialize, Debug)]
pub struct MarkdownFile {
    pub title: String,
    pub origin_path: String,
    pub output_path: String,
    pub frontmatter: Frontmatter,
    pub content: String,
    pub images: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct Frontmatter {
    pub publish: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub image: Option<Vec<String>>,
}
