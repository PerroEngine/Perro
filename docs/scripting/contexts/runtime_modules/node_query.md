# Node Query Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `query` | [`query`](#query) |
| `query_view` | [`query_view`](#query_view) |
| `query_expr` | [`query_expr`](#query_expr) |
| `query_builder` | [`query_builder`](#query_builder) |
| `query` | [`query`](#query) |
| `query_first` | [`query_first`](#query_first) |

## Overview

This runtime module belongs to `ctx.run` and documents node query calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.NodeQuery()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `query`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `pub fn query(&mut self, query: &NodeQuery) -> Vec<NodeID>` |
| Params | `&mut self, query: &NodeQuery` |
| Returns | `Vec<NodeID>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.NodeQuery().query(0.1);
        let _ = value;
    }
});
```

### `query_view`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `pub fn query_view(&mut self, query: NodeQueryView<'_>) -> Vec<NodeID>` |
| Params | `&mut self, query: NodeQueryView<'_>` |
| Returns | `Vec<NodeID>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.NodeQuery().query_view(0.1);
        let _ = value;
    }
});
```

### `query_expr`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_expr!(kind args $(,)?)` |
| Params | `kind args $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = query_expr!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `query_builder`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_builder!(kind args, in_subtree(parent) $(,)?)` |
| Params | `kind args, in_subtree(parent) $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = query_builder!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `query`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query!(ctx.run, tags[$(tag)*], in_subtree(parent) $(,)?)` |
| Params | `ctx, tags[$(tag)*], in_subtree(parent) $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = query!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `query_first`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_first!(ctx.run, kind args, in_subtree(parent) $(,)?)` |
| Params | `ctx, kind args, in_subtree(parent) $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = query_first!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```
