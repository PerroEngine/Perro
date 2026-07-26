use leptos::prelude::*;
use leptos_router::hooks::{use_params_map, use_query_map};

use crate::docs::{
    book_pages, docs_by_nav_group, docs_nav_neighbors, docs_pages, find_doc,
    grouped_docs_filtered_for_nav,
};
use crate::layout::{NotFound, PageFrame};
use crate::shared::{Seo, SeoInfo};

#[component]
pub fn DocsIndexPage() -> impl IntoView {
    let groups = docs_by_nav_group();
    let doc_count = groups.iter().map(|(_, count)| count).sum::<usize>();
    let query_map = use_query_map();
    let selected_group = move || query_map.with(|map| map.get("group"));
    let query = RwSignal::new(String::new());
    view! {
        <Seo info=SeoInfo::new(
            "Docs",
            "Browse Perro documentation for scripting, runtime APIs, resource APIs, input APIs, scene nodes, assets, examples, and CLI workflows.",
            "Perro docs, Perro API reference, Perro scripting docs, Perro nodes, Perro examples, Perro CLI docs, resource API, input API",
            "/docs",
        ) />
        <PageFrame eyebrow="Docs" title="Perro Documentation">
            <div class="doc-layout">
                <aside class="doc-filter">
                    <h2>"Explore"</h2>
                    <a href="/docs"><span>"All"</span><small>{doc_count}</small></a>
                    {groups.into_iter().map(|(group, count)| {
                        let href = format!("/docs?group={group}");
                        view! {
                        <a href=href><span>{group}</span><small>{count}</small></a>
                    }}).collect_view()}
                </aside>
                <div class="doc-results">
                    <div class="doc-intro">
                        <div>
                            <p class="eyebrow">"Start Here"</p>
                            <h2>"Book first, docs when you need detail"</h2>
                            <p>"Use the book for the linear path. Use docs for focused API, runtime, asset, and tool reference."</p>
                        </div>
                        <a class="btn primary" href="/book">"Open Book"</a>
                    </div>
                    <input
                        class="search"
                        id="docs-search"
                        type="search"
                        placeholder="Search every guide, API, and system…  /"
                        aria-label="Search documentation"
                        on:input=move |ev| query.set(event_target_value(&ev))
                    />
                    {move || grouped_docs_filtered_for_nav(&query.get(), selected_group().as_deref()).into_iter().map(|(group, docs)| view! {
                        <section class="docs-section" id=format!("group-{}", group.to_ascii_lowercase())>
                            <div class="docs-section-head">
                                <h2>{group}</h2>
                                <span>{docs.len()}" pages"</span>
                            </div>
                            <div class="doc-list">
                                {docs.into_iter().map(|doc| view! {
                                    <a class="doc-row" href=doc.route_path.as_str()>
                                        <span class="doc-row-main">
                                            <strong>{doc.title.as_str()}</strong>
                                            <span>{doc.summary.as_str()}</span>
                                        </span>
                                        <span class="doc-row-meta">{doc.nav_group.as_str()}</span>
                                    </a>
                                }).collect_view()}
                            </div>
                        </section>
                    }).collect_view()}
                </div>
            </div>
        </PageFrame>
    }
}

#[component]
pub fn BookPage() -> impl IntoView {
    view! {
        {move || match find_doc("book", "index") {
            Some(doc) => view! {
                <BookArticle doc=doc />
            }.into_any(),
            None => view! { <NotFound /> }.into_any(),
        }}
    }
}

#[component]
pub fn DocPageView() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();
    let doc = move || find_doc("docs", &slug());

    view! {
        {move || match doc() {
            Some(doc) => view! {
                <DocArticle doc=doc canonical_path=doc.route_path.clone() />
            }.into_any(),
            None => view! { <NotFound /> }.into_any(),
        }}
    }
}

#[component]
pub fn BookChapterPage() -> impl IntoView {
    let params = use_params_map();
    let slug = move || params.read().get("slug").unwrap_or_default();
    let doc = move || find_doc("book", &slug());

    view! {
        {move || match doc() {
            Some(doc) => view! {
                <BookArticle doc=doc />
            }.into_any(),
            None => view! { <NotFound /> }.into_any(),
        }}
    }
}

#[component]
fn BookArticle(doc: &'static crate::docs::DocPage) -> impl IntoView {
    let chapters = book_pages()
        .into_iter()
        .filter(|page| page.slug != "index")
        .collect::<Vec<_>>();
    let pos = chapters.iter().position(|page| page.slug == doc.slug);
    let prev = pos
        .and_then(|idx| idx.checked_sub(1))
        .and_then(|idx| chapters.get(idx).copied());
    let next = pos.and_then(|idx| chapters.get(idx + 1).copied());

    view! {
        <Seo info=doc_seo(doc, doc.route_path.as_str()) />
        <PageFrame eyebrow="Book" title=doc.title.as_str()>
            <div class="doc-layout">
                <aside class="doc-filter">
                    <h2>"Chapters"</h2>
                    <a href="/book"><span>"Start"</span><small>"0"</small></a>
                    {chapters.into_iter().enumerate().map(|(idx, chapter)| view! {
                        <a href=chapter.route_path.as_str()>
                            <span>{chapter.title.as_str()}</span>
                            <small>{idx + 1}</small>
                        </a>
                    }).collect_view()}
                </aside>
                <div class="doc-results">
                    <div class="doc-page">
                        <article class="article" inner_html=doc.html.as_str()></article>
                        <aside class="toc">
                            <strong>"On page"</strong>
                            {doc.headings.iter().filter(|h| (2..=3).contains(&h.level)).map(|h| view! {
                                <a href=format!("#{}", h.id)>{h.text.as_str()}</a>
                            }).collect_view()}
                        </aside>
                    </div>
                    <nav class="page-actions">
                        {prev.map(|page| view! {
                            <a class="btn ghost" href=page.route_path.as_str()>{format!("Previous: {}", page.title)}</a>
                        })}
                        {next.map(|page| view! {
                            <a class="btn primary" href=page.route_path.as_str()>{format!("Next: {}", page.title)}</a>
                        })}
                    </nav>
                </div>
            </div>
        </PageFrame>
    }
}

#[component]
fn DocArticle(doc: &'static crate::docs::DocPage, canonical_path: String) -> impl IntoView {
    let groups = docs_by_nav_group();
    let group_pages = grouped_docs_filtered_for_nav("", Some(doc.nav_group.as_str()))
        .into_iter()
        .flat_map(|(_, pages)| pages)
        .collect::<Vec<_>>();
    let (prev, next) = docs_nav_neighbors(doc);
    view! {
        <Seo info=doc_seo(doc, &canonical_path) />
        <PageFrame eyebrow=doc.nav_group.as_str() title=doc.title.as_str()>
            <div class="doc-layout">
                <aside class="doc-filter">
                    <h2>"Explore"</h2>
                    <a href="/docs"><span>"All"</span><small>{docs_pages().len()}</small></a>
                    {groups.into_iter().map(|(group, count)| {
                        let href = format!("/docs?group={group}");
                        view! {
                            <a href=href><span>{group}</span><small>{count}</small></a>
                        }
                    }).collect_view()}
                    <div class="doc-filter-pages">
                        <strong>{doc.nav_group.as_str()}</strong>
                        {group_pages.into_iter().map(|page| {
                            let class_name = if page.slug == doc.slug {
                                "current"
                            } else {
                                ""
                            };
                            let current = (page.slug == doc.slug).then_some("page");
                            view! {
                                <a class=class_name aria-current=current href=page.route_path.as_str()>
                                    <span>{page.title.as_str()}</span>
                                </a>
                            }
                        }).collect_view()}
                    </div>
                </aside>
                <div class="doc-results">
                    <div class="doc-page">
                        <article class="article" inner_html=doc.html.as_str()></article>
                        <aside class="toc">
                            <strong>"On page"</strong>
                            {doc.headings.iter().filter(|h| (2..=3).contains(&h.level)).map(|h| view! {
                                <a href=format!("#{}", h.id)>{h.text.as_str()}</a>
                            }).collect_view()}
                        </aside>
                    </div>
                    <nav class="page-actions doc-neighbors" aria-label="Documentation pages">
                        {prev.map(|page| view! {
                            <a class="btn ghost" href=page.route_path.as_str()>{format!("Previous: {}", page.title)}</a>
                        })}
                        {next.map(|page| view! {
                            <a class="btn primary" href=page.route_path.as_str()>{format!("Next: {}", page.title)}</a>
                        })}
                    </nav>
                </div>
            </div>
        </PageFrame>
    }
}

fn doc_seo(doc: &crate::docs::DocPage, canonical_path: &str) -> SeoInfo {
    let description = if doc.summary.is_empty() {
        format!(
            "Perro documentation for {} in the {} area, with Rust game engine API notes and examples.",
            doc.title, doc.area
        )
    } else {
        doc.summary.clone()
    };
    SeoInfo::new(&doc.title, &description, &doc.keywords, canonical_path).with_schema(format!(
        r#"{{
  "@context": "https://schema.org",
  "@type": "TechArticle",
  "headline": {},
  "description": {},
  "about": {},
  "programmingLanguage": "Rust"
}}"#,
        json_string(&doc.title),
        json_string(&description),
        json_string(&doc.area),
    ))
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"Perro\"".to_string())
}
