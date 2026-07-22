# Typed Assets

## Purpose

Store a resource ID in state when each script instance may choose a different
asset. Let the scene name the path and the loader resolve it before `on_init`.

## Data Flow

```text
scene path string -> Resource API cache -> stable typed ID -> state -> node/API
```

Scene injection resolves valid `res://`, `dlc://`, and `user://` paths into
`TextureID`, `MaterialID`, `MeshID`, `AnimationID`, `AnimationTreeID`,
`NavMeshID`, and `SoundFontID`. Resolution also works inside options, lists, map
values, tuples, and custom `#[derive(Variant)]` values.

Use typed state IDs for per-instance choices, cached resources used across
callbacks, and nested configuration. Keep a constant path only when every
instance truly uses the same asset and loading at the call site expresses the
desired lifetime.

## Failure Behavior

Invalid scene type/path keeps the field default. Loader failure keeps the
Resource API's normal nil/failure result. Guard optional or nil IDs before work
that requires a valid resource.

Runtime `set_var!` stays strict. It does not turn a path string into an asset ID.
Use a real typed ID for runtime assignment.

## Related

- [State And References](state_and_refs.md)
- [Scene-Injected Asset Variants](examples/asset_variants.md)
- [Resource context](../contexts/resource_api.md)

