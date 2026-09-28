use std::collections::HashSet;

use crate::{
    error::FileRenderError,
    models::{
        image_files::ImageFileLink,
        markdown_file::{Frontmatter, LinkSummary, MarkdownFile, Property},
    },
    preprocess::get_valid_entries_from_dir,
};

use std::{
    collections::HashMap,
    fs::{self},
    io::Read,
    ops::Not,
    path::Path,
};

use gray_matter::{
    Pod,
    engine::{Engine, YAML},
};
use unidecode::unidecode;

use crate::consts::{MATTER, OUTPUT_DIR};

/// Frontmatter keys already surfaced elsewhere and excluded from the infobox table.
const RESERVED_PROPERTY_KEYS: [&str; 2] = ["publish", "image"];

fn pod_to_display_string(pod: &Pod) -> Option<String> {
    match pod {
        Pod::Null => None,
        Pod::String(value) => Some(value.clone()),
        Pod::Integer(value) => Some(value.to_string()),
        Pod::Float(value) => Some(value.to_string()),
        Pod::Boolean(value) => Some(if *value { "Yes".to_string() } else { "No".to_string() }),
        Pod::Array(items) => {
            let joined = items
                .iter()
                .filter_map(pod_to_display_string)
                .collect::<Vec<_>>()
                .join(", ");
            joined.is_empty().not().then_some(joined)
        }
        //* Nested objects aren't displayed in the infobox table */
        Pod::Hash(_) => None,
    }
}

fn format_property_key(key: &str) -> String {
    key.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_properties(raw_matter: &str) -> Vec<Property> {
    let Ok(Pod::Hash(fields)) = YAML::parse(raw_matter) else {
        return vec![];
    };

    let mut properties: Vec<Property> = fields
        .iter()
        .filter(|(key, _)| !RESERVED_PROPERTY_KEYS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            pod_to_display_string(value).map(|value| Property {
                key: format_property_key(key),
                value,
            })
        })
        .collect();

    properties.sort_by(|a, b| a.key.cmp(&b.key));
    properties
}

#[derive(Default)]
pub struct FileIndex {
    pub markdown_files: Vec<MarkdownFile>,
    pub link_summaries: Vec<LinkSummary>,
    pub image_files: Vec<ImageFileLink>,
    pub markdown_image_files: HashSet<String>,
    pub image_lookup: HashMap<String, usize>,
    pub published_titles: HashSet<String>,
    pub link_lookup: HashMap<String, usize>,
}

impl FileIndex {
    pub fn create_index(&mut self, dir: &str, root_dir: &str) {
        let content = get_valid_entries_from_dir(dir);

        for entry in content {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            let title = entry
                .file_name()
                .to_str()
                .map(String::from)
                .unwrap_or_default();

            if title.is_empty() {
                continue;
            }

            let path = entry.path();
            let extension = path
                .extension()
                .map(|s| s.to_str().unwrap_or_default())
                .unwrap_or_default();

            if extension.is_empty().not()
                && file_type.is_file()
                && let Ok(mut file) = fs::File::open(&path)
            {
                match extension {
                    "md" => {
                        let metadata = file.metadata();
                        let mut file_content = String::new();
                        //* Skip files if they are empty */
                        if let Ok(metadata) = metadata {
                            let is_empty = metadata.len() == 0;
                            if is_empty {
                                eprintln!("{}", FileRenderError::ContentEmpty);
                                continue;
                            } else {
                                let res = file.read_to_string(&mut file_content);
                                if res.is_err() {
                                    continue;
                                }
                            }
                        } else {
                            continue;
                        }

                        //* Extract the frontmatter  */
                        let parsed_matter = MATTER.parse::<Frontmatter>(&file_content);

                        if let Ok(frontmatter) = parsed_matter
                            && frontmatter
                                .data
                                .as_ref()
                                .is_some_and(|fm| fm.publish.is_some_and(|publish| publish))
                        {
                            let path_str = path.to_str();

                            if path_str.is_none() {
                                continue;
                            }
                            let origin_path = path_str.map(String::from).unwrap_or_default();
                            let relative_path = origin_path
                                .strip_prefix(root_dir)
                                .unwrap_or(&origin_path)
                                .trim_start_matches('/')
                                .to_string();
                            let output_path = Path::new(&*OUTPUT_DIR)
                                .join(relative_path.replace(".md", ".html"))
                                .to_string_lossy()
                                .into_owned();

                            let title = title.replace(".md", "");
                            let properties = build_properties(&frontmatter.matter);

                            let fm = frontmatter.data.unwrap();
                            let mut new_file = MarkdownFile {
                                title,
                                origin_path,
                                output_path,
                                content: frontmatter.content,
                                frontmatter: fm.clone(),
                                images: vec![],
                                properties,
                            };

                            new_file.images = fm.image.unwrap_or_default();

                            for img in &new_file.images {
                                let clean_name = img.replace("[[", "").replace("]]", "");
                                self.markdown_image_files.insert(unidecode(&clean_name));
                            }

                            self.link_summaries.push(LinkSummary {
                                title: new_file.title.clone(),
                                output_path: new_file.output_path.clone(),
                            });
                            self.link_lookup.insert(
                                unidecode(&new_file.title),
                                self.link_summaries.len() - 1,
                            );
                            self.published_titles.insert(unidecode(&new_file.title));
                            self.markdown_files.push(new_file);
                        }
                    }
                    "png" | "jpg" | "jpeg" | "webp" | "gif" => {
                        let path_str = path.to_str();

                        if path_str.is_none() {
                            continue;
                        }
                        let original_path = path_str.map(String::from).unwrap_or_default();
                        let relative_path = original_path
                            .strip_prefix(root_dir)
                            .unwrap_or(&original_path)
                            .trim_start_matches('/')
                            .to_string();
                        let link = Path::new(&*OUTPUT_DIR)
                            .join(&relative_path)
                            .to_string_lossy()
                            .into_owned();
                        self.image_files.push(ImageFileLink {
                            title,
                            link,
                            original_path,
                            extension: extension.to_string(),
                        });
                    }
                    _ => {}
                }
            } else if file_type.is_dir()
                && let Some(dir_path) = path.to_str()
            {
                self.create_index(dir_path, root_dir);
            }
        }
    }

    pub fn remove_unpublished_images(&mut self) {
        self.image_files
            .retain(|img| self.markdown_image_files.contains(&unidecode(&img.title)));

        self.image_lookup = self
            .image_files
            .iter()
            .enumerate()
            .map(|(idx, img)| (unidecode(&img.title), idx))
            .collect();
    }

    pub fn copy_images(&self) {
        for image in &self.image_files {
            let dirs_path = Path::new(&image.link).parent();
            let Some(dirs_path) = dirs_path else {
                continue;
            };
            let res = fs::create_dir_all(dirs_path).inspect(|_| {
                let copy_res = fs::copy(&image.original_path, &image.link);

                if copy_res.is_err() {
                    eprintln!("{:?}", copy_res.err());
                }
            });

            if res.is_err() {
                eprintln!("{:?}", res.err());
                continue;
            }
        }
    }
}
