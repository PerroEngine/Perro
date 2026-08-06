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
| `preload_ready` | [`preload_ready`](#preload_ready) |
| `preload_pending` | [`preload_pending`](#preload_pending) |
| `load_preloaded` | [`load_preloaded`](#load_preloaded) |
| `free_preloaded` | [`free_preloaded`](#free_preloaded) |
| `drop_preloaded` | [`drop_preloaded`](#drop_preloaded) |
| `drop_preloaded_hashed` | [`drop_preloaded_hashed`](#drop_preloaded_hashed) |
| `asset_progress` | [`asset_progress`](#asset_progress) |
| `assets_pending` | [`assets_pending`](#assets_pending) |
| `assets_ready` | [`assets_ready`](#assets_ready) |
| `scene_load` | [`scene_load`](#scene_load) |
| `scene_preload` | [`scene_preload`](#scene_preload) |
| `scene_free_preloaded` | [`scene_free_preloaded`](#scene_free_preloaded) |
| `scene_drop_preloaded` | [`scene_drop_preloaded`](#scene_drop_preloaded) |
| `scene_asset_progress` | [`scene_asset_progress`](#scene_asset_progress) |
| `scene_assets_pending` | [`scene_assets_pending`](#scene_assets_pending) |
| `scene_assets_ready` | [`scene_assets_ready`](#scene_assets_ready) |

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
- Preloading never costs the calling frame: `scene_preload!(ctx.run, "res://levels/boss.pscene")` returns a handle immediately and parses + prepares the scene on a worker thread, like mesh/material/texture loads. Check `ctx.run.Scene().preload_ready(handle)` on a later frame, then `load_preloaded`.
- Spawn a prefab instance (enemy squad, pickup, particle burst): `scene_load!` a small scene and reparent its root under a spawn-point node.
- Main-menu "Play": load the first gameplay scene from the button handler.
- Reclaim memory once an area is behind the player: `scene_free_preloaded!(ctx.run, "res://levels/boss.pscene")` or `scene_drop_preloaded!`.
- Hold a loading screen until the swapped-in level can actually draw: `scene_assets_ready!(ctx.run)`. A mesh whose material is still resolving is skipped, not drawn late, so dismissing the cover early shows a world with holes in it. Bound the wait by frames and fall through, or a bad asset hangs the transition. See [Loading Gates](../../../resources/resource_management.md#loading-gates).
- Drive a loading bar: `scene_asset_progress!(ctx.run)` returns `(pending, total)` mesh draws, so the ratio needs no separate counter.

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
    // Connected to the exit trigger's "body_entered" signal; pub so it can dispatch.
    pub fn on_exit_reached(&self, ctx: &mut ScriptContext<'_, API>) {
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
| Fails when / edge behavior | Returns `Err` when `load` cannot validate or complete the operation; preserve the error text for diagnostics. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_hashed(&mut self, path_hash: u64, path: &str) -> Result<NodeID, String>` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `Err` when `load_hashed` cannot validate or complete the operation; preserve the error text for diagnostics. |

### `preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload<P: IntoScenePath>(&mut self, path: P) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path: P` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when a scene should be warmed ahead of the swap. Returns the handle immediately; parse + prepare run on a worker. |
| Fails when / edge behavior | Does not block and does not report load errors here: a failed load logs and leaves the handle not-ready. Repeat calls for one path share a handle. `load_preloaded` on a handle that is still loading waits for it. |

### `preload_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload_hashed( &mut self, path_hash: u64, path: &str, ) -> Result<PreloadedSceneID, String>` |
| Params | `&mut self, path_hash: u64, path: &str,` |
| Returns | `Result<PreloadedSceneID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `Err` when `preload_hashed` cannot validate or complete the operation; preserve the error text for diagnostics. |

### `preload_ready`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload_ready<I: IntoPreloadedSceneID>(&self, id: I) -> bool` |
| Params | `&self, id: I` |
| Returns | `bool` |
| Use when | Use when deciding whether a `preload` handle can be loaded without waiting on the worker. |
| Fails when / edge behavior | False for unknown, dropped, still-loading, and failed handles alike. |

### `preload_pending`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn preload_pending<I: IntoPreloadedSceneID>(&self, id: I) -> bool` |
| Params | `&self, id: I` |
| Returns | `bool` |
| Use when | Use when a loading screen needs to tell "still working" apart from "finished or failed". |
| Fails when / edge behavior | False once the worker reports back, whether it succeeded or failed. |

### `load_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn load_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> Result<NodeID, String>` |
| Params | `&mut self, id: I` |
| Returns | `Result<NodeID, String>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `Err` when `load_preloaded` cannot validate or complete the operation; preserve the error text for diagnostics. |

### `free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn free_preloaded<I: IntoPreloadedSceneID>(&mut self, id: I) -> bool` |
| Params | `&mut self, id: I` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `false` when `free_preloaded` cannot apply to the supplied target or inputs; `true` confirms success. |

### `drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded<T: IntoPreloadedSceneTarget>(&mut self, target: T) -> bool` |
| Params | `&mut self, target: T` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `false` when `drop_preloaded` cannot apply to the supplied target or inputs; `true` confirms success. |

### `drop_preloaded_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn drop_preloaded_hashed(&mut self, path_hash: u64, path: &str) -> bool` |
| Params | `&mut self, path_hash: u64, path: &str` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `false` when `drop_preloaded_hashed` cannot apply to the supplied target or inputs; `true` confirms success. |

### `asset_progress`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn asset_progress(&mut self) -> (u32, u32)` |
| Params | `&mut self` |
| Returns | `(pending, total)` mesh draws in the live scene graph |
| Use when | Gating a transition or driving a loading bar. `pending` counts mesh draws the renderer must skip because the mesh or a surface material is still resolving; such a draw contributes no geometry rather than appearing late. |
| Fails when / edge behavior | Never fails. Counts only what is instantiated, so assets for a scene you have not spawned yet read as ready; warm those with `mesh_reserve!` and poll `mesh_is_loaded!` alongside this. Walks every node, so poll it during loading rather than every frame of gameplay. |

### `assets_pending`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn assets_pending(&mut self) -> u32` |
| Params | `&mut self` |
| Returns | `u32` |
| Use when | Only the pending count matters, typically for a log line naming how many draws are still blocked. |
| Fails when / edge behavior | Same scan and same limits as `asset_progress`; drops the total. |

### `assets_ready`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `pub fn assets_ready(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | The gate condition itself: true when every mesh draw in the live graph can render. |
| Fails when / edge behavior | A missing or failed asset re-requests forever, so this can stay false indefinitely. Bound the wait by frames and start anyway with a log; a hang is worse than pop-in. |

### `scene_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_load!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Uses the backing `scene_load` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `scene_preload`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_preload!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Uses the backing `scene_preload` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `scene_free_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_free_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Uses the backing `scene_free_preloaded` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `scene_drop_preloaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_drop_preloaded!(ctx.run, path)` |
| Params | `ctx, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns `false` when `scene_drop_preloaded` cannot apply to the supplied target or inputs; `true` confirms success. |

### `scene_asset_progress`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_asset_progress!(ctx.run)` |
| Params | `ctx` |
| Returns | `(u32, u32)` |
| Use when | Wraps `asset_progress` for loading bars and transition gates. |
| Fails when / edge behavior | Uses the backing `asset_progress` return and behavior unchanged; the wrapper adds no coercion or fallback. |

### `scene_assets_pending`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_assets_pending!(ctx.run)` |
| Params | `ctx` |
| Returns | `u32` |
| Use when | Wraps `assets_pending` for diagnostics. |
| Fails when / edge behavior | Uses the backing `assets_pending` return and behavior unchanged; the wrapper adds no coercion or fallback. |

### `scene_assets_ready`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scene()` |
| Signature | `scene_assets_ready!(ctx.run)` |
| Params | `ctx` |
| Returns | `bool` |
| Use when | Wraps `assets_ready` as the gate condition. |
| Fails when / edge behavior | Uses the backing `assets_ready` return and behavior unchanged; the wrapper adds no coercion or fallback. |

