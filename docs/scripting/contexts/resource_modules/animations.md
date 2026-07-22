# Animations Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| Animations: `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `reserve` | [`reserve`](#reserve) |
| `reserve_hashed` | [`reserve_hashed`](#reserve_hashed) |
| `reserve_hashed_with_source` | [`reserve_hashed_with_source`](#reserve_hashed_with_source) |
| `drop` | [`drop`](#drop) |
| `get` | [`get`](#get) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `animation_load` | [`animation_load`](#animation_load) |
| `animation_reserve` | [`animation_reserve`](#animation_reserve) |
| `animation_drop` | [`animation_drop`](#animation_drop) |
| `animation_is_loaded` | [`animation_is_loaded`](#animation_is_loaded) |
| Trees: `load` | [`load`](#load-1) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source-1) |
| `get` | [`get`](#get-1) |
| `is_loaded` | [`is_loaded`](#is_loaded-1) |
| `animation_tree_load` | [`animation_tree_load`](#animation_tree_load) |
| `animation_tree_is_loaded` | [`animation_tree_is_loaded`](#animation_tree_is_loaded) |

## Purpose

`ctx.res.Animations()` loads animation clips (`.panim`) and `ctx.res.AnimationTrees()` loads blend trees (`.panimtree`) into IDs that animation-player and skeleton nodes reference. Loads return an ID immediately. Load clips so a character can play walk, run, and attack, and load a tree when transitions and blends between those clips should be data-driven rather than hand-coded.

## Use Cases

- Character move sets: load each clip (`animation_load!(ctx.res, "res://anims/hero_run.panim")`) and switch between them from gameplay state.
- Data-driven locomotion: load an animation tree with `animation_tree_load!` so idle/walk/run blends by speed without per-clip transition code.
- On-demand emotes or cutscene poses: load a clip only when the emote is triggered, then check `animation_is_loaded!` before playing.
- Inspecting a clip: `get` returns `Option<Arc<AnimationClip>>` to read duration or track info before use.
- Streaming rigs from network/save: `animation_create_from_bytes!` and `animation_tree_create_from_bytes!` decode in-memory `.panim` / `.panimtree` data.
- Preloading and freeing: `animation_reserve!` to pin a clip, `animation_drop!` to release it.

## Ownership And Choice

Animation resources own clips and trees; player/skeleton nodes own current playback state. Inject `AnimationID` / `AnimationTreeID` for authored per-instance choices. Load at runtime for downloaded or selected content. Use a tree when transitions and blends are data-driven; use direct clips for isolated actions where a state machine adds no value.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Animations()` and `ctx.res.AnimationTrees()`
- Asset types: `perro_animation::AnimationClip`, `perro_animation::AnimationTreeAsset`.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when animation data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Animations().create_from_bytes(bytes)` | `AnimationID` | Decodes `.panim` text. |
| `ctx.res.AnimationTrees().create_from_bytes(bytes)` | `AnimationTreeID` | Decodes `.panimtree` text. |
| `animation_create_from_bytes!(ctx.res, bytes)` | `AnimationID` | Macro form. |
| `animation_tree_create_from_bytes!(ctx.res, bytes)` | `AnimationTreeID` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Load a run clip at init and confirm it is ready before a helper would play it.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let run = animation_load!(ctx.res, "res://anims/hero_run.panim");
        self.on_loaded(ctx, run);
    }
});

methods!({
    fn on_loaded(&self, ctx: &mut ScriptContext<'_, API>, run: AnimationID) {
        if animation_is_loaded!(ctx.res, run) {
            if let Some(clip) = ctx.res.Animations().get(run) {
                let _ = clip; // read duration / tracks, then drive a player node
            }
        }
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> AnimationID` |
| Params | `source: S` |
| Returns | `AnimationID` |
| Use when | Loading a clip by path. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing or invalid. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> AnimationID` |
| Params | `source_hash: u64` |
| Returns | `AnimationID` |
| Use when | A precomputed path hash is available. |
| Fails when / edge behavior | Returns a nil `AnimationID` when no clip is registered for the hash. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> AnimationID` |
| Params | `source_hash: u64, source: S` |
| Returns | `AnimationID` |
| Use when | The `animation_load!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing. |

### `reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve<S: ResPathSource>(&self, source: S) -> AnimationID` |
| Params | `source: S` |
| Returns | `AnimationID` |
| Use when | Pinning a clip so it stays resident. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing. |

### `reserve_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve_hashed(&self, source_hash: u64) -> AnimationID` |
| Params | `source_hash: u64` |
| Returns | `AnimationID` |
| Use when | Reserving by a precomputed path hash. |
| Fails when / edge behavior | Returns a nil `AnimationID` when no clip is registered for the hash. |

### `reserve_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn reserve_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> AnimationID` |
| Params | `source_hash: u64, source: S` |
| Returns | `AnimationID` |
| Use when | The `animation_reserve!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing. |

### `drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn drop(&self, id: AnimationID) -> bool` |
| Params | `id: AnimationID` |
| Returns | `bool` |
| Use when | Releasing a clip the game no longer needs. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn get(&self, id: AnimationID) -> Option<Arc<AnimationClip>>` |
| Params | `id: AnimationID` |
| Returns | `Option<Arc<AnimationClip>>` |
| Use when | Reading a clip's data (duration, tracks) without owning it. |
| Fails when / edge behavior | Returns `None` when the clip is not loaded or the ID is unknown. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `pub fn is_loaded(&self, id: AnimationID) -> bool` |
| Params | `id: AnimationID` |
| Returns | `bool` |
| Use when | Polling whether the clip finished loading. |
| Fails when / edge behavior | Returns `false` while loading is pending or when the ID is unknown. |

### `animation_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `animation_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `AnimationID` |
| Use when | Macro form of `load`. A literal path hashes at compile time; an expression path calls `load`. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing. |

### `animation_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `animation_reserve!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `AnimationID` |
| Use when | Macro form of `reserve`. |
| Fails when / edge behavior | Returns a nil `AnimationID` when the file is missing. |

### `animation_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `animation_drop!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `drop`. |
| Fails when / edge behavior | Returns `false` when the ID is unknown or already dropped. |

### `animation_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Animations()` |
| Signature | `animation_is_loaded!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `is_loaded`. |
| Fails when / edge behavior | Returns `false` while loading is pending or when the ID is unknown. |

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> AnimationTreeID` |
| Params | `source: S` |
| Returns | `AnimationTreeID` |
| Use when | Loading a `.panimtree` blend tree by path. |
| Fails when / edge behavior | Returns a nil `AnimationTreeID` when the file is missing or invalid. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> AnimationTreeID` |
| Params | `source_hash: u64, source: S` |
| Returns | `AnimationTreeID` |
| Use when | The `animation_tree_load!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `AnimationTreeID` when the file is missing. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn get(&self, id: AnimationTreeID) -> Option<Arc<AnimationTreeAsset>>` |
| Params | `id: AnimationTreeID` |
| Returns | `Option<Arc<AnimationTreeAsset>>` |
| Use when | Reading a blend tree's data without owning it. |
| Fails when / edge behavior | Returns `None` when the tree is not loaded or the ID is unknown. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `pub fn is_loaded(&self, id: AnimationTreeID) -> bool` |
| Params | `id: AnimationTreeID` |
| Returns | `bool` |
| Use when | Polling whether the tree finished loading. |
| Fails when / edge behavior | Returns `false` while loading is pending or when the ID is unknown. |

### `animation_tree_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `animation_tree_load!(ctx.res, source)` |
| Params | `ctx.res, source` |
| Returns | `AnimationTreeID` |
| Use when | Macro form of `AnimationTrees().load`. |
| Fails when / edge behavior | Returns a nil `AnimationTreeID` when the file is missing. |

### `animation_tree_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.AnimationTrees()` |
| Signature | `animation_tree_is_loaded!(ctx.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool` |
| Use when | Macro form of `AnimationTrees().is_loaded`. |
| Fails when / edge behavior | Returns `false` while loading is pending or when the ID is unknown. |
