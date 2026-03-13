# Resource Context

Type:

- `res: &ResourceContext<'_, RS>`

Purpose:

- Accesing resource state at runtime

Accessors:

- `res.Textures()`
- `res.Audio()`
- `res.Meshes()`
- `res.Materials()`
- `res.Terrain()`

## Resource Modules

- [Textures Module](resource_modules/textures.md)
- [Audio Module](resource_modules/audio.md)
- [Meshes Module](resource_modules/meshes.md)
- [Materials Module](resource_modules/materials.md)
- [Terrain Module](resource_modules/terrain.md)

Each module page contains:

- Macro reference
- `res.<Module>()` method reference
- Examples
- Notes on behavior and caveats
- Exact load/reserve/drop semantics where applicable

## Simple Example

```rust
let texture_id = load_texture!(res, "res://textures/smoke.png");
let mesh_id = load_mesh!(res, "res://meshes/rock.glb");
let material_id = load_material!(res, "res://materials/smoke.pmat");
let _reserved = reserve_texture!(res, "res://textures/smoke.png");
let _ = drop_mesh!(res, "res://meshes/old.glb");

let music = bus!("music");
let _ = set_master_volume!(res, 1.0);
let _ = set_bus_volume!(res, music, 0.7);

let _ = play_audio!(
    res,
    Audio {
        source: "res://groantube.mp3",
        bus: music,
        looped: true,
        volume: 1.0,
        pitch: 1.0,
    }
);
```
