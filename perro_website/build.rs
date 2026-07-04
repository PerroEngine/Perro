use perro_compiler::{
    compile_project_bundle, ProjectBuildOptions, ProjectBuildTarget, WebOutputDir,
};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use serde::Serialize;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[path = "src/highlight.rs"]
mod highlight;

#[derive(Serialize)]
struct DocOut {
    collection: String,
    slug: String,
    route_path: String,
    title: String,
    area: String,
    summary: String,
    headings: Vec<HeadingOut>,
    keywords: String,
    markdown: String,
    html: String,
    search_text: String,
}

#[derive(Serialize)]
struct HeadingOut {
    level: u8,
    text: String,
    id: String,
}

fn main() {
    println!("cargo:rerun-if-changed=../docs");
    println!("cargo:rerun-if-changed=../perro_book");
    println!("cargo:rerun-if-changed=../README.md");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest_dir.parent().unwrap().to_path_buf();
    build_and_sync_demo(&root, "Demo2D", "demo2d").expect("sync Demo2D web bundle");
    build_and_sync_demo(&root, "Demo3D", "demo3d").expect("sync Demo3D web bundle");

    let mut docs = Vec::new();

    collect_markdown("docs", &root.join("docs"), &root.join("docs"), &mut docs);
    collect_markdown(
        "book",
        &root.join("perro_book"),
        &root.join("perro_book"),
        &mut docs,
    );
    collect_demo_doc(
        &root,
        "examples/demo2d",
        &root.join("demos/Demo2D/docs/README.md"),
        &mut docs,
    );
    collect_demo_doc(
        &root,
        "examples/demo3d",
        &root.join("demos/Demo3D/docs/README.md"),
        &mut docs,
    );

    docs.sort_by(|a, b| a.slug.cmp(&b.slug));

    let json = serde_json::to_string(&docs).unwrap();
    let out = format!("pub const DOCS_JSON: &str = {json:?};\n");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("generated_docs.rs");
    fs::write(out_path, out).unwrap();
}

fn build_and_sync_demo(root: &Path, project_name: &str, public_name: &str) -> io::Result<()> {
    let project_root = root.join("demos").join(project_name);
    emit_rerun_for_tree(&project_root)?;

    let output_dir = project_root.join(".output").join("web");
    let public_dir = root
        .join("perro_website")
        .join("public")
        .join("demos")
        .join(public_name);
    if needs_demo_rebuild(&project_root, &output_dir, &public_dir)? {
        compile_project_bundle(
            &project_root,
            ProjectBuildOptions::new(false, false)
                .with_target(ProjectBuildTarget::Web)
                .with_web_output_dir(WebOutputDir::Build),
        )
        .map_err(|err| io::Error::other(format!("{project_name} web build failed: {err}")))?;
    }

    if bundle_complete(&output_dir) {
        sync_dir(&output_dir, &public_dir)?;
    }
    Ok(())
}

fn emit_rerun_for_tree(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "target" || name == ".output" || name == ".perro" {
            continue;
        }
        if path.is_dir() {
            emit_rerun_for_tree(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    Ok(())
}

fn bundle_complete(dir: &Path) -> bool {
    ["index.html", "boot.js", "app.js", "app_bg.wasm"]
        .iter()
        .all(|required| dir.join(required).exists())
}

fn needs_demo_rebuild(
    project_root: &Path,
    output_dir: &Path,
    public_dir: &Path,
) -> io::Result<bool> {
    // `.output` is gitignored; on a fresh checkout the committed bundle in
    // `public/` is the up-to-date reference, so no rebuild is needed there.
    let reference_dir = if bundle_complete(output_dir) {
        output_dir
    } else if bundle_complete(public_dir) {
        public_dir
    } else {
        return Ok(true);
    };

    let Some(output_time) = newest_mtime(reference_dir)? else {
        return Ok(true);
    };
    let Some(input_time) = newest_demo_input_mtime(project_root)? else {
        return Ok(false);
    };
    Ok(input_time > output_time)
}

fn newest_demo_input_mtime(project_root: &Path) -> io::Result<Option<SystemTime>> {
    let mut newest = fs::metadata(project_root.join("project.toml"))
        .and_then(|meta| meta.modified())
        .ok();
    for child in ["res", "src", "docs", "deps.toml", "routes.toml"] {
        let path = project_root.join(child);
        if let Some(time) = newest_mtime(&path)? {
            newest = Some(match newest {
                Some(current) if current >= time => current,
                _ => time,
            });
        }
    }
    Ok(newest)
}

fn newest_mtime(path: &Path) -> io::Result<Option<SystemTime>> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.modified().ok());
    }

    let mut newest = metadata.modified().ok();
    for entry in fs::read_dir(path)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name == "target" || name == ".output" {
            continue;
        }
        if let Some(time) = newest_mtime(&path)? {
            newest = Some(match newest {
                Some(current) if current >= time => current,
                _ => time,
            });
        }
    }
    Ok(newest)
}

fn sync_dir(src: &Path, dst: &Path) -> io::Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir(src, dst)
}

fn copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in fs::read_dir(src)? {
        let src_path = entry?.path();
        let dst_path = dst.join(src_path.file_name().unwrap());
        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn collect_markdown(collection: &str, root: &Path, dir: &Path, out: &mut Vec<DocOut>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown(collection, root, &path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let rel = path.strip_prefix(root).unwrap();
        let slug = slug_from_rel(rel);
        push_doc(collection, &path, slug, out);
    }
}

fn collect_demo_doc(root: &Path, slug: &str, path: &Path, out: &mut Vec<DocOut>) {
    if path.exists() {
        let _ = root;
        push_doc("docs", path, slug.to_string(), out);
    }
}

fn push_doc(collection: &str, path: &Path, slug: String, out: &mut Vec<DocOut>) {
    let raw_markdown = fs::read_to_string(path).unwrap();
    let markdown = rewrite_markdown_links(collection, &slug, &raw_markdown);
    let title = first_title(&markdown).unwrap_or_else(|| title_from_slug(&slug));
    let area = if collection == "book" {
        "book".to_string()
    } else {
        slug.split('/').next().unwrap_or("docs").to_string()
    };
    let route_path = route_path(collection, &slug);
    let summary = first_para(&markdown);
    let headings = headings(&markdown);
    let html = markdown_html(&markdown);
    let keywords = doc_keywords(&slug, &title, &area, &summary, &headings);
    let search_text = search_text(&slug, &title, &summary, &headings, &markdown);
    out.push(DocOut {
        collection: collection.to_string(),
        slug,
        route_path,
        title,
        area,
        summary,
        headings,
        keywords,
        markdown,
        html,
        search_text,
    });
}

fn route_path(collection: &str, slug: &str) -> String {
    match (collection, slug) {
        ("book", "index") => "/book".to_string(),
        ("book", slug) => format!("/book/{slug}"),
        ("docs", "index") => "/docs".to_string(),
        ("docs", slug) => format!("/docs/{slug}"),
        (_, slug) => format!("/docs/{slug}"),
    }
}

fn rewrite_markdown_links(collection: &str, slug: &str, markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find("](") {
        let (before, tail) = rest.split_at(start + 2);
        out.push_str(before);
        let Some(end) = tail[2..].find(')') else {
            out.push_str(&tail[2..]);
            return out;
        };
        let target = &tail[2..2 + end];
        out.push_str(&rewrite_link_target(collection, slug, target));
        out.push(')');
        rest = &tail[3 + end..];
    }
    out.push_str(rest);
    out
}

fn rewrite_link_target(collection: &str, slug: &str, target: &str) -> String {
    if target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with('#')
        || target.starts_with('/')
        || !target.contains(".md")
    {
        return target.to_string();
    }

    let (path_part, suffix) = match target.find('#') {
        Some(idx) => (&target[..idx], &target[idx..]),
        None => (target, ""),
    };
    let Some(stripped) = path_part.strip_suffix(".md") else {
        return target.to_string();
    };

    let mut parts = if collection == "book" && stripped.starts_with("../") {
        Vec::new()
    } else {
        slug.split('/')
            .take(slug.split('/').count().saturating_sub(1))
            .collect()
    };
    for part in stripped.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                let _ = parts.pop();
            }
            part => parts.push(part),
        }
    }

    if matches!(parts.last(), Some(&"index")) {
        let _ = parts.pop();
    }
    let next_slug = parts.join("/");
    let next_collection = if collection == "book" && stripped.starts_with("../") {
        "docs"
    } else {
        collection
    };
    let route = if next_slug.is_empty() {
        route_path(next_collection, "index")
    } else {
        route_path(next_collection, &next_slug)
    };
    format!("{route}{suffix}")
}

fn slug_from_rel(rel: &Path) -> String {
    let mut parts: Vec<String> = rel
        .iter()
        .map(|part| part.to_string_lossy().replace('\\', "/"))
        .collect();
    if let Some(last) = parts.last_mut() {
        if last == "index.md" {
            *last = "index".to_string();
        } else if let Some(stripped) = last.strip_suffix(".md") {
            *last = stripped.to_string();
        }
    }
    parts.join("/")
}

fn first_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(|s| s.trim().to_string()))
}

fn first_para(markdown: &str) -> String {
    markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with('|'))
        .unwrap_or("")
        .chars()
        .take(180)
        .collect()
}

fn headings(markdown: &str) -> Vec<HeadingOut> {
    let mut out = Vec::new();
    let mut in_heading = None::<u8>;
    let mut text = String::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = Some(level_num(level));
                text.clear();
            }
            Event::Text(t) | Event::Code(t) if in_heading.is_some() => text.push_str(&t),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(level) = in_heading.take() {
                    let id = anchor_id(&text);
                    out.push(HeadingOut {
                        level,
                        text: text.trim().to_string(),
                        id,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn markdown_html(markdown: &str) -> String {
    highlight::markdown_html(markdown)
}

fn doc_keywords(
    slug: &str,
    title: &str,
    area: &str,
    summary: &str,
    headings: &[HeadingOut],
) -> String {
    let mut terms = vec![
        "Perro docs".to_string(),
        "Perro examples".to_string(),
        "Perro nodes".to_string(),
        "Perro API".to_string(),
        format!("Perro {area}"),
        title.to_string(),
    ];
    terms.extend(
        slug.split('/')
            .filter(|part| !part.is_empty())
            .map(|part| format!("Perro {}", part.replace(['_', '-'], " "))),
    );
    terms.extend(
        headings
            .iter()
            .take(8)
            .map(|heading| heading.text.clone())
            .filter(|heading| !heading.is_empty()),
    );
    if summary.to_ascii_lowercase().contains("node") {
        terms.push("scene nodes".to_string());
        terms.push("node scripting".to_string());
    }
    dedupe_join(terms)
}

fn dedupe_join(terms: Vec<String>) -> String {
    let mut out = Vec::<String>::new();
    for term in terms {
        let term = term.trim();
        if term.is_empty() {
            continue;
        }
        if !out
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(term))
        {
            out.push(term.to_string());
        }
    }
    out.join(", ")
}

fn search_text(
    slug: &str,
    title: &str,
    summary: &str,
    headings: &[HeadingOut],
    markdown: &str,
) -> String {
    let mut out = String::with_capacity(
        slug.len() + title.len() + summary.len() + markdown.len().min(4096) + 64,
    );
    out.push_str(slug);
    out.push(' ');
    out.push_str(title);
    out.push(' ');
    out.push_str(summary);
    for heading in headings {
        out.push(' ');
        out.push_str(&heading.text);
    }
    out.push(' ');
    out.extend(markdown.chars().take(4096));
    out.make_ascii_lowercase();
    out
}

fn level_num(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn anchor_id(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if (ch.is_whitespace() || ch == '-') && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn title_from_slug(slug: &str) -> String {
    slug.rsplit('/')
        .next()
        .unwrap_or(slug)
        .replace(['_', '-'], " ")
}
