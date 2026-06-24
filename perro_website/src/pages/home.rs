use leptos::prelude::*;

use crate::layout::PageFrame;
use crate::shared::{CodeBlock, Seo, SeoInfo};

#[component]
pub fn HomePage() -> impl IntoView {
    let features = [
        (
            "Simple to learn",
            "Start with scenes, nodes, and Rust scripts without large registration steps or boilerplate.",
            "scene -> node -> script",
            "Nodes and scripts stay small. Project setup stays small.",
            "Learn",
            "learn",
        ),
        (
            "Fast in release",
            "Nodes, scripts, and resources use release paths built for quick access and short loads.",
            "bake -> pack -> load",
            "Supported assets bake ahead of runtime. Runtime keeps lean data.",
            "Performance",
            "perf",
        ),
        (
            "2D + 3D",
            "Sprites, meshes, lights, UI, water, physics, animation, audio, and particles share one engine shape.",
            "2D + 3D + UI",
            "One project can target native desktop and browser builds.",
            "Render",
            "render",
        ),
        (
            "Rust scripts",
            "Write lifecycle hooks and methods as Rust files with per-node state.",
            "#[State]\nfn update()",
            "Script behavior and stored state stay separate and clear.",
            "Script",
            "script",
        ),
        (
            "Compiler-managed workflow",
            "Let Perro sync scripts, generate glue, and prepare supported assets.",
            "perro dev\nperro build",
            "Author normal files. Export turns them into runtime-ready data.",
            "Tooling",
            "tool",
        ),
        (
            "Free and open source",
            "Apache 2.0 licensed game engine work built in the open.",
            "Apache-2.0",
            "No license fee. No contract. No sales cut.",
            "Open",
            "open",
        ),
    ];

    view! {
        <Seo info=SeoInfo::new(
            "Rust Game Engine",
            "Perro is an experimental, open-source game engine written in Rust. With a focus on performance and simplicity without sacrificing either.",
            "Rust game engine docs, Rust game engine examples, scene nodes, WASM demos, Perro CLI, 2D game engine, 3D game engine",
            "/",
        ).with_schema(software_schema()) />
        <main class="home-page">
            <section class="hero home-hero">
                <div class="hero-copy">
                    <img class="hero-logo" src="/perro.svg" alt="Perro Engine" />
                    <p class="tagline">"An experimental, open-source game engine written in Rust. With a focus on performance and simplicity without sacrificing either."</p>
                    <p class="open-source-line">"Free and Open Source"</p>
                    <div class="hero-actions">
                        <a class="btn primary" href="/learn/getting-started">"Get Started"</a>
                        <a class="btn ghost" href="https://github.com/PerroEngine/Perro" target="_blank" rel="noreferrer">"GitHub"</a>
                    </div>
                </div>
            </section>

            <section class="home-features" aria-label="Perro features">
                {features.into_iter().map(|(title, body, code, note, meta, kind)| view! {
                    <HomeFeature title body code note meta kind />
                }).collect_view()}
            </section>

            <section class="band home-start">
                <p class="eyebrow">"Start"</p>
                <h2>"Ready to build Perro apps?"</h2>
                <p>"Install the CLI, create a project, run dev builds, and export native or web releases."</p>
                <div class="quick-steps" aria-label="Quick start path">
                    <a href="/learn/getting-started"><span>"01"</span><strong>"Install CLI"</strong></a>
                    <a href="/book/first_project"><span>"02"</span><strong>"Make first scene"</strong></a>
                    <a href="/examples"><span>"03"</span><strong>"Run demos"</strong></a>
                </div>
            </section>
        </main>
    }
}

#[component]
fn HomeFeature(
    title: &'static str,
    body: &'static str,
    code: &'static str,
    note: &'static str,
    meta: &'static str,
    kind: &'static str,
) -> impl IntoView {
    view! {
        <article class=format!("home-feature home-feature-{kind}")>
            <div class="feature-visual" aria-hidden="true">
                <div class="visual-scene">
                    <i></i>
                    <i></i>
                    <i></i>
                    <i></i>
                </div>
                <span>{meta}</span>
                <strong>{code}</strong>
            </div>
            <div class="feature-copy">
                <h2>{title}</h2>
                <p>{body}</p>
                <small>{note}</small>
            </div>
        </article>
    }
}

#[component]
pub fn FeatureGrid() -> impl IntoView {
    let features = [
        (
            "Perro Book",
            "Linear install-to-release guide for real project flow.",
            "/book",
        ),
        (
            "Scene Nodes",
            "Typed scene trees, stable IDs, inheritance-aware access, and clear runtime ownership.",
            "/docs/scripting/nodes",
        ),
        (
            "Rust Scripts",
            "Lifecycle hooks, methods, per-node state, and Variant-friendly data flow.",
            "/docs/scripting/README",
        ),
        (
            "2D + 3D",
            "Sprite, mesh, physics, lights, UI, water, animation, audio, and particles.",
            "/docs/project/feature_matrix",
        ),
        (
            "Static Export",
            "Bake supported assets ahead of time and ship browser bundles through WASM.",
            "/docs/project/performance_philosophy",
        ),
        (
            "Perro CLI",
            "Project create, script sync, dev run, build, web export, DLC, profile, and format.",
            "/docs/tools/perro_cli",
        ),
        (
            "Web Target",
            "Build browser demos with the same scene and runtime model used by native projects.",
            "/docs/WASM",
        ),
    ];

    view! {
        <section class="band">
            <div class="section-head">
                <p class="eyebrow">"Features"</p>
                <h2>"Built for simple authoring and fast release loads"</h2>
            </div>
            <div class="feature-grid">
                {features.into_iter().map(|(name, body, href)| view! {
                    <a class="feature-card" href=href>
                        <h3>{name}</h3>
                        <p>{body}</p>
                    </a>
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
pub fn FeaturesPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Features",
            "Perro features for Rust game development: scene nodes, Rust scripts, static export, CLI tools, 2D, 3D, physics, animation, audio, and web builds.",
            "Perro features, Rust scripts, static export, game engine CLI, 2D engine, 3D engine, physics, animation, audio",
            "/features",
        ) />
        <PageFrame eyebrow="Features" title="Perro capability map">
            <FeatureGrid />
        </PageFrame>
    }
}

#[component]
pub fn GetStartedPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Get Started",
            "Install Perro CLI, create a Rust game project, run dev builds, and export browser demos through the WebAssembly target.",
            "Perro getting started, Perro CLI install, Rust game project, WASM build, game engine tutorial",
            "/learn/getting-started",
        ) />
        <PageFrame eyebrow="Learn" title="Get started">
            <div class="article narrow">
                <h2>"1. Install CLI"</h2>
                <CodeBlock code="cargo run -p perro_cli -- install" />
                <h2>"2. Create project"</h2>
                <CodeBlock code=r#"cargo run -p perro_cli -- new --name MyGame --path D:\GameProjects"# />
                <h2>"3. Run dev"</h2>
                <CodeBlock code=r#"cargo run -p perro_cli -- dev --path D:\GameProjects\MyGame"# />
                <h2>"4. Build web"</h2>
                <CodeBlock code=r#"cargo run -p perro_cli -- build --path D:\GameProjects\MyGame --target web"# />
            </div>
        </PageFrame>
    }
}

fn software_schema() -> String {
    r#"{
  "@context": "https://schema.org",
  "@type": "SoftwareSourceCode",
  "name": "Perro Engine",
  "description": "Open-source Rust game engine for simple authoring, fast runtime systems, docs, examples, and browser demos.",
  "programmingLanguage": "Rust",
  "codeRepository": "https://github.com/PerroEngine/Perro",
  "license": "https://github.com/PerroEngine/Perro/blob/main/LICENSE",
  "applicationCategory": "GameDevelopmentTool"
}"#
    .to_string()
}
