# Signals Module

## Page Map

| Header                   | Link                                                |
| ------------------------ | --------------------------------------------------- |
| Purpose                  | [Purpose](#purpose)                                 |
| Use Cases                | [Use Cases](#use-cases)                             |
| Context                  | [Context](#context)                                 |
| Practical Example        | [Practical Example](#practical-example)             |
| API Reference            | [API Reference](#api-reference)                     |
| `connect`                | [`connect`](#connect)                               |
| `connect_many`           | [`connect_many`](#connect_many)                     |
| `disconnect`             | [`disconnect`](#disconnect)                         |
| `disconnect_many`        | [`disconnect_many`](#disconnect_many)               |
| `emit`                   | [`emit`](#emit)                                     |
| `signal_connect!`        | [`signal_connect!`](#signal_connect)                 |
| `signal_connect_pairs!`  | [`signal_connect_pairs!`](#signal_connect_pairs)     |
| `signal_disconnect!`     | [`signal_disconnect!`](#signal_disconnect)           |
| `signal_disconnect_many!` | [`signal_disconnect_many!`](#signal_disconnect_many) |
| `signal_emit!`           | [`signal_emit!`](#signal_emit)                       |

## Purpose

Signals are Perro's decoupled event bus. A script emits a named signal and any
connected handler runs, without the emitter knowing or caring who is listening.
This keeps gameplay systems independent: the boss does not call the music system
directly, it just emits `boss_defeated` and whoever cares reacts. Handlers are
ordinary script methods (`func!` / `method!`) connected by name, and emitted
`Variant` params flow through to them.

## Use Cases

- Boss health threshold triggers phase two: when HP crosses 50%, `signal_emit!(ctx.run, signal!("boss_phase_two"), params![])`; the arena script connected a handler in `on_all_init` with `signal_connect!`.
- One event, many reactions: a collected coin should bump the score, refresh the HUD, and play a sound — connect all three with `connect_many` (which forms the signal x function product) or list them explicitly.
- Wire a screen of UI buttons to their handlers at once: `signal_connect_pairs!(ctx.run, ctx.id, [("play_click", "on_play"), ("quit_click", "on_quit")])` connects each signal to exactly its paired function.
- React to timer and animation events: connect to `timer_finished!("spawn_wave")` or an animation event signal to run logic at the right beat.
- Broadcast world state: emit `player_died` so checkpoints, enemies, and UI all respond from their own scripts.
- Tear down listeners when a menu or level closes: `signal_disconnect_many!(ctx.run, ctx.id, signals, functions)`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Signals()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A boss script emits a phase-change signal once its health drops past a
threshold. Connect the handler once in `on_all_init`, then emit from update
logic; the emitter never needs a reference to the arena systems that react.

```rust
#[State]
struct BossState {
    #[default = 100.0]
    pub health: f32,
    #[default = false]
    pub phase_two: bool,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("boss_phase_two"), func!("on_phase_two"));
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, BossState, ctx.id, |state| {
            if !state.phase_two && state.health <= 50.0 {
                state.phase_two = true;
                signal_emit!(ctx.run, signal!("boss_phase_two"), params![]);
            }
        });
    }
});

methods!({
    fn on_phase_two(&self, ctx: &mut ScriptContext<'_, API>) {
        // Enrage. Re-broadcast so the music and arena hazards react too.
        signal_emit!(ctx.run, signal!("music_set_intensity"), params![variant!(1.0_f32)]);
    }
});
```

## API Reference

### `connect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn connect( &mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, params: &[Variant], ) -> bool`                                                                                          |
| Params                     | `&mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, params: &[Variant],`                                                                                                                    |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `disconnect`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn disconnect( &mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID, ) -> bool`                                                                                                           |
| Params                     | `&mut self, script_id: NodeID, signal: SignalID, function: ScriptMemberID,`                                                                                                                                        |
| Returns                    | `bool`                                                                                                                                                                                                             |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `connect_many`

| Field                      | Detail                                                                 |
| -------------------------- | ---------------------------------------------------------------------- |
| Access                     | `ctx.run.Signals()`                                                    |
| Signature                  | `pub fn connect_many(...) -> usize`                                    |
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

### `disconnect_many`

| Field                      | Detail                                                         |
| -------------------------- | -------------------------------------------------------------- |
| Access                     | `ctx.run.Signals()`                                            |
| Signature                  | `pub fn disconnect_many(...) -> usize`                 |
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

### `emit`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `pub fn emit(&mut self, signal: SignalID, params: &[Variant]) -> usize`                                                                                                                                            |
| Params                     | `&mut self, signal: SignalID, params: &[Variant]`                                                                                                                                                                  |
| Returns                    | `usize`                                                                                                                                                                                                            |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `signal_connect!`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_connect!(ctx.run, script, signal, function, params)`                                                                                                                                                       |
| Params                     | `ctx, script, signal, function, params`                                                                                                                                                                            |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `signal_connect_pairs!`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_connect_pairs!(ctx.run, script, [(signal, function), ...][, params]) -> usize`                                                                                                                             |
| Params                     | `ctx, script, [(signal_name, function_name), ...], [params]`                                                                                                                                                       |
| Returns                    | `usize` (count of new connections)                                                                                                                                                                                 |
| Use when                   | Use to wire many signals to their 1:1 same-purpose handlers at once. Unlike `connect_many` (cartesian product), each signal connects only to its paired function. Pair elements are name strings.                  |
| Fails when / edge behavior | Each pair that fails to connect simply does not increment the returned count.                                                                                                                                      |

### `signal_disconnect!`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_disconnect!(ctx.run, script, signal, function)`                                                                                                                                                            |
| Params                     | `ctx, script, signal, function`                                                                                                                                                                                    |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when code must release, remove, stop, or disconnect existing engine state.                                                                                                                                     |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `signal_disconnect_many!`

| Field                      | Detail                                                                            |
| -------------------------- | --------------------------------------------------------------------------------- |
| Access                     | `ctx.run.Signals()`                                                               |
| Signature                  | `signal_disconnect_many!(ctx.run, script, signals, functions) -> usize`           |
| Params                     | `ctx, script, signals, functions`                                                 |
| Returns                    | `usize` removed connection count                                                  |
| Use when                   | Remove many signal/function links with one call.                                  |
| Fails when / edge behavior | Missing signal/script/function pairs do not increment the returned count.         |

### `signal_emit!`

| Field                      | Detail                                                                                                                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Signals()`                                                                                                                                                                                                |
| Signature                  | `signal_emit!(ctx.run, signal, params)`                                                                                                                                                                            |
| Params                     | `ctx, signal, params`                                                                                                                                                                                              |
| Returns                    | `same as backing method`                                                                                                                                                                                           |
| Use when                   | Use when gameplay must change engine state or queue an action this frame.                                                                                                                                          |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

