# Helpers Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `log_info!` | [`log_info!`](#log_info) |
| `log_warn!` | [`log_warn!`](#log_warn) |
| `log_error!` | [`log_error!`](#log_error) |
| `log_print!` | [`log_print!`](#log_print) |
| ID and value constructors | [ID and value constructors](#id-and-value-constructors) |

## Purpose

This page collects the small cross-cutting helper macros that are not tied to
any single `ctx.run` module but show up in almost every script: the logging
family for printing diagnostics to the console, and the compact ID and value
constructors (`func!`, `signal!`, `var!`, `params!`, `variant!`) that build the
typed handles other runtime calls consume. They are the glue you reach for
regardless of which system you are actually driving.

## Use Cases

- Trace a bug during development: `log_info!("player hp = {}", hp)` prints to the engine console with the game running.
- Warn about a recoverable problem: `log_warn!("no spawn point found, using origin")`.
- Report a real failure: `log_error!("failed to load save slot {}", slot)`.
- Name the target of a signal or method call: `func!("on_hit")` / `method!("on_hit")` build the `ScriptMemberID` that `signal_connect!` and `call_method!` expect.
- Reference a signal by name: `signal!("player_died")` builds the `SignalID` for `signal_emit!` / `signal_connect!`.
- Pass typed arguments across scripts: `params![variant!(10_i32), variant!("hit")]` builds the `&[Variant]` slice for `call_method!` and `signal_emit!`.

## Context

- Script context path: `ctx.run` (the logging macros do not take a context; they print through the engine log).
- Module access: helper macros (no `ctx.run.X()` accessor).
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        match query_first!(ctx.run, all(tags["player"])) {
            Some(player) => {
                log_info!("linked to player node {}", player.index());
                signal_connect!(ctx.run, ctx.id, signal!("player_died"), func!("on_player_died"));
            }
            None => log_warn!("no player node tagged; script idle"),
        }
    }
});

methods!({
    fn on_player_died(&self, ctx: &mut ScriptContext<'_, API>) {
        log_info!("player died; showing game over");
        let _ = call_method!(ctx.run, ctx.id, method!("show_game_over"), params![]);
    }
});
```

## API Reference

The logging macros accept either a single message expression or a `format!`-style
literal plus arguments. They print through the engine log and return `()`.

### `log_info!`

| Field | Detail |
| --- | --- |
| Access | helper macro |
| Signature | `log_info!(message)` or `log_info!("fmt {}", arg, ...)` |
| Params | message expression, or format literal plus arguments |
| Returns | `()` |
| Use when | Print routine diagnostic output while the game runs. |

### `log_warn!`

| Field | Detail |
| --- | --- |
| Access | helper macro |
| Signature | `log_warn!(message)` or `log_warn!("fmt {}", arg, ...)` |
| Params | message expression, or format literal plus arguments |
| Returns | `()` |
| Use when | Flag a recoverable problem that did not stop the game. |

### `log_error!`

| Field | Detail |
| --- | --- |
| Access | helper macro |
| Signature | `log_error!(message)` or `log_error!("fmt {}", arg, ...)` |
| Params | message expression, or format literal plus arguments |
| Returns | `()` |
| Use when | Report a genuine failure (failed load, invalid state). |

### `log_print!`

| Field | Detail |
| --- | --- |
| Access | helper macro |
| Signature | `log_print!(message)` or `log_print!("fmt {}", arg, ...)` |
| Params | message expression, or format literal plus arguments |
| Returns | `()` |
| Use when | Emit plain console output without a severity level. |

### ID and value constructors

These compile-time helpers build the typed handles other runtime calls take.
They are documented in depth alongside the systems that use them; the table is a
quick index.

| Macro | Builds | Used by |
| --- | --- | --- |
| `func!("name")` / `method!("name")` | `ScriptMemberID` | [`signal_connect!`](signals.md), [`call_method!`](scripts.md) |
| `signal!("name")` | `SignalID` | [`signal_emit!` / `signal_connect!`](signals.md) |
| `var!("name")` | script member id | [`get_var!` / `set_var!`](scripts.md) |
| `params![...]` | `&[Variant]` | [`call_method!`](scripts.md), [`signal_emit!`](signals.md) |
| `variant!(expr)` | `Variant` | wrapping single values for `params!` (see [Variant](../../variant.md)) |
