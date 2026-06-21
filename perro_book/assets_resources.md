# Assets + Resources

Resources are loaded through `ctx.res` and used by scene nodes or scripts.

## Goal

Load and manage textures, meshes, materials, audio, animation data, CSV, shaders, and scene data.

## Paths

Use `res://` for project assets.

Use `user://` for writable user data.

Keep source assets under `res/`.

## Load Pattern

Most load calls return an ID.

The backend may finish later.

Use loaded checks when a system must wait.

```rust
let texture = texture_load!(ctx.res, "res://textures/player.png");
if texture_is_loaded!(ctx.res, texture) {
    log_info!("texture ready");
}
```

Scripts store those IDs like normal state.

Examples:

- `TextureID` for sprites and UI images
- `MeshID` for mesh instances
- `MaterialID` for material swaps
- `AudioID` or runtime audio values for sounds

## Scenes And Refs

Scene files can wire script vars.

Use `NodeID` for scene node refs.

Use `#[node_ref(...)]` when a state field expects a node type.

```rust
#[State]
pub struct PlayerHudState {
    #[expose]
    #[node_ref(UiTextBlock)]
    label: NodeID,

    #[default(TextureID::nil())]
    icon: TextureID,
}
```

The editor uses the node ref hint for pick lists.

The runtime still resolves the id when the script uses it.

Use `ctx.res` for resource IDs and `ctx.run` for node IDs.

## Reserve + Drop

Reserve resources that must stay alive.

Drop or release resource refs when no longer needed.

Use resource docs for exact lifetime rules per type.

## Static Export

Static builds bake supported assets into generated lookup data.

Generic files go into `assets.perro`.

Use static export when release load speed matters and runtime parse cost should be paid at build time.

## Data Files

CSV works well for:

- balance data
- item tables
- localization tables
- spawn tables

Use resource APIs to load and query CSV.

## Reference

- [Resource Management](/docs/resources/resource_management.md)
- [ResPath](/docs/resources/respath.md)
- [Resource API](/docs/scripting/contexts/resource_api.md)
- [Textures Module](/docs/scripting/contexts/resource_modules/textures.md)
- [Meshes Module](/docs/scripting/contexts/resource_modules/meshes.md)
- [Materials Module](/docs/scripting/contexts/resource_modules/materials.md)
- [CSV Module](/docs/scripting/contexts/resource_modules/csv.md)
- [Shaders](/docs/resources/shaders.md)
