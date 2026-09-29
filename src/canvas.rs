use std::{
    collections::HashSet,
    fs,
    path::{Component, Path},
};

use pulldown_cmark::{Event, Tag};
use serde::Deserialize;
use tera::Context;
use unidecode::unidecode;

use crate::{
    consts::{MARKDOWN_PARSER_OPTIONS, OUTPUT_DIR, TERA_ENGINE},
    error::FileRenderError,
    models::{file_index::FileIndex, image_files::ImageFileLink, markdown_file::LinkSummary},
    process::{escape_html, render_markdown},
};

#[derive(Debug, Deserialize)]
pub struct Canvas {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub subpath: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub background: String,
    #[serde(default, rename = "backgroundStyle")]
    pub background_style: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub from_node: String,
    pub to_node: String,
    #[serde(default)]
    pub from_side: String,
    #[serde(default)]
    pub to_side: String,
    #[serde(default)]
    pub from_end: String,
    #[serde(default = "arrow")]
    pub to_end: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub style_attributes: EdgeStyle,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeStyle {
    #[serde(default)]
    pub pathfinding_method: String,
    #[serde(default)]
    pub path: String,
}

fn arrow() -> String {
    "arrow".into()
}

#[derive(Debug)]
pub struct CanvasFile {
    pub title: String,
    pub relative_path: String,
    pub output_path: String,
    pub canvas: Canvas,
}

impl CanvasFile {
    pub fn read(path: &Path, root: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        if value
            .pointer("/metadata/frontmatter/publish")
            .and_then(|v| v.as_bool())
            != Some(true)
        {
            return Ok(None);
        }
        let canvas: Canvas = serde_json::from_value(value)?;
        let relative = path.strip_prefix(root)?;
        let output_path = Path::new(&*OUTPUT_DIR)
            .join(relative.with_extension("canvas.html"))
            .to_string_lossy()
            .into_owned();
        Ok(Some(Self {
            title: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            relative_path: relative.to_string_lossy().into_owned(),
            output_path,
            canvas,
        }))
    }
}

/// Normalize vault-relative references without permitting paths outside the vault.
fn vault_path(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in Path::new(path).components() {
        match part {
            Component::Normal(value) => parts.push(value.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// File cards use exact paths. Wiki links additionally permit an unambiguous stem.
pub(crate) fn resolve_page(index: &FileIndex, target: &str) -> Option<usize> {
    let (target, _) = target.split_once('#').unwrap_or((target, ""));
    let target = vault_path(target)?;
    let mut matches = Vec::new();
    for (i, note) in index.markdown_files.iter().enumerate() {
        let relative = Path::new(&note.origin_path)
            .strip_prefix(&index.root_dir)
            .ok()?
            .to_str()?;
        if relative == target || relative.strip_suffix(".md") == Some(&target) {
            return Some(i);
        }
        if !target.contains('/') && unidecode(&note.title) == unidecode(&target) {
            matches.push(i);
        }
    }
    for (i, canvas) in index.canvas_files.iter().enumerate() {
        let idx = index.markdown_files.len() + i;
        if canvas.relative_path == target
            || canvas.relative_path.strip_suffix(".canvas") == Some(&target)
        {
            return Some(idx);
        }
        if !target.contains('/') && unidecode(&canvas.title) == unidecode(&target) {
            matches.push(idx);
        }
    }
    (matches.len() == 1).then(|| matches[0])
}

pub(crate) fn resolve_image<'a>(index: &'a FileIndex, target: &str) -> Option<&'a ImageFileLink> {
    let target = vault_path(target)?;
    index.image_files.iter().find(|image| {
        Path::new(&image.original_path)
            .strip_prefix(&index.root_dir)
            .ok()
            .and_then(|p| p.to_str())
            == Some(target.as_str())
    })
}

/// Gather dependencies before image pruning. Only published sources contribute assets.
pub fn prepare(index: &mut FileIndex) {
    // Canvas pages have a distinct suffix so changing publication flags can remove stale output.
    let expected: HashSet<&str> = index
        .canvas_files
        .iter()
        .map(|file| file.output_path.as_str())
        .chain(
            index
                .markdown_files
                .iter()
                .map(|file| file.output_path.as_str()),
        )
        .collect();
    remove_stale_pages(Path::new(&*OUTPUT_DIR), &expected);
    for canvas in &index.canvas_files {
        index.link_summaries.push(LinkSummary {
            title: canvas.title.clone(),
            output_path: canvas.output_path.clone(),
        });
    }
    let mut assets = HashSet::new();
    let mut collect_markdown = |text: &str| {
        for event in pulldown_cmark::Parser::new_ext(text, *MARKDOWN_PARSER_OPTIONS) {
            if let Event::Start(Tag::Image { dest_url, .. }) = event
                && let Some(image) = resolve_image(index, &dest_url)
            {
                assets.insert(image.original_path.clone());
            }
        }
    };
    for note in &index.markdown_files {
        collect_markdown(&note.content);
    }
    for canvas in &index.canvas_files {
        for node in &canvas.canvas.nodes {
            if node.kind == "text" {
                collect_markdown(&node.text);
            }
        }
    }
    for canvas in &index.canvas_files {
        for node in &canvas.canvas.nodes {
            let asset = match node.kind.as_str() {
                "file" => &node.file,
                "group" => &node.background,
                _ => continue,
            };
            if let Some(image) = resolve_image(index, asset) {
                assets.insert(image.original_path.clone());
            }
        }
    }
    index.image_dependencies.extend(assets);
}

fn remove_stale_pages(dir: &Path, expected: &HashSet<&str>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            remove_stale_pages(&path, expected);
        } else if kind.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .ends_with(".canvas.html")
            && !expected.contains(path.to_string_lossy().as_ref())
            && let Err(error) = fs::remove_file(&path)
        {
            tracing::warn!(path = %path.display(), %error, "Failed to remove unpublished canvas output");
        }
    }
}

struct Card<'a> {
    node: &'a Node,
    title: String,
    html: String,
}

struct Scene<'a> {
    cards: Vec<Card<'a>>,
    edges: Vec<&'a Edge>,
}

fn scene<'a>(
    file: &'a CanvasFile,
    index: &'a FileIndex,
    ancestry: &mut Vec<String>,
    scope: &str,
) -> Scene<'a> {
    ancestry.push(file.relative_path.clone());
    let mut ids = HashSet::new();
    let mut cards = Vec::new();
    for (ordinal, node) in file.canvas.nodes.iter().enumerate() {
        if !node.x.is_finite()
            || !node.y.is_finite()
            || !node.width.is_finite()
            || !node.height.is_finite()
            || node.width <= 0.0
            || node.height <= 0.0
            || node.id.is_empty()
            || ids.contains(&node.id)
        {
            tracing::warn!(path = %file.relative_path, node = %node.id, "Invalid canvas card geometry or ID");
            continue;
        }
        let (title, html) = match node.kind.as_str() {
            "text" => ("Text".into(), render_markdown(&node.text, index).0),
            "group" => {
                let mut html = String::new();
                if let Some(image) = resolve_image(index, &node.background) {
                    if node.background_style == "repeat" {
                        let pattern = format!("background-{scope}-{ordinal}");
                        html = format!(
                            r#"<svg class="absolute inset-0 h-full w-full opacity-25" aria-hidden="true"><defs><pattern id="{pattern}" width="128" height="128" patternUnits="userSpaceOnUse"><image href="/{}" width="128" height="128"/></pattern></defs><rect width="100%" height="100%" fill="url(#{pattern})"/></svg>"#,
                            escape_html(&image.link)
                        );
                    } else {
                        let sizing = if node.background_style == "ratio" {
                            "object-contain"
                        } else {
                            "object-cover"
                        };
                        html = format!(
                            r#"<img class="absolute inset-0 h-full w-full {sizing} opacity-25" src="/{}" alt="" loading="lazy">"#,
                            escape_html(&image.link)
                        );
                    }
                }
                (node.label.clone(), html)
            }
            "file" => {
                let Some(target) = vault_path(&node.file) else {
                    continue;
                };
                if let Some(image) = resolve_image(index, &target) {
                    (
                        Path::new(&target)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        format!(
                            r#"<img class="h-full w-full object-contain" src="/{}" alt="{}" loading="lazy">"#,
                            escape_html(&image.link),
                            escape_html(&target)
                        ),
                    )
                } else if let Some(note) = index.markdown_files.iter().find(|note| {
                    Path::new(&note.origin_path)
                        .strip_prefix(&index.root_dir)
                        .ok()
                        .and_then(|p| p.to_str())
                        == Some(&target)
                }) {
                    let url = format!("/{}{}", note.output_path, node.subpath);
                    let portrait = note.images.first().and_then(|name| {
                        let name = name.replace("[[", "").replace("]]", "");
                        resolve_image(index, &name).or_else(|| index.image_lookup.get(&unidecode(&name)).map(|&i| &index.image_files[i]))
                    }).map(|image| format!(r#"<img class="max-h-64 w-full object-contain" src="/{}" alt="{}" loading="lazy">"#, escape_html(&image.link), escape_html(&note.title))).unwrap_or_default();
                    (
                        note.title.clone(),
                        format!(
                            r#"<a class="relative z-10 mb-3 inline-block rounded px-1 font-mono text-xs text-gold underline focus-visible:outline-2 focus-visible:outline-gold" href="{}">Open note</a>{portrait}{}"#,
                            escape_html(&url),
                            render_markdown(&note.content, index).0
                        ),
                    )
                } else if let Some(child) = index
                    .canvas_files
                    .iter()
                    .find(|child| child.relative_path == target)
                {
                    let mut html = format!(
                        r#"<a class="relative z-10 mb-3 inline-block rounded px-1 font-mono text-xs text-gold underline focus-visible:outline-2 focus-visible:outline-gold" href="/{}">Open canvas</a>"#,
                        escape_html(&child.output_path)
                    );
                    if !ancestry.contains(&target) {
                        let child_scope = format!("{scope}-{ordinal}");
                        let child_scene = scene(child, index, ancestry, &child_scope);
                        let bounds = child_scene.bounds();
                        let scale = ((node.width - 24.0).max(1.0) / bounds.2)
                            .min((node.height - 80.0).max(1.0) / bounds.3);
                        let tx = (node.width - bounds.2 * scale) / 2.0 - bounds.0 * scale;
                        let ty = 12.0 - bounds.1 * scale;
                        html.push_str(&format!(r#"<div class="relative h-full overflow-hidden" inert aria-hidden="true"><div class="absolute left-0 top-0 h-px w-px origin-top-left" style="transform:translate({tx}px,{ty}px) scale({scale})">{}</div></div>"#, child_scene.diagram(&child_scope)));
                    } else {
                        html.push_str("<p>Recursive canvas reference.</p>");
                    }
                    (child.title.clone(), html)
                } else {
                    tracing::warn!(path = %file.relative_path, target = %target, "Canvas reference is unpublished, missing, or unsupported");
                    continue;
                }
            }
            _ => continue,
        };
        ids.insert(&node.id);
        cards.push(Card { node, title, html });
    }
    // Filter edges only after the complete visible node set has been established.
    let edges = file
        .canvas
        .edges
        .iter()
        .filter(|edge| ids.contains(&edge.from_node) && ids.contains(&edge.to_node))
        .collect();
    ancestry.pop();
    Scene { cards, edges }
}

fn color(value: &str) -> &str {
    match value {
        "1" => "#f87171",
        "2" => "#fb923c",
        "3" => "#facc15",
        "4" => "#4ade80",
        "5" => "#22d3ee",
        "6" => "#c084fc",
        value
            if value.len() == 7
                && value.starts_with('#')
                && value[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            value
        }
        _ => "#a8a29e",
    }
}

impl Scene<'_> {
    fn bounds(&self) -> (f64, f64, f64, f64) {
        if self.cards.is_empty() {
            return (0.0, 0.0, 400.0, 300.0);
        }
        let min_x = self
            .cards
            .iter()
            .map(|c| c.node.x)
            .fold(f64::INFINITY, f64::min);
        let min_y = self
            .cards
            .iter()
            .map(|c| c.node.y)
            .fold(f64::INFINITY, f64::min);
        let max_x = self
            .cards
            .iter()
            .map(|c| c.node.x + c.node.width)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = self
            .cards
            .iter()
            .map(|c| c.node.y + c.node.height)
            .fold(f64::NEG_INFINITY, f64::max);
        (
            min_x - 48.0,
            min_y - 48.0,
            max_x - min_x + 96.0,
            max_y - min_y + 96.0,
        )
    }

    fn diagram(&self, scope: &str) -> String {
        let mut output = String::new();
        let bounds = self.bounds();
        // SVG overlays the cards without intercepting viewer interactions.
        for (ordinal, card) in self.cards.iter().enumerate() {
            let n = card.node;
            let classes = if n.kind == "group" {
                "absolute flex flex-col rounded-lg border-2 border-[color:var(--canvas-color)] bg-ink/50 text-lexicon shadow-none"
            } else {
                "absolute flex flex-col rounded-lg border-2 border-[color:var(--canvas-color)] bg-panel text-lexicon shadow-lg"
            };
            let title_classes = if n.kind == "group" {
                "relative shrink-0 truncate border-0 px-4 py-2 font-serif text-lg text-[color:var(--canvas-color)]"
            } else {
                "relative shrink-0 truncate border-b border-hairline px-4 py-2 font-serif text-lg text-parchment"
            };
            let content_classes = if n.kind == "group" {
                "static p-0"
            } else {
                "relative p-4"
            };
            let header = if n.kind == "text" {
                String::new()
            } else {
                format!(
                    r#"<h2 data-canvas-title class="{title_classes}">{}</h2>"#,
                    escape_html(&card.title)
                )
            };
            output.push_str(&format!(r#"<article class="{classes}" data-node-id="{}" style="left:{}px;top:{}px;width:{}px;height:{}px;z-index:{};--canvas-color:{}">{header}<div data-canvas-content class="{content_classes} min-h-0 max-w-none flex-1 touch-pan-y overflow-auto overscroll-contain text-sm prose prose-invert">{}</div></article>"#,
                escape_html(&n.id), n.x, n.y, n.width, n.height, ordinal + 1, color(&n.color), card.html));
        }
        let descriptions = self
            .edges
            .iter()
            .map(|edge| {
                let from = &self
                    .cards
                    .iter()
                    .find(|card| card.node.id == edge.from_node)
                    .unwrap()
                    .title;
                let to = &self
                    .cards
                    .iter()
                    .find(|card| card.node.id == edge.to_node)
                    .unwrap()
                    .title;
                if edge.label.is_empty() {
                    format!("{from} to {to}")
                } else {
                    format!("{from} to {to}: {}", edge.label)
                }
            })
            .collect::<Vec<_>>()
            .join("; ");
        output.push_str(&format!(r#"<svg class="pointer-events-none absolute overflow-visible" role="img" aria-label="Connections: {}" style="left:{}px;top:{}px;width:{}px;height:{}px;z-index:{}" viewBox="{} {} {} {}">"#, escape_html(&descriptions), bounds.0, bounds.1, bounds.2, bounds.3, self.cards.len() + 1, bounds.0, bounds.1, bounds.2, bounds.3));
        for (ordinal, edge) in self.edges.iter().enumerate() {
            let from = self
                .cards
                .iter()
                .find(|c| c.node.id == edge.from_node)
                .unwrap()
                .node;
            let to = self
                .cards
                .iter()
                .find(|c| c.node.id == edge.to_node)
                .unwrap()
                .node;
            let (a, direction_a) = endpoint(from, to, &edge.from_side);
            let (b, direction_b) = endpoint(to, from, &edge.to_side);
            let control_a = (a.0 + direction_a.0 * 40.0, a.1 + direction_a.1 * 40.0);
            let control_b = (b.0 + direction_b.0 * 40.0, b.1 + direction_b.1 * 40.0);
            let path = if edge.style_attributes.pathfinding_method == "square" {
                let mid = if direction_a.0 != 0.0 && direction_b.0 != 0.0 {
                    let x = (control_a.0 + control_b.0) / 2.0;
                    format!("L{x} {} L{x} {}", control_a.1, control_b.1)
                } else if direction_a.1 != 0.0 && direction_b.1 != 0.0 {
                    let y = (control_a.1 + control_b.1) / 2.0;
                    format!("L{} {y} L{} {y}", control_a.0, control_b.0)
                } else {
                    format!("L{} {}", control_b.0, control_a.1)
                };
                format!(
                    "M{} {} L{} {} {mid} L{} {} L{} {}",
                    a.0, a.1, control_a.0, control_a.1, control_b.0, control_b.1, b.0, b.1
                )
            } else {
                format!(
                    "M{} {} C{} {} {} {} {} {}",
                    a.0, a.1, control_a.0, control_a.1, control_b.0, control_b.1, b.0, b.1
                )
            };
            let stroke = color(&edge.color);
            let marker = format!("arrow-{scope}-{ordinal}");
            output.push_str(&format!(r#"<defs><marker id="{marker}" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 z" fill="{stroke}"/></marker></defs>"#));
            let start = if edge.from_end == "arrow" {
                format!(r#" marker-start="url(#{marker})""#)
            } else {
                String::new()
            };
            let end = if edge.to_end == "arrow" {
                format!(r#" marker-end="url(#{marker})""#)
            } else {
                String::new()
            };
            let dash = if edge.style_attributes.path == "short-dashed" {
                r#" stroke-dasharray="5 5""#
            } else {
                ""
            };
            output.push_str(&format!(r#"<path data-edge-id="{}" d="{path}" fill="none" stroke="{stroke}" stroke-width="2"{start}{end}{dash}/>"#, escape_html(&edge.id)));
            if !edge.label.is_empty() {
                let x = (a.0 + b.0) / 2.0;
                let y = (a.1 + b.1) / 2.0;
                output.push_str(&format!(r#"<text x="{x}" y="{y}" text-anchor="middle" class="font-mono text-sm stroke-ink [paint-order:stroke] stroke-[5px] [stroke-linejoin:round]" fill="{stroke}">"#));
                for (line, text) in edge.label.lines().enumerate() {
                    let dy = if line == 0 { -8 } else { 18 };
                    output.push_str(&format!(
                        r#"<tspan x="{x}" dy="{dy}">{}</tspan>"#,
                        escape_html(text)
                    ));
                }
                output.push_str("</text>");
            }
        }
        output.push_str("</svg>");
        output
    }
}

fn endpoint(node: &Node, other: &Node, side: &str) -> ((f64, f64), (f64, f64)) {
    let side = match side {
        "top" | "right" | "bottom" | "left" => side,
        _ => {
            let dx = other.x + other.width / 2.0 - node.x - node.width / 2.0;
            let dy = other.y + other.height / 2.0 - node.y - node.height / 2.0;
            if dx.abs() > dy.abs() {
                if dx > 0.0 { "right" } else { "left" }
            } else if dy > 0.0 {
                "bottom"
            } else {
                "top"
            }
        }
    };
    match side {
        "top" => ((node.x + node.width / 2.0, node.y), (0.0, -1.0)),
        "bottom" => (
            (node.x + node.width / 2.0, node.y + node.height),
            (0.0, 1.0),
        ),
        "left" => ((node.x, node.y + node.height / 2.0), (-1.0, 0.0)),
        _ => (
            (node.x + node.width, node.y + node.height / 2.0),
            (1.0, 0.0),
        ),
    }
}

pub fn render(file: &CanvasFile, index: &FileIndex) -> Result<(), FileRenderError> {
    let scene = scene(file, index, &mut Vec::new(), "root");
    let bounds = scene.bounds();
    let mut context = Context::new();
    context.insert("title", &file.title);
    context.insert("section", "pages");
    context.insert("output_dir", &*OUTPUT_DIR);
    context.insert("diagram", &scene.diagram("root"));
    context.insert(
        "bounds",
        &format!("{} {} {} {}", bounds.0, bounds.1, bounds.2, bounds.3),
    );
    context.insert("node_count", &scene.cards.len());
    let html = TERA_ENGINE.render("canvas.html", &context)?;
    if let Some(parent) = Path::new(&file.output_path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&file.output_path, html).map_err(FileRenderError::Write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::markdown_file::{Frontmatter, MarkdownFile};
    use serde_json::{Value, json};

    fn file_node(id: &str, file: &str) -> Value {
        json!({"id":id,"type":"file","file":file,"x":0,"y":0,"width":400,"height":400})
    }

    fn text_node(id: &str, text: &str) -> Value {
        json!({"id":id,"type":"text","text":text,"x":-500,"y":-200,"width":400,"height":400})
    }

    fn canvas(path: &str, nodes: Vec<Value>, edges: Vec<Value>) -> CanvasFile {
        CanvasFile {
            title: path.into(),
            relative_path: path.into(),
            output_path: format!("output/{path}.html"),
            canvas: serde_json::from_value(json!({"nodes":nodes,"edges":edges})).unwrap(),
        }
    }

    fn index() -> FileIndex {
        let mut index = FileIndex {
            root_dir: "/vault".into(),
            ..Default::default()
        };
        for folder in ["NPCs", "Other"] {
            index.markdown_files.push(MarkdownFile {
                title: "Pinewood".into(),
                origin_path: format!("/vault/{folder}/Pinewood.md"),
                output_path: format!("output/{folder}/Pinewood.html"),
                content: format!("Visible {folder} note"),
                frontmatter: Frontmatter {
                    publish: Some(true),
                    tags: None,
                    image: None,
                },
                images: vec![],
                properties: vec![],
            });
            index.link_summaries.push(LinkSummary {
                title: "Pinewood".into(),
                output_path: format!("output/{folder}/Pinewood.html"),
            });
        }
        index
    }

    #[test]
    fn removes_unpublished_missing_and_external_cards_and_all_incident_edges() {
        let mut index = index();
        index.canvas_files.push(canvas("Parent.canvas", vec![
            file_node("public", "NPCs/Pinewood.md"), file_node("private-note", "NPCs/Secret.md"),
            file_node("private-canvas", "Private.canvas"), text_node("text", "Public text"),
            json!({"id":"web","type":"link","url":"https://example.com","x":0,"y":0,"width":100,"height":100}),
        ], vec![
            json!({"id":"ok","fromNode":"public","toNode":"text"}),
            json!({"id":"secret-source","fromNode":"private-note","toNode":"public"}),
            json!({"id":"secret-target","fromNode":"text","toNode":"private-canvas"}),
            json!({"id":"missing","fromNode":"public","toNode":"absent"}),
            json!({"id":"external","fromNode":"web","toNode":"text"}),
        ]));
        prepare(&mut index);
        let scene = scene(&index.canvas_files[0], &index, &mut vec![], "test");
        assert_eq!(scene.cards.len(), 2);
        assert_eq!(scene.edges.len(), 1);
        let html = scene.diagram("test");
        for forbidden in [
            "Secret",
            "Private",
            "example.com",
            "secret-source",
            "secret-target",
            "external",
            "absent",
        ] {
            assert!(!html.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn nested_canvases_filter_recursively_and_cycles_become_links() {
        let mut index = index();
        index.canvas_files = vec![
            canvas(
                "A.canvas",
                vec![
                    file_node("child", "B.canvas"),
                    file_node("again", "B.canvas"),
                ],
                vec![],
            ),
            canvas(
                "B.canvas",
                vec![
                    file_node("next", "C.canvas"),
                    file_node("hidden", "Secret.canvas"),
                ],
                vec![json!({"id":"hidden-edge","fromNode":"next","toNode":"hidden"})],
            ),
            canvas(
                "C.canvas",
                vec![
                    file_node("cycle", "A.canvas"),
                    text_node("content", "Third level"),
                ],
                vec![],
            ),
        ];
        prepare(&mut index);
        let scene = scene(&index.canvas_files[0], &index, &mut vec![], "test");
        let html = scene.diagram("test");
        assert_eq!(html.matches("Third level").count(), 2);
        assert_eq!(html.matches("Recursive canvas reference").count(), 2);
        assert!(html.contains("Open canvas"));
        assert!(!html.contains("Secret.canvas"));
        assert!(!html.contains("hidden-edge"));
    }

    #[test]
    fn resolves_exact_paths_and_rejects_ambiguous_stems_and_vault_escapes() {
        let index = index();
        assert_eq!(resolve_page(&index, "NPCs/Pinewood.md"), Some(0));
        assert_eq!(resolve_page(&index, "Other/Pinewood"), Some(1));
        assert_eq!(resolve_page(&index, "Pinewood"), None);
        assert_eq!(resolve_page(&index, "../NPCs/Pinewood.md"), None);
        assert_eq!(resolve_page(&index, "/NPCs/Pinewood.md"), None);
    }

    #[test]
    fn renders_square_dashed_multiline_edges_and_arrow_defaults() {
        let mut index = index();
        index.canvas_files.push(canvas("Family.canvas", vec![text_node("a", "A"), file_node("b", "NPCs/Pinewood.md")], vec![
            json!({"id":"married","fromNode":"a","fromSide":"right","toNode":"b","toSide":"left","toEnd":"none","label":"Part of\nHomestead","styleAttributes":{"pathfindingMethod":"square","path":"short-dashed"}}),
            json!({"id":"default","fromNode":"b","toNode":"a"}),
        ]));
        prepare(&mut index);
        let scene = scene(&index.canvas_files[0], &index, &mut vec![], "test");
        let html = scene.diagram("test");
        assert!(html.contains("stroke-dasharray=\"5 5\""));
        assert!(html.contains("<tspan"));
        assert!(html.contains("Homestead"));
        assert!(!html.contains("marker-end=\"url(#arrow-test-0)\""));
        assert!(html.contains("marker-end=\"url(#arrow-test-1)\""));
    }

    #[test]
    fn text_is_escaped_and_unsafe_html_and_links_cannot_execute() {
        let index = index();
        let html = render_markdown(
            "<script>alert(1)</script>\n\n[bad](javascript:alert)\n\n![missing](Secret.png)",
            &index,
        )
        .0;
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("</a>"));
        assert!(!html.contains("<img"));
        assert!(html.contains("missing"));
        assert_eq!(color("red; background:url(evil)"), "#a8a29e");
    }

    #[test]
    fn retains_canvas_only_images_and_group_backgrounds_by_exact_path() {
        let mut index = index();
        for path in [
            "Images/portrait.png",
            "Images/background.avif",
            "Other/portrait.png",
        ] {
            index.image_files.push(ImageFileLink {
                title: Path::new(path)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .into(),
                original_path: format!("/vault/{path}"),
                link: format!("output/{path}"),
                extension: "png".into(),
            });
        }
        index.canvas_files.push(canvas("Images.canvas", vec![
            file_node("image", "Images/portrait.png"),
            json!({"id":"group","type":"group","background":"Images/background.avif","backgroundStyle":"repeat","label":"People","x":0,"y":0,"width":800,"height":600}),
        ], vec![]));
        prepare(&mut index);
        index.remove_unpublished_images();
        assert_eq!(index.image_files.len(), 2);
        assert!(resolve_image(&index, "Other/portrait.png").is_none());
        let scene = scene(&index.canvas_files[0], &index, &mut vec![], "test");
        let html = scene.diagram("test");
        assert!(html.contains("patternUnits=\"userSpaceOnUse\""));
        assert!(html.contains("Images/portrait.png"));
    }
}
