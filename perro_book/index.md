# Perro Book

Build one small Perro game from install to release.

## Goal

Learn the engine as one connected flow: create a project, own state, connect
scene nodes, read input, load assets, add feedback, and ship native or web.

## Mental Model

The book follows one feature thread.

```text
input -> player script -> scene nodes -> world feedback -> UI
      -> build pipeline -> native or web release
```

Reference pages answer “what call exists?”

Book chapters answer “which ownership and communication shape fits?”

## Use / Avoid

Read chapters in order for first use.

Jump to the [API Map](api_map.md) when the system choice already feels clear.

Avoid treating generated glue or packed output as hand-authored source.

Avoid holding broad runtime access when a fixed node ref or method fits.

## Read Order

Read chapters in numbered order for first use.

Return through search, related pages, or the API map after the first project.

## Chapter Map

| Step | Chapter | Goal |
| --- | --- | --- |
| 1 | [Install + Tools](install.md) | Install CLI, check env, make project. |
| 2 | [First Project](first_project.md) | Create a small game loop. |
| 3 | [Scenes + Nodes](scenes_nodes.md) | Own a scene tree and choose node types. |
| 4 | [Scripting Model](scripting_model.md) | Split state, behavior, methods, and variants. |
| 5 | [Rust Scripting](rust_scripting.md) | Write state, lifecycle, and methods. |
| 6 | [Runtime Nodes](runtime_nodes.md) | Read and mutate nodes with short API calls. |
| 7 | [Generated Script Glue](generated_script_glue.md) | See what check, dev, and build generate. |
| 8 | [Input](input.md) | Read devices and remappable actions. |
| 9 | [Assets + Resources](assets_resources.md) | Load and own textures, meshes, audio, and data. |
| 10 | [UI, Animation, Audio](ui_animation_audio.md) | Add visible and audible feedback. |
| 11 | [Physics + Queries](physics_queries.md) | Use bodies, casts, areas, tags, and queries. |
| 12 | [Demos + Web Export](demos_web.md) | Read demos and build WASM output. |
| 13 | [Performance + Release](performance_release.md) | Profile, pack, test, and ship. |
| 14 | [API Map](api_map.md) | Jump from concepts to exact reference pages. |

## Data Flow

Each chapter carries the same player feature forward.

Data starts at project files.

CLI and compiler steps generate glue and packed assets.

Runtime APIs expose narrow windows to scripts.

Release keeps the same game logic with target-specific limits called out.

## Full Example

Use this reading path:

```powershell
cargo run -p perro_cli -- install
cargo run -p perro_cli -- new --name MyGame --path D:\GameProjects
cargo run -p perro_cli -- check --path D:\GameProjects\MyGame
cargo run -p perro_cli -- dev --path D:\GameProjects\MyGame
```

Continue through scenes, scripting, input, and assets before release work.

## Book Contract

Each chapter gives a goal, mental model, ownership choice, complete code shape,
failure behavior, performance notes, compatibility limits, and reference links.

The book explains decisions.

Reference docs define exact calls.

## Failure Behavior

Each chapter names validation points and runtime fallbacks.

Stop at the first `perro check` error.

Do not diagnose generated Rust before fixing the source script or scene path.

## Performance + Compatibility

Book examples favor narrow access, ahead-of-time asset work, and explicit
platform limits.

Native and web share the project model.

Platform chapters mark APIs that differ or do not exist on web.

## API Reference

Use the [API Map](api_map.md) after the guided path.

Use the [Docs Index](../docs/index.md) for all current reference pages.

## Related Pages

- [Install + Tools](install.md)
- [First Project](first_project.md)
- [Script Authoring Guide](../docs/scripting/authoring/index.md)
- [Performance + Release](performance_release.md)
