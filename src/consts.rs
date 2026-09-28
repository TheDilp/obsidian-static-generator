use std::sync::LazyLock;

use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::Options;
use tera::Tera;

pub static MATTER: LazyLock<Matter<YAML>> = LazyLock::new(Matter::<YAML>::new);
pub static TERA_ENGINE: LazyLock<Tera> = LazyLock::new(|| {
    //* Load templates */
    let mut tera = Tera::new();
    if let Err(e) = tera.load_from_glob("templates/**/*.html") {
        println!("Parsing error(s): {}", e);
        ::std::process::exit(1);
    }
    tera.autoescape_on(vec![".html"]);
    tera
});

pub static MARKDOWN_PARSER_OPTIONS: LazyLock<Options> = LazyLock::new(|| {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_GFM);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_WIKILINKS);
    options.insert(Options::ENABLE_SUBSCRIPT);
    options.insert(Options::ENABLE_SUPERSCRIPT);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_MATH);

    options
});

pub static OUTPUT_DIR: LazyLock<String> = LazyLock::new(|| {
    if let Ok(output_dir) = std::env::var("OUTPUT_DIR") {
        output_dir
    } else {
        "output".to_string()
    }
});

pub const VALID_EXTENSIONS: [&str; 9] = [
    "md", "canvas", "base", "jpeg", "jpg", "png", "webp", "avif", "gif",
];
