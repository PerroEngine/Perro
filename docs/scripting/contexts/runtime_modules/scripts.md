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

This runtime module belongs to `ctx.run` and documents scripts calls.

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

## API Reference

### `with_state`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn with_state<T: 'static, V: Default, F>(&mut self, script_id: NodeID, f: F) -> V where F: FnOnce(&T) -> V,` |
| Params | `&mut self, script_id: NodeID, f: F) -> V where F: FnOnce(&T` |
| Returns | `V where F: FnOnce(&T) -> V,` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().with_state(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `with_state_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn with_state_mut<T: 'static, V, F>(&mut self, script_id: NodeID, f: F) -> Option<V> where F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, script_id: NodeID, f: F) -> Option<V> where F: FnOnce(&mut T` |
| Returns | `Option<V> where F: FnOnce(&mut T) -> V,` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().with_state_mut(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `script_attach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_attach<P: ResPathSource>(&mut self, node_id: NodeID, script_path: P) -> bool` |
| Params | `&mut self, node_id: NodeID, script_path: P` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().script_attach(ctx.id, "res://path/to/resource");
        let _ = value;
    }
});
```

### `script_attach_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_attach_hashed(&mut self, node_id: NodeID, script_path_hash: u64) -> bool` |
| Params | `&mut self, node_id: NodeID, script_path_hash: u64` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().script_attach_hashed(ctx.id, 0);
        let _ = value;
    }
});
```

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn script_detach(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().script_detach(ctx.id);
        let _ = value;
    }
});
```

### `remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn remove(&mut self, script_id: NodeID) -> bool` |
| Params | `&mut self, script_id: NodeID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().remove(ctx.id);
        let _ = value;
    }
});
```

### `set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().set_update_enabled(ctx.id, true);
        let _ = value;
    }
});
```

### `set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_fixed_update_enabled(&mut self, script_id: NodeID, enabled: bool) -> bool` |
| Params | `&mut self, script_id: NodeID, enabled: bool` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().set_fixed_update_enabled(ctx.id, true);
        let _ = value;
    }
});
```

### `get_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn get_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M) -> Variant` |
| Params | `&mut self, script_id: NodeID, member: M` |
| Returns | `Variant` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().get_var(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `set_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn set_var<M: IntoScriptMemberID>(&mut self, script_id: NodeID, member: M, value: Variant)` |
| Params | `&mut self, script_id: NodeID, member: M, value: Variant` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().set_var(ctx.id, Default::default(), variant!(0_i32));
        let _ = value;
    }
});
```

### `call_method`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `pub fn call_method<M: IntoScriptMemberID>( &mut self, script_id: NodeID, method: M, params: &[Variant], ) -> Variant` |
| Params | `&mut self, script_id: NodeID, method: M, params: &[Variant],` |
| Returns | `Variant` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Scripts().call_method(ctx.id, Default::default(), variant!(0_i32));
        let _ = value;
    }
});
```

### `with_state`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `with_state!(ctx.run, state_ty, id, f)` |
| Params | `ctx, state_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = with_state!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `with_state_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `with_state_mut!(ctx.run, state_ty, id, f)` |
| Params | `ctx, state_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = with_state_mut!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `script_attach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_attach!(ctx.run, id, path)` |
| Params | `ctx, id, path` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = script_attach!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `script_detach`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_detach!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = script_detach!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `script_set_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = script_set_update_enabled!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `script_set_fixed_update_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `script_set_fixed_update_enabled!(ctx.run, id, enabled)` |
| Params | `ctx, id, enabled` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = script_set_fixed_update_enabled!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `get_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `get_var!(ctx.run, id, member)` |
| Params | `ctx, id, member` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = get_var!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `set_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `set_var!(ctx.run, id, member, value)` |
| Params | `ctx, id, member, value` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = set_var!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `call_method`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Scripts()` |
| Signature | `call_method!(ctx.run, id, method, params)` |
| Params | `ctx, id, method, params` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = call_method!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```
