# Scripts Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

## Overview

This runtime module belongs to `ctx.run` and documents script state and method calls.

Use it for:

- typed self-state access through `with_state!` and `with_state_mut!`
- dynamic field access through `get_var!` and `set_var!`
- self or cross-script dynamic method calls through `call_method!`

Source path:

- `perro_source/api_modules/perro_runtime_api/src/sub_apis/script.rs`
- `perro_source/runtime_project/perro_runtime/src/rt_ctx/scripts.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs`

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Scripts()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let result = call_method!(ctx.run, ctx.id, method!("ping"), params![]);
        let _ = result;
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

