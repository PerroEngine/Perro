# Signals Module

## Page Map

| Header                   | Link                                                |
| ------------------------ | --------------------------------------------------- |
| Overview                 | [Overview](#overview)                               |
| Context                  | [Context](#context)                                 |
| API Reference            | [API Reference](#api-reference)                     |
| `signal_connect`         | [`signal_connect`](#signal_connect)                 |
| `signal_connect_many`    | [`signal_connect_many`](#signal_connect_many)       |
| `signal_disconnect`      | [`signal_disconnect`](#signal_disconnect)           |
| `signal_disconnect_many` | [`signal_disconnect_many`](#signal_disconnect_many) |
| `signal_emit`            | [`signal_emit`](#signal_emit)                       |
| `signal_connect`         | [`signal_connect`](#signal_connect)                 |
| `signal_disconnect`      | [`signal_disconnect`](#signal_disconnect)           |
| `signal_disconnect_many` | [`signal_disconnect_many`](#signal_disconnect_many) |
| `signal_emit`            | [`signal_emit`](#signal_emit)                       |

## Overview

This runtime module belongs to `ctx.run` and documents signals calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Signals()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_emit!(ctx.run, signal!("player_spawned"), params![ctx.id]);
    }
});
```

## API Reference

### `signal_connect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn signal_connect( &mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, params: &[Variant], ) -> bool`                                                                                   |
| Params                     | `&mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, params: &[Variant],`                                                                                                                    |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Signals().signal_connect(ctx.id, 0.0, 0.1, variant!(0_i32));
        let _ = value;
    }
});
```

### `signal_disconnect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn signal_disconnect( &mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, ) -> bool`                                                                                                    |
| Params                     | `&mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID,`                                                                                                                                        |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Signals().signal_disconnect(ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `signal_connect_many`

| Field                      | Detail                                                                 |
| -------------------------- | ---------------------------------------------------------------------- |
| Access                     | `ctx.run.Signals()`                                                    |
| Signature                  | `signal_connect_many!(ctx.run, script, signals, functions, params)`    |
| Params                     | `ctx, script, signals, functions, params`                              |
| Returns                    | `usize` new connection count                                           |
| Use when                   | Connect many signals to one function, or one signal to many functions. |
| Fails when / edge behavior | Duplicate signal/script/function pairs do not add new connections.     |

Example:

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let _ = signal_connect_many!(
            ctx.run,
            ctx.id,
            [signal!("open"), signal!("close")],
            [func!("on_window_signal")]
        );
        let _ = signal_connect_many!(
            ctx.run,
            ctx.id,
            [signal!("changed")],
            [func!("refresh_ui"), func!("mark_dirty")]
        );
        let _ = signal_connect_many!(
            ctx.run,
            ctx.id,
            [signal!("hover"), signal!("click")],
            [func!("play_sound"), func!("track_input")]
        );
    }
});
```

### `signal_disconnect_many`

| Field                      | Detail                                                         |
| -------------------------- | -------------------------------------------------------------- |
| Access                     | `ctx.run.Signals()`                                            |
| Signature                  | `signal_disconnect_many!(ctx.run, script, signals, functions)` |
| Params                     | `ctx, script, signals, functions`                              |
| Returns                    | `usize` removed connection count                               |
| Use when                   | Remove many signal/function links at once.                     |
| Fails when / edge behavior | Missing signal/script/function pairs do not count as removed.  |

Example:

```rust
let _ = signal_disconnect_many!(
    ctx.run,
    ctx.id,
    [signal!("hover"), signal!("click")],
    [func!("play_sound"), func!("track_input")]
);
```

### `signal_emit`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn signal_emit(&mut self, signal: SignalID, params: &[Variant]) -> usize`                                                                                                                                     |
| Params                     | `&mut self, signal: SignalID, params: &[Variant]`                                                                                                                                                                  |
| Returns                    | `usize`                                                                                                                                                                                                            |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Signals().signal_emit(Default::default(), variant!(0_i32));
        let _ = value;
    }
});
```

### `signal_connect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_connect!(ctx.run, script, signal, function, params)`                                                                                                                                                       |
| Params                     | `ctx, script, signal, function, params`                                                                                                                                                                            |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = signal_connect!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `signal_disconnect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_disconnect!(ctx.run, script, signal, function)`                                                                                                                                                            |
| Params                     | `ctx, script, signal, function`                                                                                                                                                                                    |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = signal_disconnect!(ctx.run, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `signal_emit`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_emit!(ctx.run, signal, params)`                                                                                                                                                                            |
| Params                     | `ctx, signal, params`                                                                                                                                                                                              |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = signal_emit!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```
