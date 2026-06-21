# Perro Website

Leptos SSR website for Perro.

## Run

```powershell
cargo check -p perro_website
cargo check -p perro_website --no-default-features --features hydrate --target wasm32-unknown-unknown
cargo leptos watch --package perro_website
```

`cargo leptos` req:

```powershell
cargo install cargo-leptos
```

## Demos

Sync built web demos:

```powershell
perro_website\scripts\sync_demos.ps1
```

Build demo bundles:

```powershell
cargo run -p perro_cli -- build --path demos\Demo2D --target web
cargo run -p perro_cli -- build --path demos\Demo3D --target web
```

`public/demos/demo2d/index.html` and `public/demos/demo3d/index.html` keep browser previews live when full exported bundles are not synced.
