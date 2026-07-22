# Scripts Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `with_state` | [`with_state`](#with_state) |
| `with_state_mut` | [`with_state_mut`](#with_state_mut) |
| `script_attach` | [`script_attach`](#script_attach) |
| `script_attach_hashed` | [`script_attach_hashed`](#script_attach_hashed) |
| `script_detach` | [`script_detach`](#script_detach) |
| `remove` | [`remove`](#remove) |
| `set_update_enabled` | [`set_update_enabled`](#set_update_enabled) |
| `set_fixed_update_enabled` | [`set_fixed_update_enabled`](#set_fixed_update_enabled) |
| `get_var` | [`get_var`](#get_var) |
| `get_node_var` | [`get_node_var`](#get_node_var) |
| `set_var` | [`set_var`](#set_var) |
| `call_method` | [`call_method`](#call_method) |
| `with_state` | [`with_state`](#with_state) |
| `with_state_mut` | [`with_state_mut`](#with_state_mut) |
| `script_attach` | [`script_attach`](#script_attach) |
| `script_detach` | [`script_detach`](#script_detach) |
| `script_set_update_enabled` | [`script_set_update_enabled`](#script_set_update_enabled) |
| `script_set_fixed_update_enabled` | [`script_set_fixed_update_enabled`](#script_set_fixed_update_enabled) |
| `get_var` | [`get_var`](#get_var) |
| `set_var` | [`set_var`](#set_var) |
| `call_method` | [`call_method`](#call_method) |

## Purpose

The scripts module is how one script reaches another. It reads and writes the
typed state a script owns, calls its methods by name, and manages script
lifetime at runtime (attach, detach, enable/disable per-frame updates). This is
what lets a trap tell the player script to take damage, a manager read every
enemy's health, or a cutscene freeze an NPC's logic without deleting it.

`with_state!` / `with_state_mut!` give typed, allocation-free access when the
state type is known. `get_var!` / `set_var!` / `call_method!` work dynamically by
member name and go through `Variant`, for generic tools, UI, and animation
events that do not know the concrete script type.

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Known state type on self or another node | `with_state!` / `with_state_mut!` | Typed, allocation-free access with compiler-checked fields | Closure borrow must end before another `ctx.run` call |
| Trap asks one player to take damage | `call_method!` | Receiver owns damage rules and returns a result | Method name, argument order, and decode are runtime contracts |
| Tool knows `volume` by name, not state type | `get_var!` / `set_var!` | Dynamic member access fits adapters and tools | Strict type mismatch fails; it does not coerce asset paths |
| State member contains a dynamic node ref | `get_node_var!` | Decodes the node-ref member directly to `NodeID` | Missing/wrong member returns no usable target |
| Spawned node needs optional behavior selected at runtime | `script_attach!` / `script_detach!` | Script lifetime follows runtime composition | Attach path/build must exist; detach removes its owned state |
| Cutscene pauses one behavior without deleting data | update enable flags | State and node remain alive while callbacks stop | Signals/method calls may still reach the script; this is not full suspension |

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Scripts()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A spike-trap script reads its scene-wired player ref and calls `take_damage`.
The trap does not need to know the player's script type or `#include` it: the
call resolves the method by name at runtime and returns the player's typed reply
wrapped in a `Variant`.

```rust
#[State]
struct HazardState {
    #[expose]
    #[node_ref(Node2D, Node3D)]
    target: Option<NodeID>,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let target = with_state!(ctx.run, HazardState, ctx.id, |state| state.target).unwrap_or_default();

        if let Some(player) = target {
            let survived = call_method!(
                ctx.run,
                player,
                method!("take_damage"),
                params![25.0_f32]
            );
            // The called method returned a bool; decode the Variant reply.
            if let Some(false) = survived.as_bool() {
                signal_emit!(ctx.run, signal!("player_died"), params![]);
            }
        }
    }
});
```

`get_var!` and `call_method!` return `Variant` because member lookup is dynamic.

The called method can still return a primitive such as `bool`, `i32`, `f32`, or `String`.

Generated script glue wraps that typed return into `Variant`.

Decode with `as_*`, `parse::<T>()`, or `into_parse::<T>()`.

See [Variant](../../variant.md).

## API Reference

### `with_state`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn with_state<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V> where F: FnOnce(&T) -> V,` |
| Params | `&mut self, script_id: NodeID, f: F` |
| Returns | `Option<V>` |
| Use when | Read typed state on this script or another known script type without dynamic `Variant` conversion. |
| Fails when / edge behavior | Returns `None` when the script id is missing or the stored state type is not `T`. |

### `with_state_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V> where F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, script_id: NodeID, f: F) -> Option<V> where F: FnOnce(&mut T` |
| Returns | `Option<V> where F: FnOnce(&mut T) -> V,` |
| Use when | Mutate typed state in place while keeping the mutable borrow inside one closure. |
| Fails when / edge behavior | Returns `None` when the script id is missing or the stored state type is not `T`. |

### `script_attach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_attach<P: ResPathSource>(&mut self, node_id: NodeID, script_path: P) -> bool` |
| Params | `&mut self, node_id: NodeID, script_path: P` |
| Returns | `bool` |
| Use when | Add or replace one node's behavior from a runtime-selected script path. The new script gets default state, runs `on_init` synchronously, then joins queued `on_all_init`/update work. |
| Fails when / edge behavior | Returns `false` for a missing node, missing script constructor, or failed attach. No scene vars are accepted. |

### `script_attach_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool` |
| Params | `&mut self, node_id: NodeID, script_path_hash: u64` |
| Returns | `bool` |
| Use when | Add or replace behavior when the script path hash is already available. Init order and default-state behavior match `script_attach`. |
| Fails when / edge behavior | Returns `false` when the node or registered script hash is missing or attach fails. It accepts no scene vars. |

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_detach(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use `script_detach` to script detach for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `script_detach` cannot apply to the supplied target or inputs; `true` confirms success. |

### `remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn remove(&mut self, script_id: NodeID) -> bool` |
| Params | `&mut self, script_id: NodeID` |
| Returns | `bool` |
| Use when | Use `remove` to remove across runtime scripts; prefer typed state access when concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `remove` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use `set_update_enabled` to set update enabled across runtime scripts; prefer typed state access when concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `set_update_enabled` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use `set_fixed_update_enabled` to set fixed update enabled across runtime scripts; prefer typed state access when concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `set_fixed_update_enabled` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn get_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M) -> Variant` |
| Params | `&mut self, script_id: NodeID, member: M` |
| Returns | `Variant` |
| Use when | Read another script's field dynamically by member name/hash, such as UI, animation event, or generic tool code. |
| Fails when / edge behavior | Returns `Variant::Null` when the script id or member does not resolve. |

### `set_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M, value: Variant)` |
| Params | `&mut self, script_id: NodeID, member: M, value: Variant` |
| Returns | `()` |
| Use when | Write another script's field dynamically from scene events, animation events, UI, or cross-script code. |
| Fails when / edge behavior | No-op when the script id or member does not resolve, or when `Variant` cannot parse into the field type. |

### `call_method`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn call_method<M: IntoScriptMemberID>( &mut self, script_id: NodeID, method: M, params: &[Variant], ) -> Variant` |
| Params | `&mut self, script_id: NodeID, method: M, params: &[Variant],` |
| Returns | `Variant` |
| Use when | Call self or another script dynamically by method name/hash; prefer direct Rust helper calls for known same-script logic. |
| Fails when / edge behavior | Returns `Variant::Null` when the script id, method, or params do not resolve. Primitive method returns are wrapped into `Variant`. |

### `with_state`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `with_state!(ctx.run, state_ty, id, f)` |
| Params | `ctx, state_ty, id, f` |
| Returns | `Option<V>` with the closure result |
| Use when | Use `with_state` to with state for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `None` for a missing ID or wrong state type. |

### `with_state_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `with_state_mut!(ctx.run, state_ty, id, f)` |
| Params | `ctx, state_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use `with_state_mut` to with state mut for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Uses the backing `with_state_mut` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `script_attach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_attach!(ctx.run, id, path)` |
| Params | `ctx, id, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Macro form for path-based runtime attach/replacement; use an explicit init method after attach when post-`on_init` configuration is acceptable. |
| Fails when / edge behavior | Returns the backing attach `bool`; `false` means the node/script could not attach. No scene vars are applied. |

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_detach!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `script_detach` to script detach for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `script_detach` cannot apply to the supplied target or inputs; `true` confirms success. |

### `script_set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `script_set_update_enabled` to script set update enabled for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `script_set_update_enabled` cannot apply to the supplied target or inputs; `true` confirms success. |

### `script_set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_fixed_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `script_set_fixed_update_enabled` to script set fixed update enabled for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `script_set_fixed_update_enabled` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `get_var!(ctx.run, id, member)` |
| Params | `ctx, id, member` |
| Returns | `Variant` |
| Use when | Use `get_var` to get var across runtime scripts; prefer typed state access when concrete state type is known. |
| Fails when / edge behavior | Returns `Variant::Nil` when `get_var` cannot resolve the requested dynamic member or call result. |

### `get_node_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `get_node_var!(ctx.run, id, member) -> NodeID` |
| Params | `ctx, id, member` |
| Returns | `NodeID` |
| Use when | Use to read a node-ref script var (`NodeScriptVar::NodeRef`) back as a `NodeID` without manual `Variant::as_node` unwrapping. |
| Fails when / edge behavior | Returns `NodeID::nil()` when the var is missing or is not a node reference. |

### `set_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `set_var!(ctx.run, id, member, value)` |
| Params | `ctx, id, member, value` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_var` to set var across runtime scripts; prefer typed state access when concrete state type is known. |
| Fails when / edge behavior | Returns `false` when `set_var` cannot apply to the supplied target or inputs; `true` confirms success. |

### `call_method`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `call_method!(ctx.run, id, method, params)` |
| Params | `ctx, id, method, params` |
| Returns | `Variant` |
| Use when | Use `call_method` to call method for runtime script composition; prefer typed state access when the concrete state type is known. |
| Fails when / edge behavior | Returns `Variant::Nil` when `call_method` cannot resolve the requested dynamic member or call result. |

