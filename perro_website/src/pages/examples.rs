use leptos::prelude::*;

use crate::layout::PageFrame;
use crate::shared::{CodeBlock, DemoCard, Seo, SeoInfo};

#[component]
pub fn ExamplesPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Examples and Demos",
            "Run Perro 2D and 3D WebAssembly demos, inspect browser builds, and use demo projects as examples for Rust game engine scenes, assets, and scripts.",
            "Perro examples, Perro demos, Rust game engine examples, WASM demos, Demo2D, Demo3D, 2D game demo, 3D game demo",
            "/examples",
        ).with_schema(examples_schema()) />
        <PageFrame eyebrow="Examples" title="Perro demos">
            <div class="demo-grid">
                <DemoCard name="Demo2D" body="Open 2D web bundle." href="/examples/2d" />
                <DemoCard name="Demo3D" body="Open 3D web bundle." href="/examples/3d" />
            </div>
        </PageFrame>
    }
}

#[component]
pub fn Demo2dPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Demo2D",
            "Run the Perro Demo2D browser build with sprites, lights, water, physics, animation, UI, skeletal samples, and WebAssembly output.",
            "Perro Demo2D, 2D WASM demo, sprite demo, 2D lights, 2D physics, Rust game engine demo",
            "/examples/2d",
        ) />
        <DemoPage title="Demo2D" src="/demos/demo2d/index.html" code="cargo run -p perro_cli -- dev --path demos\\Demo2D --target web" />
    }
}

#[component]
pub fn Demo3dPage() -> impl IntoView {
    view! {
        <Seo info=SeoInfo::new(
            "Demo3D",
            "Run the Perro Demo3D browser build with materials, lights, water, particles, sky, physics, audio, mesh demos, and WebAssembly output.",
            "Perro Demo3D, 3D WASM demo, 3D materials, 3D lights, particles demo, physics demo, Rust game engine demo",
            "/examples/3d",
        ) />
        <DemoPage title="Demo3D" src="/demos/demo3d/index.html" code="cargo run -p perro_cli -- dev --path demos\\Demo3D --target web" />
    }
}

#[component]
fn DemoPage(title: &'static str, src: &'static str, code: &'static str) -> impl IntoView {
    view! {
        <PageFrame eyebrow="Examples" title=title>
            <div class="demo-runner">
                <div class="demo-toolbar">
                    <CodeBlock code=code />
                    <a class="btn ghost" href=src target="_blank" rel="noreferrer">"Open full"</a>
                </div>
                <div class="demo-frame">
                    <iframe src=src title=title tabindex="0" allow="autoplay; fullscreen; gamepad"></iframe>
                </div>
            </div>
        </PageFrame>
    }
}

fn examples_schema() -> String {
    r#"{
  "@context": "https://schema.org",
  "@type": "CollectionPage",
  "name": "Perro examples and demos",
  "description": "Runnable 2D and 3D WebAssembly demos for the Perro Rust game engine.",
  "hasPart": [
    {"@type": "SoftwareApplication", "name": "Demo2D", "applicationCategory": "Game"},
    {"@type": "SoftwareApplication", "name": "Demo3D", "applicationCategory": "Game"}
  ]
}"#
    .to_string()
}
