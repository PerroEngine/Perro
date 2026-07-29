# Assets + Resources

Resources are loaded through `ctx.res` and used by scene nodes or scripts.

## Goal

Load and manage textures, meshes, materials, audio, animation data, CSV, shaders, and scene data.

## Ownership Model

Put a per-instance asset choice in typed state and inject its path from the
scene. Keep a literal path in code only when every instance must use the same
asset. Load at runtime when the path is discovered, downloaded, or selected by
player data.

This separates authored choice from resource lifetime: scene injection resolves
the typed ID before `on_init`, while the resource cache controls reuse and load
state.

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

## Loading Screens

Assigning an ID before the load finishes is normal. Do not poll for it.

One case does need the wait: a transition the player watches.

A mesh whose mesh or material is still loading is skipped, not drawn late.

Dismiss a loading screen early and the level appears with holes in it.

Ask the scene graph instead of tracking a list:

```rust
if scene_assets_ready!(ctx.run) {
    // every mesh draw in the live graph can render
}
```

`scene_asset_progress!(ctx.run)` returns `(pending, total)` for a loading bar.

That only sees nodes that exist. For a scene not spawned yet, warm the paths
with `mesh_reserve!` and poll `mesh_is_loaded!`.

Always bound the wait by frames and start anyway.

A missing file re-requests forever, and a hang is worse than pop-in.

See [Loading Gates](/docs/resources/resource_management.md#loading-gates).

## Scenes And Refs

Scene files can wire script vars.

Use `NodeID` for scene node refs.

Use `#[node_ref(...)]` when a state field expects a node type.

```rust
#[State]
pub struct PlayerHudState {
    #[expose]
    #[node_ref(UiTextBlock)]
    label: Option<NodeID>,

    #[expose]
    icon: TextureID,
}
```

The editor uses the node ref hint for pick lists.

The runtime still resolves the id when the script uses it.

Scene `script_vars` also accept resource paths for typed asset-ID fields:

```text
script_vars = {
    label = @ScoreLabel,
    icon = "res://ui/player_icon.png"
}
```

Resolution happens before `on_init`. Reusing the same path uses the normal
resource cache. Decode failure keeps the field default. Asset load failure uses
the resource module's normal nil/failure behavior.

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
- [Scenes Module](/docs/scripting/contexts/runtime_modules/scenes.md)
- [ResPath](/docs/resources/respath.md)
- [Resource API](/docs/scripting/contexts/resource_api.md)
- [Textures Module](/docs/scripting/contexts/resource_modules/textures.md)
- [Meshes Module](/docs/scripting/contexts/resource_modules/meshes.md)
- [Materials Module](/docs/scripting/contexts/resource_modules/materials.md)
- [CSV Module](/docs/scripting/contexts/resource_modules/csv.md)
- [Shaders](/docs/resources/shaders.md)
