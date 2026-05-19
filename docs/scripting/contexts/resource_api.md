# Resource API

Type:

- `ctx: &mut ScriptContext<'_, API>`
- resource window handle: `ctx.res`

Purpose:

- Accesing resource state at runtime
- Mesh/material data types used by resource APIs are available from Perro prelude.

Accessors:

- `ctx.res.Animations()`
- `ctx.res.Textures()`
- `ctx.res.Audio()`
- `ctx.res.Csv()`
- `ctx.res.Meshes()`
- `ctx.res.Materials()`
- `ctx.res.Skeletons()`
- `ctx.res.Draw2D()`
- `ctx.res.Localization()`
- Direct global post-processing methods (no accessor)
- Direct visual accessibility methods (no accessor)
- Direct viewport query method (no accessor)

## Resource Modules

- [Animations Module](resource_modules/animations.md)
- [Textures Module](resource_modules/textures.md)
- [Audio Module](resource_modules/audio.md)
- [CSV Module](resource_modules/csv.md)
- [Meshes Module](resource_modules/meshes.md)
- [Materials Module](resource_modules/materials.md)
- [Skeletons Module](resource_modules/skeletons.md)
- [Draw2D Module](resource_modules/draw_2d.md)
- [Localization Module](resource_modules/localization.md)
- [Global Post Processing](resource_modules/post_processing.md)
- [Visual Accessibility](resource_modules/visual_accessibility.md)

Each module page contains:

- Macro reference
- `ctx.res.<Module>()` method reference
- Examples
- Notes on behavior and caveats
- Exact load/reserve/drop semantics where applicable
- Copy snapshot read/write/create data APIs for runtime resources where supported

Reserve convention:

- `load` implies `reserved: false` (auto-evict when no references remain).
- `reserve` implies `reserved: true` (keep cached until explicit drop).

Async load convention:

- ID-returning resource loads return the ID immediately.
- Decode, parse, and GPU/audio backend work can finish later.
- Until loaded, draw/play paths may skip that resource or use an empty/default value.
- Use `*_is_loaded!` when gameplay needs to know readiness.
- Good uses: hide loading UI, delay spawning visible objects, wait before starting a cutscene, defer soundfont notes until bank load completes.
- Bad uses: calling every frame for every object when retained render state can naturally skip missing resources.
- Prefer polling during load screens, state transitions, or a small timer such as every 1-2 seconds.

Loaded check macros:

- `texture_is_loaded!(res, texture_id) -> bool`
- `mesh_is_loaded!(res, mesh_id) -> bool`
- `material_is_loaded!(res, material_id) -> bool`
- `animation_is_loaded!(res, animation_id) -> bool`
- `animation_tree_is_loaded!(res, animation_tree_id) -> bool`
- `audio_is_loaded!(res, source) -> bool`
- `midi_soundfont_is_loaded!(res, soundfont_id) -> bool`

Copy data workflow:

- Read snapshot copy (`mesh_get_data!` / `material_get_data!`)
- Edit in script code
- Commit once (`mesh_write!` / `material_write!`)
- Prefer load-screen/batched edits, not per-frame writes

## Localization Setup

Localization source is a sibling CSV next to `project.toml`.

Use one filename:

- `localization.csv`
- `locale.csv`
- `translations.csv`

Do not put this file in `res/`.

`project.toml` only sets the default locale:

```toml
[localization]
default_locale = "en"
```

If `[localization]` or `default_locale` is unset, Perro uses `en`.

CSV format:

- First column must be `key`.
- Other columns are language codes (`en`, `es`, `fr`, `ja`, `zh`, or custom codes).
- `key` stores lookup keys.

Example:

```csv
key,en,es
menu.start,Start,Iniciar
menu.quit,Quit,Salir
```

Behavior:

- Dev mode loads the sibling CSV from disk and keeps only the active locale column in memory.
- Static mode compiles per-locale hashed lookup tables.
- `assets.perro` only packs files from `res/`, so sibling localization CSV files are never packed.

## Simple Example

```rust
let texture_id = texture_load!(ctx.res, "res://textures/smoke.png");
let mesh_id = mesh_load!(ctx.res, "res://meshes/rock.glb");
let material_id = material_load!(ctx.res, "res://materials/smoke.pmat");
let bones = skeleton_load_bones!(ctx.res, "res://models/rig.gltf:skeleton[0]");
let _reserved = texture_reserve!(ctx.res, "res://textures/smoke.png");
let _ = mesh_drop!(ctx.res, "res://meshes/old.glb");

let music = audio_bus!("music");
let _ = audio_set_master_volume!(ctx.res, 1.0);
let _ = audio_bus_set_volume!(ctx.res, music, 0.7);
let _ = audio_bus_set_speed!(ctx.res, music, 1.0);
let viewport = get_viewport_size!(ctx.res);
let items = csv_load!(ctx.res, "res://data/items.csv");
let sword = items.find_primary("sword").and_then(|row| row.get(1));

let _ = audio_play!(
    ctx.res,
    music,
    Audio {
        source: "res://groantube.mp3",
        looped: true,
        volume: 1.0,
        speed: 1.0,
        from_start: 0.0,
        from_end: 0.0,
    }
);
```
