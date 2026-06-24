use leptos::prelude::*;
use leptos_meta::{Link, Meta, Script, Title};

use crate::highlight;

pub const SITE_NAME: &str = "Perro Engine";
pub const BASE_KEYWORDS: &str = "Perro Engine, Rust game engine, open source game engine, WASM game engine, 2D game engine, 3D game engine";

pub struct SeoInfo {
    pub title: String,
    pub description: String,
    pub keywords: String,
    pub path: String,
    pub image: String,
    pub schema: Option<String>,
}

impl SeoInfo {
    pub fn new(title: &str, description: &str, keywords: &str, path: &str) -> Self {
        Self {
            title: title.to_string(),
            description: description.to_string(),
            keywords: join_keywords(keywords),
            path: path.to_string(),
            image: social_image_path(path),
            schema: None,
        }
    }

    pub fn with_schema(mut self, schema: String) -> Self {
        self.schema = Some(schema);
        self
    }
}

#[component]
pub fn Seo(info: SeoInfo) -> impl IntoView {
    let full_title = format!("{} | {SITE_NAME}", info.title);
    let url = absolute_url(&info.path);
    let image = absolute_url(&info.image);
    let schema = info.schema;

    view! {
        <Title text=full_title.clone() />
        <Meta name="description" content=info.description.clone() />
        <Meta name="keywords" content=info.keywords />
        <Meta name="robots" content="index, follow" />
        <Meta property="og:type" content="website" />
        <Meta property="og:site_name" content=SITE_NAME />
        <Meta property="og:title" content=full_title.clone() />
        <Meta property="og:description" content=info.description.clone() />
        <Meta property="og:image" content=image.clone() />
        <Meta property="og:image:type" content="image/png" />
        <Meta property="og:image:width" content="1200" />
        <Meta property="og:image:height" content="630" />
        <Meta property="og:image:alt" content=SITE_NAME />
        <Meta property="og:url" content=url.clone() />
        <Meta name="twitter:card" content="summary_large_image" />
        <Meta name="twitter:image" content=image />
        <Meta name="twitter:image:alt" content=SITE_NAME />
        <Meta name="twitter:title" content=full_title />
        <Meta name="twitter:description" content=info.description />
        <Link rel="canonical" href=url />
        {schema.map(|json| view! {
            <Script type_="application/ld+json">{json}</Script>
        })}
    }
}

fn join_keywords(keywords: &str) -> String {
    if keywords.trim().is_empty() {
        return BASE_KEYWORDS.to_string();
    }
    format!("{BASE_KEYWORDS}, {keywords}")
}

fn social_image_path(path: &str) -> String {
    let slug = path.trim_matches('/').replace('/', "__");
    if slug.is_empty() {
        "/og/home.svg".to_string()
    } else {
        format!("/og/{slug}.png")
    }
}

fn absolute_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }

    let base = site_base_url();
    if base.is_empty() {
        return path.to_string();
    }
    format!("{base}/{}", path.trim_start_matches('/'))
}

fn site_base_url() -> String {
    #[cfg(feature = "ssr")]
    {
        std::env::var("PERRO_SITE_URL")
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "ssr"))]
    {
        String::new()
    }
}

#[component]
pub fn CodeBlock(code: &'static str) -> impl IntoView {
    let html = highlight::code_block_html("text", code);
    view! {
        <div inner_html=html></div>
    }
}

#[component]
pub fn DemoCard(name: &'static str, body: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <a class="demo-card" href=href>
            <span>{name}</span>
            <p>{body}</p>
        </a>
    }
}
