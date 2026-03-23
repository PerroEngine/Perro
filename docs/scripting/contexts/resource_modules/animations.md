# Animations Module

Access:

- `res.Animations()`

Macros:

- `animation_load!(res, source) -> AnimationID`
- `animation_reserve!(res, source) -> AnimationID`
- `animation_drop!(res, source) -> bool`

Methods:

- `res.Animations().load(source) -> AnimationID`
- `res.Animations().reserve(source) -> AnimationID`
- `res.Animations().drop(source) -> bool`

## What `load` Does

- source string is trimmed and used as cache key
- empty source returns `AnimationID::nil()`
- if already loaded, returns existing `AnimationID`
- if not loaded, parses/loads clip and allocates a new `AnimationID`

## What `reserve` Does

- loads and keeps in memory

## What `drop` Does

- removes source-to-ID mapping
- removes stored clip data for that ID
- frees ID slot
- returns `false` if source is unknown/empty

## Example

```rust
let anim_id = animation_load!(res, "res://animations/hero_run.panim");
let _same = animation_reserve!(res, "res://animations/hero_run.panim");
let _ = animation_drop!(res, "res://animations/hero_run.panim");
```

## Typical Runtime Flow With `AnimationPlayer` (using RuntimeContext)

1. `animation_load!(res, source)` to get `AnimationID`
2. `anim_player_set_clip!(ctx, animation_player_id, animation_id)`
3. `anim_player_bind!(ctx, animation_player_id, ["ClipObjectName": target_node_id, ...])`
4. `anim_player_play!(ctx, animation_player_id)`

Binding note:

- `ClipObjectName` is the object key declared in `.panim [Objects]` (without `@`).
- Bind each clip object to a runtime node with the expected node type for that track data.
- scene authoring bindings use map entries: `{ ClipObjectName = SceneKey }` or `{ "ClipObjectName": SceneKey }`.
