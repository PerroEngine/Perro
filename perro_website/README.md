# Perro Website

Leptos SSR website for Perro.

## Run

```powershell
cargo check -p perro_website
cargo check -p perro_website --no-default-features --features hydrate --target wasm32-unknown-unknown
cargo leptos watch -p perro_website
```

`cargo leptos` req:

```powershell
cargo install cargo-leptos
```

## Stripe sponsor checkout

Copy `.env.example` to a local env file and set Stripe test keys plus Price IDs.

`POST /api/sponsor` accepts:

```json
{ "id": 1, "amount": null }
```

- `1..7`: monthly subscription tiers
- `0`: one-time USD support from `$1` to `$99,999`
- `101..107`: corporate subscription tiers

Fixed tiers use server-owned Stripe Price IDs.

Never expose `STRIPE_SECRET_KEY` in browser code or commit local env files.

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
