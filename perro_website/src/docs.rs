use pulldown_cmark::{html, Options, Parser};
use serde::Deserialize;
use std::sync::LazyLock;

include!(concat!(env!("OUT_DIR"), "/generated_docs.rs"));

#[derive(Clone, Deserialize)]
pub struct DocPage {
    pub collection: String,
    pub slug: String,
    pub route_path: String,
    pub title: String,
    pub area: String,
    pub summary: String,
    pub headings: Vec<DocHeading>,
    pub keywords: String,
    pub markdown: String,
    pub html: String,
    pub search_text: String,
}

#[derive(Clone, Deserialize)]
pub struct DocHeading {
    pub level: u8,
    pub text: String,
    pub id: String,
}

static DOCS: LazyLock<Vec<DocPage>> =
    LazyLock::new(|| serde_json::from_str(DOCS_JSON).expect("generated docs json"));

pub fn docs() -> &'static [DocPage] {
    DOCS.as_slice()
}

pub fn docs_pages() -> Vec<&'static DocPage> {
    docs()
        .iter()
        .filter(|doc| doc.collection == "docs")
        .collect()
}

pub fn book_pages() -> Vec<&'static DocPage> {
    let mut pages = docs()
        .iter()
        .filter(|doc| doc.collection == "book")
        .collect::<Vec<_>>();
    pages.sort_by_key(|doc| book_rank(&doc.slug));
    pages
}

fn book_rank(slug: &str) -> usize {
    match slug {
        "index" => 0,
        "install" => 1,
        "first_project" => 2,
        "scenes_nodes" => 3,
        "scripting_model" => 4,
        "rust_scripting" => 5,
        "runtime_nodes" => 6,
        "generated_script_glue" => 7,
        "input" => 8,
        "assets_resources" => 9,
        "ui_animation_audio" => 10,
        "physics_queries" => 11,
        "demos_web" => 12,
        "performance_release" => 13,
        "api_map" => 14,
        _ => usize::MAX,
    }
}

pub fn find_doc(collection: &str, slug: &str) -> Option<&'static DocPage> {
    let normalized = slug.trim_matches('/');
    docs()
        .iter()
        .find(|doc| doc.collection == collection && doc.slug == normalized)
}

pub fn docs_by_area() -> Vec<(&'static str, usize)> {
    let mut out = Vec::<(&'static str, usize)>::new();
    for doc in docs().iter().filter(|doc| doc.collection == "docs") {
        if let Some((area, count)) = out.last_mut().filter(|(area, _)| *area == doc.area) {
            let _ = area;
            *count += 1;
        } else {
            out.push((doc.area.as_str(), 1));
        }
    }
    out
}

pub fn grouped_docs() -> Vec<(&'static str, Vec<&'static DocPage>)> {
    let mut out = Vec::<(&'static str, Vec<&'static DocPage>)>::new();
    for doc in docs().iter().filter(|doc| doc.collection == "docs") {
        if let Some((area, area_docs)) = out.last_mut().filter(|(area, _)| *area == doc.area) {
            let _ = area;
            area_docs.push(doc);
        } else {
            out.push((doc.area.as_str(), vec![doc]));
        }
    }
    out
}

pub fn grouped_docs_filtered(query: &str) -> Vec<(&'static str, Vec<&'static DocPage>)> {
    grouped_docs_filtered_for_area(query, None)
}

pub fn grouped_docs_filtered_for_area(
    query: &str,
    area: Option<&str>,
) -> Vec<(&'static str, Vec<&'static DocPage>)> {
    let needle = query.trim().to_ascii_lowercase();
    let area = area.map(str::trim).filter(|area| !area.is_empty());
    if needle.is_empty() && area.is_none() {
        return grouped_docs();
    }

    let mut out = Vec::<(&'static str, Vec<&'static DocPage>)>::new();
    for doc in docs()
        .iter()
        .filter(|doc| doc.collection == "docs")
        .filter(|doc| area.is_none_or(|area| doc.area == area))
        .filter(|doc| needle.is_empty() || doc_matches(doc, &needle))
    {
        if let Some((area, area_docs)) = out.last_mut().filter(|(area, _)| *area == doc.area) {
            let _ = area;
            area_docs.push(doc);
        } else {
            out.push((doc.area.as_str(), vec![doc]));
        }
    }
    out
}

pub fn markdown_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn doc_matches(doc: &DocPage, needle: &str) -> bool {
    doc.search_text.contains(needle)
}

#[cfg(test)]
mod tests {
    use super::{docs, markdown_html};
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };

    #[test]
    fn renders_raw_markdown_without_hiding_examples() {
        let html = markdown_html(
            r#"Example:

```rust
let value = 1;
let _ = value;
```
"#,
        );

        assert!(html.contains("let value = 1;"));
    }

    #[test]
    fn docs_do_not_contain_stale_script_model_terms() {
        let banned = [
            "PerroScript",
            "ScriptCtx",
            "#[derive(Default, PerroScript)]",
            "derive Perro script",
            "script struct",
            "with_state!(ctx,",
            "with_state_mut!(ctx,",
            "with_state!(ctx.run, 0.0",
            "with_state_mut!(ctx.run, 0.0",
        ];

        let mut failures = Vec::new();
        for doc in docs() {
            if doc.collection != "docs" {
                continue;
            }
            for needle in banned {
                if doc.markdown.contains(needle) {
                    failures.push(format!("{} contains `{needle}`", doc.slug));
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn docs_do_not_contain_generated_placeholder_examples() {
        let mut failures = Vec::new();
        for doc in docs() {
            if doc.collection != "docs" {
                continue;
            }
            if doc.markdown.contains("let value =") && doc.markdown.contains("let _ = value;") {
                failures.push(format!(
                    "{} contains generated placeholder example",
                    doc.slug
                ));
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn api_ref_headings_match_compiled_api_names() {
        let root = workspace_root();
        let source_names = collect_source_api_names(&root);
        let mut failures = Vec::new();

        for doc in docs()
            .iter()
            .filter(|doc| doc.collection == "docs")
            .filter(|doc| doc.slug.starts_with("scripting/contexts/"))
        {
            for heading in api_ref_headings(&doc.markdown) {
                if !source_names.contains(&heading) {
                    failures.push(format!("{} has no source API `{heading}`", doc.slug));
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn api_ref_signatures_name_their_heading() {
        let mut failures = Vec::new();

        for doc in docs()
            .iter()
            .filter(|doc| doc.collection == "docs")
            .filter(|doc| doc.slug.starts_with("scripting/contexts/"))
        {
            let mut current = None::<String>;
            for line in doc.markdown.lines() {
                if let Some(name) = h3_code_name(line) {
                    current = Some(name);
                    continue;
                }
                if let (Some(name), Some(sig)) =
                    (current.as_deref(), table_detail(line, "| Signature |"))
                {
                    let macro_name = format!("{name}!");
                    if !sig.contains(name) && !sig.contains(&macro_name) {
                        failures.push(format!(
                            "{} `{name}` signature does not name API: {sig}",
                            doc.slug
                        ));
                    }
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn docs_do_not_use_generic_api_placeholder_prose() {
        let banned = [
            "Use when this exact typed operation matches the system state the script needs to read or change.",
            "Option returns None for missing data",
            "ID-based calls fail when the ID is stale",
        ];

        let mut failures = Vec::new();
        for doc in docs() {
            if doc.collection != "docs" {
                continue;
            }
            for needle in banned {
                if doc.markdown.contains(needle) {
                    failures.push(format!("{} contains placeholder prose", doc.slug));
                }
            }
        }

        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn book_and_docs_are_separate_collections() {
        assert!(docs()
            .iter()
            .any(|doc| doc.collection == "book" && doc.slug == "index"));
        assert!(docs()
            .iter()
            .any(|doc| doc.collection == "docs" && doc.slug == "index"));
        assert!(docs().iter().all(|doc| {
            if doc.collection == "book" {
                doc.route_path == "/book" || doc.route_path.starts_with("/book/")
            } else {
                doc.route_path == "/docs" || doc.route_path.starts_with("/docs/")
            }
        }));
    }

    #[test]
    fn docs_index_does_not_include_book_pages() {
        let docs_pages = super::docs_pages();
        assert!(!docs_pages.is_empty());
        assert!(docs_pages.iter().all(|doc| doc.collection == "docs"));
    }

    #[test]
    fn scripting_docs_cover_runtime_nodes_methods_and_variants() {
        let overview = super::find_doc("docs", "scripting/README").expect("scripting overview");
        for needle in [
            "custom methods",
            "runtime node access",
            "cross-script calls",
            "`Variant` conversion",
        ] {
            assert!(
                overview.markdown.contains(needle),
                "scripting overview missing `{needle}`"
            );
        }

        let methods = super::find_doc("docs", "scripting/methods").expect("methods doc");
        assert!(methods.markdown.contains("Primitive method returns"));
        assert!(methods.markdown.contains("always returns a `Variant`"));
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("website crate has workspace parent")
            .to_path_buf()
    }

    fn collect_source_api_names(root: &Path) -> BTreeSet<String> {
        let mut names = BTreeSet::new();
        for dir in [
            "perro_source/api_modules",
            "perro_source/script_stack",
            "perro_source/runtime_project/perro_runtime",
            "perro_source/runtime_project/perro_scene",
            "perro_source/core",
        ] {
            collect_source_api_names_dir(&root.join(dir), &mut names);
        }
        names
    }

    fn collect_source_api_names_dir(dir: &Path, names: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_source_api_names_dir(&path, names);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            collect_source_api_names_text(&text, names);
        }
    }

    fn collect_source_api_names_text(text: &str, names: &mut BTreeSet<String>) {
        for line in text.lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("pub fn ") {
                push_ident(rest, names);
            } else if let Some(rest) = line.strip_prefix("fn ") {
                push_ident(rest, names);
            } else if let Some(rest) = line.strip_prefix("macro_rules! ") {
                push_ident(rest, names);
            } else if let Some(rest) = line.strip_prefix("pub struct ") {
                push_ident(rest, names);
            } else if let Some(rest) = line.strip_prefix("pub enum ") {
                push_ident(rest, names);
            } else if let Some(rest) = line.strip_prefix("pub trait ") {
                push_ident(rest, names);
            }
        }
    }

    fn push_ident(rest: &str, names: &mut BTreeSet<String>) {
        let ident = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !ident.is_empty() {
            names.insert(ident);
        }
    }

    fn api_ref_headings(markdown: &str) -> Vec<String> {
        markdown.lines().filter_map(h3_code_name).collect()
    }

    fn h3_code_name(line: &str) -> Option<String> {
        let trimmed = line.trim();
        let rest = trimmed.strip_prefix("### `")?;
        let name = rest.strip_suffix('`')?;
        Some(name.trim_end_matches('!').to_string())
    }

    fn table_detail<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
        let rest = line.trim().strip_prefix(prefix)?;
        Some(rest.trim().trim_end_matches('|').trim())
    }
}
