# State And References

## Purpose And Mental Model

State is per-script-instance memory. Put a value here when one callback writes
it and a later callback must read it, or when a scene must choose it per
instance.

```text
scene/default -> #[State] instance -> lifecycle/method callbacks
```

## Put Values In `#[State]`

Use `#[State]` for data that belongs to one script instance and must survive a
callback:

- mutable gameplay values such as health, velocity, or mode
- cached runtime values used by later callbacks
- fixed node dependencies as `NodeID` or `Option<NodeID>`
- per-instance resources as typed IDs such as `TextureID` or `MeshID`

Keep constants as Rust constants. Keep one-callback results as local vars.

Do not use state as a global registry by default. Put scene-wide flow on its
controller and shared immutable values in Rust modules/resources.

```rust
#[derive(Clone, Default, Variant)]
struct CharacterLook {
    portrait: TextureID,
    materials: Vec<MaterialID>,
}

#[State]
struct PlayerState {
    #[default = 100]
    #[expose]
    health: i32,

    #[expose]
    #[node_ref(Camera3D)]
    camera: Option<NodeID>,

    #[expose]
    look: CharacterLook,

    velocity: Vector3,
}
```

## `#[expose]` Only Organizes The Inspector

`#[expose]` controls editor inspector layout. It does not gate runtime access
or scene injection. Any state field may receive a scene `script_vars` value.

```text
script_vars = {
    health = 125,
    camera = @MainCamera,
    look = {
        portrait = "res://textures/player.png",
        materials = ["res://materials/body.pmat", "res://materials/trim.pmat"]
    }
}
```

## Asset Path Coercion

Scene strings coerce to `TextureID`, `MaterialID`, `MeshID`, `AnimationID`,
`AnimationTreeID`, `NavMeshID`, and `SoundFontID` before `on_init`.

Coercion recurses through options, lists, map values, tuples, and custom
`#[derive(Variant)]` values. Missing or invalid values keep the field default.
Resource load failure keeps each resource API's normal nil/failure result.

Runtime `set_var!` stays strict. It expects the field's runtime `Variant` type;
it does not load an asset from a path string.

## Missing References

Use `Option<NodeID>` when a scene ref may be absent. A referenced node may also
be removed later. Skip absent targets without a panic or required log.

```rust
let camera = with_state!(ctx.run, PlayerState, ctx.id, |state| state.camera);

if let Some(camera) = camera {
    with_node_mut!(ctx.run, Camera3D, camera, |node| {
        node.fov = 70.0;
    });
}
```

Copy the ID out before node access. A non-nil ID may still refer to a node
removed after injection, so typed access remains failure-tolerant.

## Related

- [Ownership And Scene Wiring](ownership_and_scene_wiring.md)
- [Typed Assets](typed_assets.md)
- [State API](../state.md)

[Back To Guide](index.md)
