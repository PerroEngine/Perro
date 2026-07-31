# Perro Docs

Use this reference when you know the system, file, command, or API you need.
Start with the [Perro Book](../perro_book/index.md) for a guided first project.

## Goal

Find one trusted page fast, see the ownership model, copy a complete example,
and know what failure looks like before the code runs.

## Mental Model

Perro docs follow the same runtime split as the engine:

```text
project files -> compiler + CLI -> packed assets -> runtime -> script context
```

Script context windows stay explicit:

- `ctx.run` -> live scene, nodes, time, signals, and runtime mutation
- `ctx.res` -> assets, resource formats, and resource lifetime
- `ctx.ipt` -> keyboard, mouse, gamepad, and action input

## Use / Avoid

Use the book for a linear path.

Use reference pages for exact calls, fields, limits, and failure behavior.

Avoid copying a single API call without its ownership and lifetime rules.

Avoid using history pages as current guidance.

## Page Table

### Start

- [Install + tools](../perro_book/install.md)
- [First project](../perro_book/first_project.md)
- [Perro CLI](tools/perro_cli.md)
- [Perro Editor](tools/perro_editor.md)
- [`project.toml`](project/project_toml.md)
- [Feature matrix](project/feature_matrix.md)

### Author

- [Script authoring guide](scripting/authoring/index.md)
- [Scenes + nodes](../perro_book/scenes_nodes.md)
- [Node types](scripting/nodes.md)
- [Node collections](scripting/node_collections.md)
- [UI nodes](scripting/ui.md)

### Script

- [Scripting overview](scripting/README.md)
- [State](scripting/state.md)
- [Lifecycle](scripting/lifecycle.md)
- [Methods](scripting/methods.md)
- [Variants](scripting/variant.md)

### Runtime

- [Runtime API](scripting/contexts/runtime_api.md)
- [Input API](scripting/contexts/input_api.md)
- [Resource API](scripting/contexts/resource_api.md)
- [Node runtime module](scripting/contexts/runtime_modules/nodes.md)
- [Physics runtime module](scripting/contexts/runtime_modules/physics.md)

### Assets

- [Resource management](resources/resource_management.md)
- [Materials](resources/materials.md)
- [Material files](resources/pmat.md)
- [Shaders](resources/shaders.md)
- [Animation](resources/animation.md)
- [Animation files](resources/panim.md)
- [Audio](resources/audio.md)
- [SSAO](resources/ssao.md)
- [2D shadows](resources/shadows2d.md)
- [Navigation meshes](resources/pnav.md)

### Platforms

- [Web / WASM](WASM.md)
- [HTTP](networking/http.md)
- [Multiplayer](networking/multiplayer.md)
- [Steamworks](platform/steamworks.md)
- [DLC](platform/dlc.md)

### Ship

- [Performance philosophy](project/performance_philosophy.md)
- [Release build profile](project/release_build_profile.md)
- [Web release](../perro_book/demos_web.md)
- [Performance + release](../perro_book/performance_release.md)

### Reference

- [Scene node specs](project/scene_node_specs.md)
- [2D node fields](scripting/scene_node_templates/2d.md)
- [3D node fields](scripting/scene_node_templates/3d.md)
- [UI node fields](scripting/scene_node_templates/ui.md)
- [Book API map](../perro_book/api_map.md)

### History

History and audit pages preserve decisions, measurements, and migration notes.
Use current guides for implementation.

- [Concept + example audit](concept_example_audit.md)
- [Codebase audit](project/codebase_audit_2026-07-09.md)
- [Static scene script index design](project/static_scene_script_index_design.md)

## Data Flow

Search reads titles, summaries, headings, slugs, and the first body segment.
Group filters narrow results without changing canonical routes.

```text
markdown -> build validation -> generated docs data
         -> left nav + article + right TOC
         -> search + deep links + prev/next
```

## Full Example

To add player input:

1. Read [Input](../perro_book/input.md) for the decision model.
2. Use [Input API](scripting/contexts/input_api.md) for context ownership.
3. Pick [Actions](scripting/contexts/input_modules/actions.md) for remappable input.
4. Copy a complete example and keep its failure checks.
5. Run `perro check` before `perro dev`.

## Failure Behavior

The website build fails on empty summaries, duplicate routes, duplicate heading
IDs, missing nav groups, broken internal docs links, and broken anchors.

Unknown docs routes render the shared 404 page.

## Performance + Compatibility

Search runs against generated local text.

No hosted search service or client index fetch sits on the reading path.

Docs work with SSR, hydration, keyboard navigation, and reduced motion.

## API Reference

Use the [Book API Map](../perro_book/api_map.md) for topic-to-reference links.

Use the generated docs search for exact call names.

## Related Pages

- [Perro Book](../perro_book/index.md)
- [Writing Standard](writing_standard.md)
- [Getting Started](../perro_book/install.md)
- [Perro CLI](tools/perro_cli.md)
