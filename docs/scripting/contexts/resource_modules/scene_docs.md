# Scene Docs Module

## Page Map

| Header            | Link                                 |
| ----------------- | ------------------------------------ |
| Purpose           | [Purpose](#purpose)                  |
| Use Cases         | [Use Cases](#use-cases)              |
| Context           | [Context](#context)                  |
| Data Types        | [Data Types](#data-types)            |
| API Reference     | [API Reference](#api-reference)      |
| `load`            | [`load`](#load)                      |
| `load_hashed`     | [`load_hashed`](#load_hashed)        |
| `save`            | [`save`](#save)                      |
| `save_hashed`     | [`save_hashed`](#save_hashed)        |
| `write`           | [`write`](#write)                    |
| `scene_load_doc!` | [`scene_load_doc!`](#scene_load_doc) |
| `scene_save_doc!` | [`scene_save_doc!`](#scene_save_doc) |
| Editor Example    | [Editor Example](#editor-example)    |

## Purpose

`SceneDocs` works with `.scn` text documents.

This module is for parsing, inspecting, generating, and writing scene files. It is mostly editor/tooling-facing. Gameplay code usually loads scenes through `ctx.run.Scenes()` instead of loading or saving `.scn` source documents.

Use this page when building an editor feature, scene conversion tool, debug exporter, project migration script, or custom authoring flow that needs the scene file format as data.

Do not treat this as the normal gameplay scene-load API. For runtime scene instancing, use [Scenes Module](../runtime_modules/scenes.md).

## Use Cases

- Custom level editor: load a `.scn` with `scene_load_doc!`, edit `doc.scene_mut()`, and write it back with `scene_save_doc!`.
- Scene conversion or migration tools: parse many `.scn` files, transform them, and re-serialize with `save` after `normalize_links()`.
- Debug exporter: dump a live document to `.scn` source text with `write(&doc).to_text()` without touching asset storage.
- Duplicating and templating levels: `load` a base scene, tweak it, then `save` to a new path as a variant.
- Inspecting authored data: read scene vars and node structure from `SceneDoc` for validation or reporting.

## Ownership And Choice

Scene docs own parsed or generated scene data before runtime instantiation. Use them for tools, procedural scene authoring, import/export, or runtime content pipelines. Use runtime node APIs to change the live world. Keep document edits and live-node edits separate, and validate references before loading the resulting document as a scene.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.SceneDocs()`
- Backing source: `perro_resource_api/src/sub_apis/scene_doc.rs`
- Runtime implementation: `perro_runtime/src/rs_ctx/scene_doc.rs`
- Scene document type: `perro_scene::SceneDoc`

`ctx.res` means resource/file access. These calls may touch asset storage and parse/write source text.

## Data Types

### `SceneDoc`

Signature:

```rust
pub struct SceneDoc {
    pub vars: Cow<'static, [SceneVar]>,
    pub scene: Scene,
}
```

Meaning:

| Field   | Type                       | Meaning                                                                               |
| ------- | -------------------------- | ------------------------------------------------------------------------------------- |
| `vars`  | `Cow<'static, [SceneVar]>` | Scene document variables collected from source text, excluding root alias duplicates. |
| `scene` | `Scene`                    | Parsed scene data used by runtime preparation.                                        |

Common methods:

| Signature                                           | Returns      | Use when                                         | Edge behavior                           |
| --------------------------------------------------- | ------------ | ------------------------------------------------ | --------------------------------------- |
| `pub fn SceneDoc::parse(src: &str) -> Self`         | `SceneDoc`   | Parse `.scn` source text into document data.     | Parses vars and scene from source text. |
| `pub fn SceneDoc::from_scene(scene: Scene) -> Self` | `SceneDoc`   | Wrap prepared scene data as a writable document. | Starts with no vars.                    |
| `pub fn into_scene(self) -> Scene`                  | `Scene`      | Drop document vars and keep parsed scene.        | Consumes doc.                           |
| `pub fn scene(&self) -> &Scene`                     | `&Scene`     | Inspect parsed scene.                            | Borrow only.                            |
| `pub fn scene_mut(&mut self) -> &mut Scene`         | `&mut Scene` | Edit parsed scene data.                          | Caller must keep scene data valid.      |
| `pub fn normalize_links(&mut self)`                 | `()`         | Sync parent/child links before saving.           | `save` calls this before writing.       |
| `pub fn to_text(&self) -> String`                   | `String`     | Convert document back to `.scn` text.            | Clones and normalizes before writing.   |

### `SceneWrite`

Signature:

```rust
pub struct SceneWrite<'a> {
    doc: &'a SceneDoc,
}
```

Common methods:

| Signature                                           | Returns          | Use when                                                  |
| --------------------------------------------------- | ---------------- | --------------------------------------------------------- |
| `pub fn SceneWrite::new(doc: &'a SceneDoc) -> Self` | `SceneWrite<'a>` | Create a writer view over a scene doc.                    |
| `pub fn to_text(&self) -> String`                   | `String`         | Write `.scn` source text without saving to asset storage. |

## API Reference

### `load`

| Field                      | Detail                                                                                                                                                |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.SceneDocs()`                                                                                                                                 |
| Signature                  | `pub fn load<P: ResPathSource>(&self, path: P) -> Result<SceneDoc, String>`                                                                           |
| Params                     | `path: P`, usually a `res://...scn` path or compatible resource path source.                                                                          |
| Returns                    | `Result<SceneDoc, String>`                                                                                                                            |
| Use when                   | Use when editor/tooling code needs to parse a `.scn` file into document data.                                                                         |
| Why                        | Keeps source-level scene vars and parsed scene data available together.                                                                               |
| Fails when / edge behavior | Returns `Err(String)` if asset load fails or source bytes are not valid UTF-8. Parser errors are represented by parser behavior in `SceneDoc::parse`. |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let doc = ctx.res.SceneDocs().load("res://levels/arena.scn");

        if let Ok(doc) = doc {
            let text = ctx.res.SceneDocs().write(&doc).to_text();
            let _ = text;
        }
    }
});
```

### `load_hashed`

| Field                      | Detail                                                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.SceneDocs()`                                                                                                             |
| Signature                  | `pub fn load_hashed<P: ResPathSource>(&self, path_hash: u64, path: P) -> Result<SceneDoc, String>`                                |
| Params                     | `path_hash: u64`, `path: P`.                                                                                                      |
| Returns                    | `Result<SceneDoc, String>`                                                                                                        |
| Use when                   | Use when a literal path hash is already available from macro/static lookup flow.                                                  |
| Why                        | Mirrors resource module hashed path patterns. Runtime implementation currently delegates to normal load after accepting the hash. |
| Fails when / edge behavior | Same as `load`. Hash is ignored by the default runtime implementation.                                                            |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let doc = ctx.res.SceneDocs().load_hashed(0, "res://levels/arena.scn");
        let _ = doc;
    }
});
```

### `save`

| Field                      | Detail                                                                                         |
| -------------------------- | ---------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.SceneDocs()`                                                                          |
| Signature                  | `pub fn save<P: ResPathSource, D: IntoSceneDoc>(&self, path: P, doc: D) -> Result<(), String>` |
| Params                     | `path: P`, `doc: D` where `D` can be `SceneDoc`, `&SceneDoc`, `Scene`, or `&Scene`.            |
| Returns                    | `Result<(), String>`                                                                           |
| Use when                   | Use when editor/tooling code needs to write `.scn` source back to asset storage.               |
| Why                        | Normalizes scene links before converting document to text and saving bytes.                    |
| Fails when / edge behavior | Returns `Err(String)` if save fails. Calls `normalize_links()` before writing.                 |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Ok(mut doc) = ctx.res.SceneDocs().load("res://levels/arena.scn") {
            doc.normalize_links();
            let _ = ctx.res.SceneDocs().save("res://levels/arena_copy.scn", &doc);
        }
    }
});
```

### `save_hashed`

| Field                      | Detail                                                                                                                            |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.SceneDocs()`                                                                                                             |
| Signature                  | `pub fn save_hashed<P: ResPathSource, D: IntoSceneDoc>(&self, path_hash: u64, path: P, doc: D) -> Result<(), String>`             |
| Params                     | `path_hash: u64`, `path: P`, `doc: D` where `D: IntoSceneDoc`.                                                                    |
| Returns                    | `Result<(), String>`                                                                                                              |
| Use when                   | Use when editor/tooling code saves a `.scn` doc and already has a literal path hash.                                              |
| Why                        | Mirrors resource module hashed path patterns. Runtime implementation currently delegates to normal save after accepting the hash. |
| Fails when / edge behavior | Same as `save`. Hash is ignored by the default runtime implementation.                                                            |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Ok(doc) = ctx.res.SceneDocs().load("res://levels/arena.scn") {
            let _ = ctx.res.SceneDocs().save_hashed(0, "res://levels/arena_copy.scn", doc);
        }
    }
});
```

### `write`

| Field                      | Detail                                                                                       |
| -------------------------- | -------------------------------------------------------------------------------------------- |
| Access                     | `ctx.res.SceneDocs()`                                                                        |
| Signature                  | `pub fn write<'a>(&self, doc: &'a SceneDoc) -> SceneWrite<'a>`                               |
| Params                     | `doc: &'a SceneDoc`.                                                                         |
| Returns                    | `SceneWrite<'a>`                                                                             |
| Use when                   | Use when editor/tooling code needs `.scn` source text without saving a file.                 |
| Why                        | Gives a small writer object with `to_text()`.                                                |
| Fails when / edge behavior | `write` itself does not fail. `to_text()` clones and normalizes the doc before writing text. |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Ok(doc) = ctx.res.SceneDocs().load("res://levels/arena.scn") {
            let text = ctx.res.SceneDocs().write(&doc).to_text();
            let _ = text;
        }
    }
});
```

### `scene_load_doc!`

| Field                      | Detail                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| Access                     | macro over `ctx.res`                                                                        |
| Signature                  | `scene_load_doc!(ctx.res, path)`                                                            |
| Params                     | `ctx.res`, `path`.                                                                          |
| Returns                    | `Result<SceneDoc, String>`                                                                  |
| Use when                   | Use when editor/tooling code loads `.scn` document data and wants the macro form.           |
| Why                        | Literal paths use a compile-time hash and call `load_hashed`; expression paths call `load`. |
| Fails when / edge behavior | Same as `load`.                                                                             |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let doc = scene_load_doc!(ctx.res, "res://levels/arena.scn");
        let _ = doc;
    }
});
```

### `scene_save_doc!`

| Field                      | Detail                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------- |
| Access                     | macro over `ctx.res`                                                                        |
| Signature                  | `scene_save_doc!(ctx.res, path, doc)`                                                       |
| Params                     | `ctx.res`, `path`, `doc`.                                                                   |
| Returns                    | `Result<(), String>`                                                                        |
| Use when                   | Use when editor/tooling code saves `.scn` document data and wants the macro form.           |
| Why                        | Literal paths use a compile-time hash and call `save_hashed`; expression paths call `save`. |
| Fails when / edge behavior | Same as `save`.                                                                             |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Ok(doc) = scene_load_doc!(ctx.res, "res://levels/arena.scn") {
            let _ = scene_save_doc!(ctx.res, "res://levels/arena_copy.scn", &doc);
        }
    }
});
```

## Editor Example

This shows the intended shape: load `.scn` text into a document, operate on document data, write source text or save a new `.scn`.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.export_copy(ctx);
    }
});

methods!({
    fn export_copy(&self, ctx: &mut ScriptContext<'_, API>) {
        let Ok(mut doc) = scene_load_doc!(ctx.res, "res://levels/arena.scn") else {
            return;
        };

        doc.normalize_links();

        let scene_text = ctx.res.SceneDocs().write(&doc).to_text();
        let _ = scene_text;

        let _ = scene_save_doc!(ctx.res, "res://levels/arena_copy.scn", &doc);
    }
});
```
