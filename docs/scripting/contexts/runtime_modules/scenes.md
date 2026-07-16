# Scenes Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `preload` | [`preload`](#preload) |
| `preload_hashed` | [`preload_hashed`](#preload_hashed) |
| `load_preloaded` | [`load_preloaded`](#load_preloaded) |
| `free_preloaded` | [`free_preloaded`](#free_preloaded) |
| `drop_preloaded` | [`drop_preloaded`](#drop_preloaded) |
| `drop_preloaded_hashed` | [`drop_preloaded_hashed`](#drop_preloaded_hashed) |
| `scene_load` | [`scene_load`](#scene_load) |
| `scene_preload` | [`scene_preload`](#scene_preload) |
| `scene_free_preloaded` | [`scene_free_preloaded`](#scene_free_preloaded) |
| `scene_drop_preloaded` | [`scene_drop_preloaded`](#scene_drop_preloaded) |

## Purpose

The scenes module instances and swaps `.pscene` files while the game runs. This
is how you move from a menu into gameplay, transition between levels, and spawn
prefab instances such as enemy waves or destructible props. Loading returns the
`NodeID` of the new subtree's root, so gameplay code can immediately parent,
position, or configure what it just spawned. Preloading warms a scene off the
hot path so the actual swap does not hitch mid-action.

## Use Cases

- Level transition when the player reaches an exit: `scene_load!(ctx.run, "res://levels/level2.pscene")` returns the new root `NodeID`.
- Seamless streaming: `scene_preload!(ctx.run, "res://levels/boss.pscene")` during a calm corridor, then instance the warmed copy with `ctx.run.Scene().load_preloaded(id)` at the boss door.
- Spawn a prefab instance (enemy squad, pickup, particle burst): `scene_load!` a small scene and reparent its root under a spawn-point node.
- Main-menu "Play": load the first gameplay scene from the button handler.
- Reclaim memory once an area is behind the player: `scene_free_preloaded!(ctx.run, "res://levels/boss.pscene")` or `scene_drop_preloaded!`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Scene()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Preload the next level at startup, then swap to it when a door-trigger signal
fires. `scene_load!` and `load_preloaded` return `Result<NodeID, String>`, so
handle the error case.

```rust
#[State]
struct DoorState {
    #[default = NodeID::nil()]
    pub next_area: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        // Warm the next level so the transition does not stutter.
        let _ = scene_preload!(ctx.run, "res://levels/level2.pscene");
    }
});

methods!({
    // Connected to the exit trigger's "body_entered" signal.
    fn on_exit_reached(&self, ctx: &mut ScriptContext<'_, API>) {
        match scene_load!(ctx.run, "res://levels/level2.pscene") {
            Ok(root) => {
                with_state_mut!(ctx.run, DoorState, ctx.id, |state| state.next_area = root);
            }
            Err(err) => {
                let _ = err; // log or fall back to a safe scene
            }
        }
    }
});
```

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load<S: IntoSceneLoadSource>(&mut self, source: S) -> Result<NodeID, String>` |
| Params | `&mut self, source: S` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String>` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload<P: IntoScenePath>(&mut self, path: P) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path: P` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `preload_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload_hashed( &mut self, path_hash: u64, path: &str, ) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path_hash: u64, path: &str,` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> Result<NodeID, String>` |
| Params | `&mut self, id: I` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn free_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> bool` |
| Params | `&mut self, id: I` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded<T: IntoPreloadedSceneTarget>(&mut self, target: T) -> bool` |
| Params | `&mut self, target: T` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `drop_preloaded_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded_hashed(&mut self, path_hash: u64, path: &str) -> bool` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `scene_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_load!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `scene_preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_preload!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `scene_free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_free_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `scene_drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_drop_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

