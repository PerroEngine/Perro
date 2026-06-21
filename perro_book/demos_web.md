# Demos + Web Export

Perro can build runnable web demos through WASM.

## Goal

Run Demo2D, Demo3D, and your game in the browser.

## Demo Projects

Use shipped demos as working refs:

- `demos/Demo2D`
- `demos/Demo3D`

Run:

```powershell
perro dev --path demos\Demo2D --target web
perro dev --path demos\Demo3D --target web
```

Source workspace:

```powershell
cargo run -p perro_cli -- dev --path demos\Demo2D --target web
cargo run -p perro_cli -- dev --path demos\Demo3D --target web
```

## Web Build

Build your game for web:

```powershell
perro build --path D:\GameProjects\MyGame --target web
```

Output goes under project build output.

Website demo bundles can be synced into `perro_website/public/demos`.

## Web Limits

Expect browser limits:

- user gesture needed for some audio starts
- filesystem access differs from desktop
- CPU/GPU budgets vary by browser
- network and storage APIs have browser rules

Test desktop and browser separately.

## Demo Docs

Demos should show:

- scene path
- script path
- controls
- systems shown
- web build command

## Reference

- [WASM / Web Target](/docs/WASM.md)
- [Perro CLI](/docs/tools/perro_cli.md)
- [Demo2D Docs](/docs/examples/demo2d)
- [Demo3D Docs](/docs/examples/demo3d)
