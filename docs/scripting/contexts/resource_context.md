# Resource Context

Type:

- `res: &ResourceContext<'_, RS>`

Purpose:

- Accesing resource state at runtime

Accessors:

- `res.Animations()`
- `res.Textures()`
- `res.Audio()`
- `res.Meshes()`
- `res.Materials()`
- `res.Skeletons()`
- `res.Draw2D()`
- `res.Localization()`
- Direct global post-processing methods (no accessor)
- Direct visual accessibility methods (no accessor)
- Direct viewport query method (no accessor)

## Resource Modules

- [Animations Module](resource_modules/animations.md)
- [Textures Module](resource_modules/textures.md)
- [Audio Module](resource_modules/audio.md)
- [Meshes Module](resource_modules/meshes.md)
- [Materials Module](resource_modules/materials.md)
- [Skeletons Module](resource_modules/skeletons.md)
- [Draw2D Module](resource_modules/draw_2d.md)
- [Localization Module](resource_modules/localization.md)
- [Global Post Processing](resource_modules/post_processing.md)
- [Visual Accessibility](resource_modules/visual_accessibility.md)

Each module page contains:

- Macro reference
- `res.<Module>()` method reference
- Examples
- Notes on behavior and caveats
- Exact load/reserve/drop semantics where applicable

Reserve convention:

- `load` implies `reserved: false` (auto-evict when no references remain).
- `reserve` implies `reserved: true` (keep cached until explicit drop).

## Localization Setup

Localization source is configured in `project.toml`:

```toml
[localization]
source = "res://localization.csv"
key = "key"
default_locale = "en"
```

CSV format:

- Header must contain key column plus locale columns.
- `key` column stores lookup keys.
- Locale columns are language codes (`en`, `es`, `fr`, `ja`, `zh`, or custom codes).

Example:

```csv
key,en,es
menu.start,Start,Iniciar
menu.quit,Quit,Salir
```

Behavior:

- Dev mode loads the configured CSV from disk/asset path and keeps only the active locale column in memory.
- Static mode compiles per-locale hashed lookup tables; the configured localization CSV is excluded from `assets.perro` to avoid duplication.

## Simple Example

```rust
let texture_id = texture_load!(res, "res://textures/smoke.png");
let mesh_id = mesh_load!(res, "res://meshes/rock.glb");
let material_id = material_load!(res, "res://materials/smoke.pmat");
let bones = skeleton_load_bones!(res, "res://models/rig.gltf:skeleton[0]");
let _reserved = texture_reserve!(res, "res://textures/smoke.png");
let _ = mesh_drop!(res, "res://meshes/old.glb");

let music = audio_bus!("music");
let _ = audio_set_master_volume!(res, 1.0);
let _ = audio_bus_set_volume!(res, music, 0.7);
let _ = audio_bus_set_speed!(res, music, 1.0);
let viewport = get_viewport_size!(res);

let _ = audio_play!(
    res,
    Audio {
        source: "res://groantube.mp3",
        bus: music,
        looped: true,
        volume: 1.0,
        speed: 1.0,
        from_start: 0.0,
        from_end: 0.0,
    }
);
```
