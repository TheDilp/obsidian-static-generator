use std::collections::HashSet;

use crate::{
    error::FileRenderError,
    models::{
        image_files::ImageFileLink,
        markdown_file::{Frontmatter, MarkdownFile},
    },
    preprocess::get_valid_entries_from_dir,
    process::render_file,
};

use std::{
    fs::{self},
    io::Read,
    ops::Not,
};

use gray_matter::{Matter, engine::YAML};

use crate::consts::OUTPUT_DIR;

#[derive(Default)]
pub struct FileIndex {
    pub markdown_files: Vec<MarkdownFile>,
    pub image_files: Vec<ImageFileLink>,
    pub markdown_image_files: HashSet<String>,
}

impl FileIndex {
    pub fn create_index(&mut self, dir: &str, root_dir: &str) {
        let content = get_valid_entries_from_dir(dir);

        for item in content
            .iter()
            .filter(|entry| entry.as_ref().is_ok_and(|s| s.file_type().is_ok()))
        {
            //* Checked above */
            let entry = item.as_ref().unwrap();
            let file_type = entry.file_type().unwrap();

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
                        let matter = Matter::<YAML>::new();

                        let parsed_matter = matter.parse::<Frontmatter>(&file_content);

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
                            let output_path = format!(
                                "{}/{}",
                                *OUTPUT_DIR,
                                relative_path.replace(".md", ".html")
                            );

                            let title = title.replace(".md", "");

                            let fm = frontmatter.data.unwrap();
                            let mut new_file = MarkdownFile {
                                title,
                                origin_path,
                                output_path,
                                content: frontmatter.content,
                                frontmatter: fm.clone(),
                                images: vec![],
                            };

                            new_file.images = fm.image.unwrap_or_default();

                            for img in &new_file.images {
                                self.markdown_image_files
                                    .insert(img.replace("[[", "").replace("]]", ""));
                            }

                            if let Err(err) = render_file(&new_file, self) {
                                println!("{err}");
                                continue;
                            }

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
                        let link = format!("{}/{}", *OUTPUT_DIR, relative_path);
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
            .retain(|img| self.markdown_image_files.contains(&img.title));
    }

    pub fn copy_images(&self) {
        for image in &self.image_files {
            let mut dirs_path = image.link.split("/").collect::<Vec<&str>>();
            dirs_path.pop();
            let dirs_path = dirs_path.join("/");
            let res = fs::create_dir_all(&dirs_path).inspect(|_| {
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
