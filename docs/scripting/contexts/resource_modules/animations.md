# Animations Module

Access:

- `res.Animations()`

Macros:

- `animation_load!(res, source) -> AnimationID`
- `animation_is_loaded!(res, animation_id) -> bool`
- `animation_tree_load!(res, source) -> AnimationTreeID`
- `animation_tree_is_loaded!(res, animation_tree_id) -> bool`
- `animation_reserve!(res, source) -> AnimationID`
- `animation_drop!(res, source) -> bool`

Methods:

- `res.Animations().load(source) -> AnimationID`
- `res.Animations().is_loaded(animation_id) -> bool`
- `res.Animations().reserve(source) -> AnimationID`
- `res.Animations().drop(source) -> bool`
- `res.AnimationTrees().load(source) -> AnimationTreeID`
- `res.AnimationTrees().is_loaded(animation_tree_id) -> bool`

## What `load` Does

- source string is trimmed and used as cache key
- empty source returns `AnimationID::nil()`
- if already loaded, returns existing `AnimationID`
- if not loaded, allocates a new `AnimationID` and queues parse/load work

## What `is_loaded` Does

- returns `true` once clip/tree data is ready
- returns `false` while pending, after drop, or for unknown/nil IDs
- use before starting animation playback when missing first frame would be visible
- use for load-screen progress or cutscene gates
- skip when player can idle/default pose until clip resolves

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
let ready = animation_is_loaded!(res, anim_id);
let tree_id = animation_tree_load!(res, "res://animations/hero.panimtree");
let tree_ready = animation_tree_is_loaded!(res, tree_id);
let _same = animation_reserve!(res, "res://animations/hero_run.panim");
let _ = animation_drop!(res, "res://animations/hero_run.panim");
```

## Typical Runtime Flow With `AnimationPlayer` (using RuntimeWindow)

1. `animation_load!(res, source)` to get `AnimationID`
2. `anim_player_set_clip!(ctx, animation_player_id, animation_id)`
3. `anim_player_bind!(ctx, animation_player_id, ["ClipObjectName": target_node_id, ...])`
4. `anim_player_play!(ctx, animation_player_id)`

Binding note:

- `ClipObjectName` is the object key declared in `.panim [Objects]` (without `@`).
- Bind each clip object to a runtime node with the expected node type for that track data.
- scene authoring bindings use map entries: `{ ClipObjectName = @SceneKey }` or `{ "ClipObjectName": @SceneKey }`.

