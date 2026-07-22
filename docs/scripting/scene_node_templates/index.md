# Scene Node Templates

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Templates | [Templates](#templates) |

## Purpose

These pages are copy-and-paste `.scn` field references for authoring scenes by hand or in the editor. Each shows the exact node blocks and field names for a node type — 2D, 3D, UI, and multi-node examples — so you can place a camera, sprite, mesh, light, or physics body without guessing. For nodes a script builds at runtime (spawn packs, generated UI, prefabs), use [Node Collections](../node_collections.md) instead.

## Use Cases

- Look up the fields for a 2D node you are placing (sprite, camera, tilemap, water): [2D `.scn` fields](2d.md).
- Look up the fields for a 3D node (mesh instance, camera, light, skeleton): [3D `.scn` fields](3d.md).
- Build a screen-space HUD or menu from UI nodes: [UI `.scn` fields](ui.md).
- Copy a working multi-node fragment (camera streams, webcam, script vars, animation bindings, render layers, physics parity): [Extra `.scn` examples](examples.md).

## Decision Guide

Use templates to learn or copy the exact field shape accepted by `.scn` parsing. Use the node and authoring guides to decide which node owns a behavior. A template proves syntax; it does not replace ownership, reference, or lifecycle design.

## Templates

- [2D `.scn` fields](2d.md)
- [3D `.scn` fields](3d.md)
- [UI `.scn` fields](ui.md)
- [Extra `.scn` examples](examples.md)
