# Demos + Web Export

Perro can build runnable web demos through WASM.

## Goal

Run Demo2D, Demo3D, and your game in the browser.

## How To Read A Demo

Start from its README feature map. Open the named scene to see authored nodes,
`script_vars`, and asset choices. Open the linked script to see lifecycle and
communication. Run the lane last to observe the complete flow.

Copy the ownership pattern, not its asset paths or stress counts. Demo2D and
Demo3D favor visible feature isolation; a production game may combine those
systems behind different scene boundaries.

## Demo Projects

Use shipped demos as working refs:

- `demos/ScriptPatterns` for one small communication flow
- `demos/Demo2D`
- `demos/Demo3D`
- `demos/DemoUI`

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
- [ScriptPatterns Feature Map](../demos/ScriptPatterns/README.md)
- [Demo2D Feature Map](../demos/Demo2D/README.md)
- [Demo3D Feature Map](../demos/Demo3D/README.md)
- [DemoUI Feature Map](../demos/DemoUI/README.md)
