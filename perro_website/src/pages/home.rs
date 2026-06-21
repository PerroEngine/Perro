use leptos::prelude::*;

use crate::docs::docs_by_area;
use crate::layout::PageFrame;
use crate::shared::{CodeBlock, DemoCard, Seo, SeoInfo};

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Rust Game Engine",
            "Perro is an open-source Rust game engine for simple authoring, fast runtime systems, scene nodes, docs, examples, and WebAssembly demos.",
            "Rust game engine docs, Rust game engine examples, scene nodes, WASM demos, Perro CLI, 2D game engine, 3D game engine",
            "/",
        ).with_schema(software_schema()) />
        <main>
            <section class="hero">
                <div class="hero-badge">"WASM demos"</div>
                <div class="hero-copy">
                    <img class="hero-logo" src="/perro.svg" alt="Perro Engine" />
                    <p class="tagline">"Experimental. Open source. Rust-first."</p>
                    <p class="lead">"A game engine focused on simple authoring, fast runtime systems, and direct control over the code that ships."</p>
                    <div class="hero-actions">
                        <a class="btn primary" href="/learn/getting-started">"Get Started"</a>
                        <a class="btn ghost" href="/book">"Read Book"</a>
                        <a class="btn ghost" href="/examples">"Run Demos"</a>
                    </div>
                </div>
                <div class="hero-note">"2D + 3D"</div>
            </section>

            <section class="band split quick-start">
                <div>
                    <p class="eyebrow">"Quick start"</p>
                    <h2>"Create, run, build"</h2>
                    <p>"Perro CLI owns script sync, dev runner builds, static asset baking, and web bundle output."</p>
                </div>
                <CodeBlock code=r#"cargo run -p perro_cli -- new --name MyGame
cargo run -p perro_cli -- dev --path D:\MyGame
cargo run -p perro_cli -- build --path D:\MyGame --target web"# />
            </section>

            <CodeExamples />
            <FeatureGrid />
            <DemoBand />
            <DocsPreview />
        </main>
    }
}

#[component]
fn CodeExamples() -> impl IntoView {
    let examples = [
        (
            "Node script",
            "Attach Rust logic to scene nodes.",
            r#"use perro_api::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct PlayerState {
    #[default(240.0)]
    #[expose]
    speed: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let speed = with_state!(ctx.run, PlayerState, ctx.id, |state| state.speed);
        let mut delta = Vector2::ZERO;

        if key_down!(ctx.ipt, KeyCode::KeyD) {
            delta.x += 1.0;
        }
        if key_down!(ctx.ipt, KeyCode::KeyA) {
            delta.x -= 1.0;
        }

        if delta.length_squared() > 0.0 {
            let step = delta.normalized() * speed * dt;
            let _ = with_base_node_mut!(ctx.run, SelfNodeType, ctx.id, |node| {
                node.transform.position += step;
            });
        }
    }
});"#,
        ),
        (
            "Scene query",
            "Find typed nodes without stringly glue.",
            r#"query_each!(ctx.run, all(tags["enemy"], tags["alive"]), |id| {
    call_method!(ctx.run, id, method!("wake"), params![]);
});"#,
        ),
        (
            "Web build",
            "Ship demos through the WASM target.",
            r#"cargo run -p perro_cli -- build \
  --path D:\MyGame \
  --target web"#,
        ),
    ];

    view! {
        <section class="band code-showcase">
            <div class="section-head">
                <p class="eyebrow">"Code first"</p>
                <h2>"Small Rust pieces, wired through scenes"</h2>
            </div>
            <div class="code-grid">
                {examples.into_iter().map(|(name, body, code)| view! {
                    <article class="code-card">
                        <div class="code-card-head">
                            <h3>{name}</h3>
                            <p>{body}</p>
                        </div>
                        <CodeBlock code=code />
                    </article>
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
pub fn FeatureGrid() -> impl IntoView {
    let features = [
        ("Perro Book", "Linear install-to-release guide.", "/book"),
        (
            "Scenes + NodeID",
            "Object-centered scene trees with typed node access.",
            "/docs/scripting/nodes",
        ),
        (
            "Rust scripts",
            "Lifecycle hooks, methods, and per-node state.",
            "/docs/scripting/README",
        ),
        (
            "2D + 3D",
            "Sprite, mesh, physics, lights, UI, water, animation.",
            "/docs/project/feature_matrix",
        ),
        (
            "Static export",
            "Bake supported assets for lower runtime load cost.",
            "/docs/project/performance_philosophy",
        ),
        (
            "Perro CLI",
            "new, check, dev, build, web, DLC, profile, format.",
            "/docs/tools/perro_cli",
        ),
        (
            "Web target",
            "Build browser bundles and route scenes through WASM.",
            "/docs/WASM",
        ),
    ];

    view! {
        <section class="band">
            <div class="section-head">
                <p class="eyebrow">"Features"</p>
                <h2>"Built 4 simple authoring + fast release loads"</h2>
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
fn DemoBand() -> impl IntoView {
    view! {
        <section class="band demo-band">
            <div class="section-head">
                <p class="eyebrow">"Examples"</p>
                <h2>"Run 2D + 3D demos in browser"</h2>
            </div>
            <div class="demo-grid">
                <DemoCard name="Demo2D" body="Sprite stress, lights, water, physics, animation, skeletal tails." href="/examples/2d" />
                <DemoCard name="Demo3D" body="Materials, lights, water, particles, sky, physics, audio, mesh demos." href="/examples/3d" />
            </div>
        </section>
    }
}

#[component]
fn DocsPreview() -> impl IntoView {
    let docs = docs_by_area();
    view! {
        <section class="band">
            <div class="section-head row">
                <div>
                    <p class="eyebrow">"Docs"</p>
                    <h2>"API ref grid"</h2>
                </div>
                <a class="text-link" href="/docs">"Open docs"</a>
            </div>
            <div class="doc-grid">
                {docs.into_iter().take(8).map(|(area, count)| {
                    let href = format!("/docs?area={area}");
                    view! {
                    <a class="doc-card" href=href>
                        <strong>{area}</strong>
                        <span>{count}" pages"</span>
                    </a>
                }}).collect_view()}
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
