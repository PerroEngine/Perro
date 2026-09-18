# Scene Node Templates

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Templates | [Templates](#templates) |

## Purpose

These pages are copy-and-paste `.scn` field references for authoring scenes by hand or in the editor. Each shows the exact node blocks and field names for a node type — 2D, 3D, UI, and multi-node examples — so you can place a camera, sprite, mesh, light, or physics body without guessing. For nodes a script builds at runtime (spawn packs, generated UI, prefabs), use [Node Collections](../node_collections.md) instead.

## Authoring Rule

Author known game composition in `.scn` files. Put topology, child nodes,
stable names, transforms, assets, scripts, tags, and `script_vars` in the scene.
This keeps composition reusable and reviewable, and lets the editor and build
tools inspect the same source.

Keep Rust for state, lifecycle, methods, signals, queries, and runtime choices.
Do not build a fixed scene tree with `create_nodes!` or `node_collection!`.
Load a reusable authored subtree with `scene_load!` or `scene_preload!`; use
runtime node creation only for content that does not exist until play time.

## Use Cases

- Look up the fields for a 2D node you are placing (sprite, camera, tilemap, water): [2D `.scn` fields](2d.md).
- Look up the fields for a 3D node (mesh instance, camera, light, skeleton): [3D `.scn` fields](3d.md).
- Build a screen-space HUD or menu from UI nodes: [UI `.scn` fields](ui.md).
- Copy a working multi-node fragment (camera streams, webcam, script vars, animation bindings, render layers, physics parity): [Extra `.scn` examples](examples.md).

## Decision Guide

Use templates to learn or copy the exact field shape accepted by `.scn` parsing. Use the node and authoring guides to decide which node owns a behavior. A template proves syntax; it does not replace ownership, reference, or lifecycle design.

Scene -> topology + fixed wiring
Rust -> behavior + dynamic decisions
Runtime node creation -> transient or genuinely runtime-generated content

## Templates

- [2D `.scn` fields](2d.md)
- [3D `.scn` fields](3d.md)
- [UI `.scn` fields](ui.md)
- [Extra `.scn` examples](examples.md)
