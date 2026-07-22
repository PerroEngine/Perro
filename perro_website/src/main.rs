#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use axum::{routing::get, Router};
    use leptos::config::get_configuration;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use perro_website_lib::App;
    use tower_http::services::{ServeDir, ServeFile};

    tracing_subscriber::fmt().with_env_filter("info").init();

    let conf = get_configuration(Some("perro_website/Cargo.toml"))?;
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);
    let site_root = std::path::PathBuf::from(leptos_options.site_root.as_ref());
    let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let public_root = crate_root.join("public");
    let style_root = crate_root.join("style");

    let app = Router::new()
        .nest_service("/style", ServeDir::new(style_root))
        .nest_service("/demos", ServeDir::new(public_root.join("demos")))
        .nest_service("/tiers", ServeDir::new(public_root.join("tiers")))
        .route_service("/perro.svg", ServeFile::new(public_root.join("perro.svg")))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/og/{*path}", get(og_image))
        .leptos_routes(&leptos_options, routes, App)
        .fallback_service(ServeDir::new(site_root))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("perro_website @ http://{addr}");
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

#[cfg(feature = "ssr")]
async fn og_image(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl axum::response::IntoResponse {
    use axum::response::IntoResponse;

    let wants_png = path.ends_with(".png");
    let normalized = path
        .strip_suffix(".png")
        .or_else(|| path.strip_suffix(".svg"))
        .unwrap_or(&path);
    let (title, label, summary) = og_text(normalized);
    let svg = social_svg(&title, &label, &summary);
    if wants_png {
        return match svg_to_png(&svg) {
            Ok(png) => ([(axum::http::header::CONTENT_TYPE, "image/png")], png).into_response(),
            Err(_) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "social image render failed",
            )
                .into_response(),
        };
    }

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "image/svg+xml; charset=utf-8",
        )],
        svg,
    )
        .into_response()
}

#[cfg(feature = "ssr")]
async fn robots_txt(headers: axum::http::HeaderMap) -> impl axum::response::IntoResponse {
    let base = public_base_url(&headers);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        format!("User-agent: *\nAllow: /\nSitemap: {base}/sitemap.xml\n"),
    )
}

#[cfg(feature = "ssr")]
fn og_text(path: &str) -> (String, String, String) {
    let route = path.replace("__", "/");
    let route = if route == "home" { "" } else { route.as_str() };

    if let Some(slug) = route.strip_prefix("docs/") {
        if let Some(doc) = perro_website_lib::docs::find_doc("docs", slug) {
            let summary = if doc.summary.is_empty() {
                format!("Perro {} docs", doc.area)
            } else {
                doc.summary.clone()
            };
            return (doc.title.clone(), format!("Docs / {}", doc.area), summary);
        }
    }
    if let Some(slug) = route.strip_prefix("book/") {
        if let Some(doc) = perro_website_lib::docs::find_doc("book", slug) {
            let summary = if doc.summary.is_empty() {
                "Perro book chapter".to_string()
            } else {
                doc.summary.clone()
            };
            return (doc.title.clone(), "Book".to_string(), summary);
        }
    }

    match route {
        "" => (
            "Rust Game Engine".to_string(),
            "Perro Engine".to_string(),
            "Open-source Rust engine for docs, nodes, examples, and WASM demos.".to_string(),
        ),
        "book" => (
            "Perro Book".to_string(),
            "Book".to_string(),
            "Guided install-to-release path for Perro docs, nodes, examples, and demos."
                .to_string(),
        ),
        "nodes" => (
            "Scene Node Registry".to_string(),
            "Nodes".to_string(),
            "Search 2D, 3D, UI, physics, audio, animation, camera, and light nodes.".to_string(),
        ),
        "examples" => (
            "Examples and Demos".to_string(),
            "Examples".to_string(),
            "Run Perro 2D and 3D WebAssembly demos in the browser.".to_string(),
        ),
        "examples/2d" => (
            "Demo2D".to_string(),
            "WASM Demo".to_string(),
            "Sprites, lights, water, physics, animation, UI, and skeletal samples.".to_string(),
        ),
        "examples/3d" => (
            "Demo3D".to_string(),
            "WASM Demo".to_string(),
            "Materials, lights, water, particles, sky, physics, audio, and mesh demos.".to_string(),
        ),
        "sponsor" => (
            "Support Perro".to_string(),
            "Sponsor".to_string(),
            "Fund runtime optimization, platform support, docs, examples, demos, and tooling."
                .to_string(),
        ),
        "docs" => (
            "Perro Docs".to_string(),
            "Docs".to_string(),
            "Browse scripting, runtime API, resource API, input API, nodes, and CLI docs."
                .to_string(),
        ),
        "learn/getting-started" => (
            "Get Started".to_string(),
            "Learn".to_string(),
            "Install Perro CLI, create a Rust project, run dev, and export web builds.".to_string(),
        ),
        "features" => (
            "Perro Features".to_string(),
            "Features".to_string(),
            "Scene nodes, Rust scripts, static export, 2D, 3D, physics, animation, and web."
                .to_string(),
        ),
        "assets" => (
            "Assets and Templates".to_string(),
            "Assets".to_string(),
            "Use demo assets and templates for scenes, scripts, animation, UI, and physics."
                .to_string(),
        ),
        "community" => (
            "Build with Perro".to_string(),
            "Community".to_string(),
            "Join through GitHub issues, docs patches, demos, and small reproducible reports."
                .to_string(),
        ),
        "news" => (
            "Project Notes".to_string(),
            "News".to_string(),
            "Track docs, demos, editor work, runtime polish, and CLI progress.".to_string(),
        ),
        _ => (
            "Perro Engine".to_string(),
            "Rust Game Engine".to_string(),
            "Open-source Rust game engine with docs, nodes, examples, and demos.".to_string(),
        ),
    }
}

#[cfg(feature = "ssr")]
fn svg_to_png(svg: &str) -> Result<Vec<u8>, String> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|err| err.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(1200, 630).ok_or("could not create pixmap")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|err| err.to_string())
}

#[cfg(feature = "ssr")]
fn social_svg(title: &str, label: &str, summary: &str) -> String {
    let title_lines = svg_lines(title, 24, 3);
    let summary_lines = svg_lines(summary, 54, 2);
    let title_text = svg_text_lines(&title_lines, 86, 226, 68, 78, "title");
    let summary_text = svg_text_lines(&summary_lines, 90, 458, 30, 42, "summary");
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#10151b"/>
      <stop offset="0.54" stop-color="#18232b"/>
      <stop offset="1" stop-color="#27313a"/>
    </linearGradient>
  </defs>
  <rect width="1200" height="630" fill="url(#bg)"/>
  <rect x="48" y="48" width="1104" height="534" rx="34" fill="none" stroke="#d2ff72" stroke-width="2" opacity="0.74"/>
  <circle cx="1042" cy="138" r="70" fill="#d2ff72"/>
  <text x="1042" y="159" text-anchor="middle" font-family="Arial, Helvetica, sans-serif" font-size="58" font-weight="800" fill="#10151b">P</text>
  <text x="86" y="126" font-family="Arial, Helvetica, sans-serif" font-size="30" font-weight="800" fill="#d2ff72">PERRO ENGINE</text>
  <text x="86" y="170" font-family="Arial, Helvetica, sans-serif" font-size="24" font-weight="700" fill="#c7d3df">{}</text>
  {}
  {}
  <text x="86" y="560" font-family="Arial, Helvetica, sans-serif" font-size="24" font-weight="700" fill="#d2ff72">perroengine.com</text>
</svg>"##,
        xml_escape(label),
        title_text,
        summary_text
    )
}

#[cfg(feature = "ssr")]
fn svg_lines(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::<String>::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.len() + word.len() + 1 > max_chars {
            lines.push(current);
            current = String::new();
            if lines.len() == max_lines {
                break;
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }
    lines
}

#[cfg(feature = "ssr")]
fn svg_text_lines(
    lines: &[String],
    x: i32,
    y: i32,
    size: i32,
    line_height: i32,
    class_name: &str,
) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            format!(
                r#"<text x="{x}" y="{}" class="{class_name}" font-family="Arial, Helvetica, sans-serif" font-size="{size}" font-weight="800" fill="{}">{}</text>"#,
                y + (index as i32 * line_height),
                if class_name == "title" { "#f8fbff" } else { "#c7d3df" },
                xml_escape(line)
            )
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

#[cfg(feature = "ssr")]
async fn sitemap_xml(headers: axum::http::HeaderMap) -> impl axum::response::IntoResponse {
    let base = public_base_url(&headers);
    let mut urls = vec![
        SitemapUrl::new("", "weekly", "1.0"),
        SitemapUrl::new("features", "monthly", "0.8"),
        SitemapUrl::new("book", "weekly", "0.9"),
        SitemapUrl::new("nodes", "weekly", "0.9"),
        SitemapUrl::new("learn/getting-started", "monthly", "0.8"),
        SitemapUrl::new("docs", "weekly", "0.9"),
        SitemapUrl::new("examples", "weekly", "0.9"),
        SitemapUrl::new("examples/2d", "weekly", "0.8"),
        SitemapUrl::new("examples/3d", "weekly", "0.8"),
        SitemapUrl::new("news", "monthly", "0.5"),
        SitemapUrl::new("community", "monthly", "0.5"),
        SitemapUrl::new("assets", "monthly", "0.7"),
        SitemapUrl::new("sponsor", "monthly", "0.4"),
    ];
    urls.extend(
        perro_website_lib::docs::docs()
            .iter()
            .filter(|doc| doc.route_path != "/book" && doc.route_path != "/docs")
            .map(|doc| {
                SitemapUrl::new(
                    doc.route_path.trim_start_matches('/'),
                    "monthly",
                    doc_priority(doc),
                )
            }),
    );

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
"#,
    );
    for url in urls {
        let loc = if url.path.is_empty() {
            base.clone()
        } else {
            format!("{base}/{}", url.path)
        };
        xml.push_str("  <url>\n    <loc>");
        xml.push_str(&xml_escape(&loc));
        xml.push_str("</loc>\n    <changefreq>");
        xml.push_str(url.changefreq);
        xml.push_str("</changefreq>\n    <priority>");
        xml.push_str(url.priority);
        xml.push_str("</priority>\n  </url>\n");
    }
    xml.push_str("</urlset>\n");

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/xml; charset=utf-8",
        )],
        xml,
    )
}

#[cfg(feature = "ssr")]
struct SitemapUrl {
    path: String,
    changefreq: &'static str,
    priority: &'static str,
}

#[cfg(feature = "ssr")]
impl SitemapUrl {
    fn new(path: impl Into<String>, changefreq: &'static str, priority: &'static str) -> Self {
        Self {
            path: path.into(),
            changefreq,
            priority,
        }
    }
}

#[cfg(feature = "ssr")]
fn doc_priority(doc: &perro_website_lib::docs::DocPage) -> &'static str {
    if doc.slug.starts_with("examples/") || doc.area == "book" {
        "0.8"
    } else if doc.slug.contains("nodes") || doc.area == "scripting" {
        "0.7"
    } else {
        "0.6"
    }
}

#[cfg(feature = "ssr")]
fn public_base_url(headers: &axum::http::HeaderMap) -> String {
    if let Ok(base) = std::env::var("PERRO_SITE_URL") {
        let base = base.trim().trim_end_matches('/');
        if !base.is_empty() {
            return base.to_string();
        }
    }

    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("127.0.0.1:3000");
    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_else(|| {
            if host.starts_with("localhost") || host.starts_with("127.0.0.1") {
                "http"
            } else {
                "https"
            }
        });
    format!("{proto}://{host}")
}

#[cfg(feature = "ssr")]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(not(feature = "ssr"))]
fn main() {}
