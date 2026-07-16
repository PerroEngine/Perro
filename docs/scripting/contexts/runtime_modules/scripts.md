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

- Deal damage across scripts: `call_method!(ctx.run, player_id, method!("take_damage"), params![variant!(10.0_f32)])` from a trap or projectile.
- Read another script's field the typed way: pull the player's `health` with `with_state!(ctx.run, PlayerState, player_id, |s| s.health)`.
- Mutate your own state in place: `with_state_mut!(ctx.run, MyState, ctx.id, |s| s.ammo -= 1)`.
- Generic UI/tool access by name: `set_var!(ctx.run, id, var!("volume"), variant!(0.8_f32))` and `get_var!(ctx.run, id, var!("volume"))`.
- Follow a node-reference field: `get_node_var!(ctx.run, id, var!("target"))` returns the referenced `NodeID`.
- Add behaviour to a spawned node at runtime: `script_attach!(ctx.run, node_id, "res://scripts/enemy.rs")`; remove it with `script_detach!`.
- Freeze an NPC during a cutscene without destroying it: `script_set_update_enabled!(ctx.run, npc_id, false)`, re-enable afterwards.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Scripts()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A spike-trap script finds the player node and calls its `take_damage` method.
The trap does not need to know the player's script type or `#include` it: the
call resolves the method by name at runtime and returns the player's typed reply
wrapped in a `Variant`.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Some(player) = query_first!(ctx.run, any(name["Player"], tags["player"])) {
            let survived = call_method!(
                ctx.run,
                player,
                method!("take_damage"),
                params![variant!(25.0_f32)]
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
| Signature | `pub fn with_state<T: 'static, V: Default, F>(&mut self, script_id: NodeID, f: F) -> V where F: FnOnce(&T) -> V,` |
| Params | `&mut self, script_id: NodeID, f: F) -> V where F: FnOnce(&T` |
| Returns | `V where F: FnOnce(&T) -> V,` |
| Use when | Read typed state on this script or another known script type without dynamic `Variant` conversion. |
| Fails when / edge behavior | Returns `V::default()` when the script id is missing or the stored state type is not `T`. |

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
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_attach_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool` |
| Params | `&mut self, node_id: NodeID, script_path_hash: u64` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_detach(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn remove(&mut self, script_id: NodeID) -> bool` |
| Params | `&mut self, script_id: NodeID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_state_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `with_state_mut!(ctx.run, state_ty, id, f)` |
| Params | `ctx, state_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_attach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_attach!(ctx.run, id, path)` |
| Params | `ctx, id, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_detach!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `script_set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_fixed_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `get_var!(ctx.run, id, member)` |
| Params | `ctx, id, member` |
| Returns | `Variant` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `call_method`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `call_method!(ctx.run, id, method, params)` |
| Params | `ctx, id, method, params` |
| Returns | `Variant` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

