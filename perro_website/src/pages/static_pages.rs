use leptos::prelude::*;

use crate::layout::PageFrame;
use crate::shared::{CodeBlock, Seo, SeoInfo};

#[component]
pub fn NewsPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "News",
            "Track Perro project notes for docs, demos, editor work, runtime polish, CLI flows, and Rust game engine progress.",
            "Perro news, Perro project notes, Rust game engine progress, docs updates, demo updates",
            "/news",
        ) />
        <PageFrame eyebrow="News" title="Project notes">
            <section class="band flat">
                <div class="section-head">
                    <p class="eyebrow">"Current focus"</p>
                    <h2>"Docs, demos, editor, runtime polish"</h2>
                </div>
                <div class="feature-grid">
                    <InfoCard title="Website docs" body="Code examples use `#[State]`, `lifecycle!`, `methods!`, and runtime macros." href="/docs/scripting/README" />
                    <InfoCard title="Demos" body="Demo2D and Demo3D web bundles stay linked from examples." href="/examples" />
                    <InfoCard title="CLI flow" body="Project create, script sync, dev run, build, web export, and doctor live in Perro CLI." href="/docs/tools/perro_cli" />
                </div>
            </section>
            <section class="band flat split">
                <div>
                    <p class="eyebrow">"Check status"</p>
                    <h2>"Use repo state as source of truth"</h2>
                    <p>"Open commits, issues, and pull requests for live project progress."</p>
                </div>
                <div class="page-actions">
                    <a class="btn primary" href="https://github.com/PerroEngine/Perro" target="_blank" rel="noreferrer">"Repository"</a>
                    <a class="btn ghost" href="https://github.com/PerroEngine/Perro/issues" target="_blank" rel="noreferrer">"Issues"</a>
                </div>
            </section>
        </PageFrame>
    }
}

#[component]
pub fn CommunityPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Community",
            "Join Perro development through GitHub issues, docs patches, examples, demos, and small reproducible Rust game engine reports.",
            "Perro community, Perro GitHub, Rust game engine issues, contribute docs, share demos",
            "/community",
        ) />
        <PageFrame eyebrow="Community" title="Build with Perro">
            <section class="band flat">
                <div class="section-head">
                    <p class="eyebrow">"Start points"</p>
                    <h2>"Use GitHub for code, issues, and patches"</h2>
                </div>
                <div class="feature-grid">
                    <InfoCard title="Report bugs" body="Include OS, GPU, command, scene/script path, and repro steps." href="https://github.com/PerroEngine/Perro/issues" />
                    <InfoCard title="Contribute docs" body="Fix stale examples, add focused pages, and link to real APIs." href="https://github.com/PerroEngine/Perro/tree/main/docs" />
                    <InfoCard title="Share demos" body="Use the demo projects as patterns for scenes, scripts, assets, and web export." href="/examples" />
                </div>
            </section>
            <section class="band flat">
                <div class="section-head">
                    <p class="eyebrow">"Bug report shape"</p>
                    <h2>"Small repros help most"</h2>
                </div>
                <CodeBlock code=r#"perro doctor --path D:\GameProjects\MyGame
perro check --path D:\GameProjects\MyGame
perro dev --path D:\GameProjects\MyGame"# />
            </section>
        </PageFrame>
    }
}

#[component]
pub fn AssetsPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Assets and Templates",
            "Use Perro demo assets and templates for 2D, 3D, scenes, scripts, animation, UI, physics, audio, and WebAssembly examples.",
            "Perro assets, Perro templates, demo assets, scene templates, script templates, animation assets, Rust game assets",
            "/assets",
        ) />
        <PageFrame eyebrow="Assets" title="Demos and templates">
            <section class="band flat">
                <div class="section-head">
                    <p class="eyebrow">"Project assets"</p>
                    <h2>"Use shipped demos as working refs"</h2>
                </div>
                <div class="demo-grid">
                    <InfoCard title="Demo2D" body="Sprites, lights, water, physics, animation, UI, and skeletal samples." href="/examples/2d" />
                    <InfoCard title="Demo3D" body="Materials, lights, water, particles, sky, physics, audio, and mesh samples." href="/examples/3d" />
                    <InfoCard title="New project" body="Generate a clean game folder with scenes, input map, deps, and script template." href="/learn/getting-started" />
                </div>
            </section>
            <section class="band flat split">
                <div>
                    <p class="eyebrow">"Template commands"</p>
                    <h2>"Create assets from CLI"</h2>
                    <p>"Use `new_scene`, `new_script`, `new_animation`, and `new_panimtree` for project-local files."</p>
                </div>
                <CodeBlock code=r#"perro new_script --name Player --res /scripts
perro new_scene --name Arena --template 3D
perro new_animation --name HeroRun"# />
            </section>
        </PageFrame>
    }
}

#[component]
fn InfoCard(title: &'static str, body: &'static str, href: &'static str) -> impl IntoView {
    view! {
        <a class="feature-card" href=href>
            <h3>{title}</h3>
            <p>{body}</p>
        </a>
    }
}
