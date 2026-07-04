# Node Query Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `query` | [`query`](#query) |
| `query_iter` | [`query_iter`](#query_iter) |
| `query_view` | [`query_view`](#query_view) |
| `query_expr` | [`query_expr`](#query_expr) |
| `query_builder` | [`query_builder`](#query_builder) |
| `query` | [`query`](#query) |
| `query_iter` | [`query_iter`](#query_iter-1) |
| `query_each` | [`query_each`](#query_each) |
| `query_map` | [`query_map`](#query_map) |
| `query_first` | [`query_first`](#query_first) |

## Overview

This runtime module belongs to `ctx.run` and documents node query calls.
Use it when game code needs dynamic node groups by tag, name, type, subtree, render layer, or spatial bounds (`within[origin, size]`).

Prefer macros for script code:

- `query!` when full `Vec<NodeID>` is useful.
- `query_iter!` when iterator adapters make code cleaner.
- `query_each!` when you only need a side effect per node.
- `query_map!` when you want derived values in a `Vec`.
- `query_first!` when one node is enough.

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
| Use when | Use when code already has a reusable `NodeQuery` and needs the full ID list. |
| Fails when / edge behavior | Query misses return an empty `Vec`. Returned IDs can become stale if nodes are removed later. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let q = query_builder!(all(tags["enemy"], not(tags["dead"])));
        let ids = ctx.run.NodeQuery().query(&q);
        let _ = ids.len();
    }
});
```

### `query_iter`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `pub fn query_iter(&mut self, query: &NodeQuery) -> std::vec::IntoIter<NodeID>` |
| Params | `&mut self, query: &NodeQuery` |
| Returns | `std::vec::IntoIter<NodeID>` |
| Use when | Use when you already have a reusable `NodeQuery` and want iterator adapters. |
| Fails when / edge behavior | Query misses return an empty iterator. This still allocates the same owned `Vec<NodeID>` as `query`. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let q = query_builder!(all(tags["pickup"], not(tags["claimed"])));
        let first_three = ctx.run.NodeQuery().query_iter(&q).take(3).collect::<Vec<_>>();
        let _ = first_three;
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
| Use when | Use internally for borrowed query views and temporary subtree overrides. Most script code should use macros. |
| Fails when / edge behavior | Query misses return an empty `Vec`. Returned IDs can become stale if nodes are removed later. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let q = query_builder!(all(tags["enemy"]));
        let ids = ctx.run.NodeQuery().query_view(q.as_view());
        let _ = ids;
    }
});
```

### `query_expr`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_expr!(kind args $(,)?)` |
| Params | `kind args $(,)?` |
| Returns | `QueryExpr` |
| Use when | Use when building query expressions for reuse or for adding predicates conditionally. |
| Fails when / edge behavior | Compile errors catch invalid macro syntax. `tags[...]` must be inside `all`, `any`, or `not` when executed through `query!`. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let expr = query_expr!(all(tags["enemy"], not(tags["dead"])));
        let q = NodeQuery::new().where_expr(expr);
        let ids = ctx.run.NodeQuery().query(&q);
        let _ = ids;
    }
});
```

### `query_builder`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_builder!(kind args, in_subtree(parent) $(,)?)` |
| Params | `kind args, in_subtree(parent) $(,)?` |
| Returns | `NodeQuery` |
| Use when | Use when one filter is shared across systems, helper functions, or multiple query calls. |
| Fails when / edge behavior | `in_subtree(...)` stored on the builder can be overridden by macro call scope for that call only. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let q = query_builder!(all(base_type[Node3D], tags["interactable"]));
        let ids = query!(ctx.run, &q);
        let _ = ids;
    }
});
```

### `query`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query!(ctx.run, tags[$(tag)*], in_subtree(parent) $(,)?)` |
| Params | `ctx, tags[$(tag)*], in_subtree(parent) $(,)?` |
| Returns | `Vec<NodeID>` |
| Use when | Use when code needs the complete match list for loops, counts, storage, or multi-pass work. |
| Fails when / edge behavior | Query misses return an empty `Vec`. Returned IDs can become stale if nodes are removed later. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let enemies = query!(ctx.run, all(tags["enemy"], not(tags["dead"])));
        for id in enemies {
            call_method!(ctx.run, id, method!("alert"), params![]);
        }
    }
});
```

### `query_iter`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_iter!(ctx.run, expr)` |
| Params | `ctx, expr, optional in_subtree(parent)` |
| Returns | `impl Iterator<Item = NodeID>` |
| Use when | Use when you want iterator adapters and do not need to name the intermediate `Vec`. |
| Fails when / edge behavior | Query misses return an empty iterator. Same allocation behavior as `query!`. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let ids = query_iter!(ctx.run, all(tags["pickup"]))
            .take(8)
            .collect::<Vec<_>>();
        let _ = ids;
    }
});
```

### `query_each`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_each!(ctx.run, expr, |id| { ... })` |
| Params | `ctx, expr, optional in_subtree(parent), closure` |
| Returns | `()` |
| Use when | Use when each match triggers one action and no result list is needed. |
| Fails when / edge behavior | Query misses run the closure zero times. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        query_each!(ctx.run, all(tags["ally"], tags["alive"]), |id| {
            call_method!(ctx.run, id, method!("on_team_buff"), params![variant!(5.0_f32)]);
        });
    }
});
```

### `query_map`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_map!(ctx.run, expr, |id| value)` |
| Params | `ctx, expr, optional in_subtree(parent), closure` |
| Returns | `Vec<T>` |
| Use when | Use when every matching node maps to one output value. |
| Fails when / edge behavior | Query misses return an empty `Vec`. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let positions = query_map!(ctx.run, all(tags["enemy"], base_type[Node3D]), |id| {
            get_global_pos_3d!(ctx.run, id)
        });
        let _ = positions;
    }
});
```

### `query_first`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.NodeQuery()` |
| Signature | `query_first!(ctx.run, kind args, in_subtree(parent) $(,)?)` |
| Params | `ctx, kind args, in_subtree(parent) $(,)?` |
| Returns | `Option<NodeID>` |
| Use when | Use when one match is enough, such as player lookup, target fallback, or singleton manager nodes. |
| Fails when / edge behavior | Query misses return `None`. If several nodes match, current query order decides the first result. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Some(id) = query_first!(ctx.run, any(name["Player"], tags["primary_target"])) {
            set_var!(ctx.run, id, var!("selected"), variant!(true));
        }
    }
});
```
