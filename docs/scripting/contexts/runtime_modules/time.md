# Time Module

## Page Map

| Header                | Link                                          |
| --------------------- | --------------------------------------------- |
| Purpose               | [Purpose](#purpose)                           |
| Use Cases             | [Use Cases](#use-cases)                       |
| Context               | [Context](#context)                           |
| Practical Example     | [Practical Example](#practical-example)       |
| Named Timers          | [Named Timers](#named-timers)                 |
| API Reference         | [API Reference](#api-reference)               |
| `get_delta`           | [`get_delta`](#get_delta)                     |
| `get_fixed_delta`     | [`get_fixed_delta`](#get_fixed_delta)         |
| `get_elapsed`         | [`get_elapsed`](#get_elapsed)                 |
| `get_simulation_time` | [`get_simulation_time`](#get_simulation_time) |
| `get_graphics_time`   | [`get_graphics_time`](#get_graphics_time)     |
| `get_frame_time`      | [`get_frame_time`](#get_frame_time)           |
| `get_fps`             | [`get_fps`](#get_fps)                         |
| `get_profiling`       | [`get_profiling`](#get_profiling)             |
| `delta_time`          | [`delta_time`](#delta_time)                   |
| `delta_time_capped`   | [`delta_time_capped`](#delta_time_capped)     |
| `delta_time_clamped`  | [`delta_time_clamped`](#delta_time_clamped)   |
| `fixed_delta_time`    | [`fixed_delta_time`](#fixed_delta_time)       |
| `elapsed_time`        | [`elapsed_time`](#elapsed_time)               |
| `simulation_time`     | [`simulation_time`](#simulation_time)         |
| `graphics_time`       | [`graphics_time`](#graphics_time)             |
| `frame_time`          | [`frame_time`](#frame_time)                   |
| `fps`                 | [`fps`](#fps)                                 |
| `profiling`           | [`profiling`](#profiling)                     |

## Purpose

The time module gives scripts the frame clock the whole game runs on. Movement,
cooldowns, timers, and animations all need to know how much wall-clock time has
passed since the last frame so behaviour stays identical at 30 or 240 FPS.
It also exposes elapsed clocks for HUD timers and a per-frame profiling snapshot
for performance overlays, plus named one-shot timers that fire a signal after a
delay without any per-frame countdown bookkeeping.

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Move per rendered frame | `delta_time!` | Converts per-second rates to current frame distance | Large hitches can create large steps |
| Movement must resist hitches/window drags | capped or clamped delta | Bounds one-frame motion | Drops part of elapsed wall time |
| Deterministic simulation step | fixed delta in `on_fixed_update` | Step duration does not follow render FPS | Fixed hook cadence differs from render cadence |
| Cooldown only needs ready/not-ready | named timer + finish signal | No per-frame state mutation | One timer slot per name; use distinct names for concurrency |
| UI displays cooldown progress every frame | state clock | Intermediate remaining value is available | Script must decrement and clamp it each frame |
| Performance overlay samples engine stats | `fps!` / `profiling!` | Uses runtime measurements rather than script estimates | Values are observations, not stable gameplay clocks |

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Time()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A dash ability: read a spike-safe delta each frame to advance a cooldown, then
move the body frame-rate-independently while the dash is active.

```rust
#[State]
struct DashState {
    #[default = 0.0]
    pub cooldown: f32,
    #[default = 0.0]
    pub dash_left: f32,
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        // Clamp the delta so a hitch cannot teleport the player.
        let dt = delta_time_capped!(ctx.run, 0.1);

        let dash = with_state_mut!(ctx.run, DashState, ctx.id, |state| {
            state.cooldown = (state.cooldown - dt).max(0.0);
            if state.dash_left > 0.0 {
                state.dash_left -= dt;
                true
            } else {
                false
            }
        }).unwrap_or(false);

        if dash {
            if let Some(mut pos) = get_global_pos_3d!(ctx.run, ctx.id) {
                pos.z -= 20.0 * dt; // 20 units/second forward
                set_global_pos_3d!(ctx.run, ctx.id, pos);
            }
        }
    }
});
```

## Named Timers

Use named timers for one-shot delays without script state vars or per-frame countdown code.

One timer exists per name. Starting the same name resets it. Use distinct names
for concurrent delays. Keep state clocks only when each-frame progress matters.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, timer_finished!("wait"), func!("on_wait"));
        timer_start!(ctx.run, Duration::from_secs(2), "wait");
    }
});

methods!({
    fn on_wait(&self, _ctx: &mut ScriptContext<'_, API>) {
        // Runs automatically after two seconds.
    }
});
```

`timer_start!(ctx.run, duration, "name")` emits `name_started` immediately.

It emits `name_finished` when game-frame time reaches the deadline.

Use `timer_started!("name")` and `timer_finished!("name")` to connect without spelling signal names twice.

Starting the same name again resets its deadline. Code from any script may reset it with a new duration.

Use `timer_cancel!`, `timer_is_active!`, and `timer_remaining!` for optional control. Remaining time returns `Option<Duration>`. `Duration::ZERO` finishes immediately.

The runtime stores one active timer per name in a central deadline heap. Frames inspect only expired deadlines instead of decrementing every timer.

Literal names, `_started` / `_finished` suffixes, and all ID hashes resolve at compile time. Timer starts and queries allocate no strings.

Timer registry keys use `TimerID`. Use `timer!("name")` for direct module calls.

String expressions and variables work too. Dynamic starts hash the name at runtime and build two temporary suffixed strings; dynamic cancel, active, and remaining queries only hash the supplied name.

```rust
let timer_name = format!("player_{}_wait", ctx.id.index());
timer_start!(ctx.run, Duration::from_millis(250), timer_name.as_str());
let left = timer_remaining!(ctx.run, timer_name.as_str());
```

## API Reference

### `get_delta`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_delta(&mut self) -> f32`                                                                               |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `get_delta` to get delta from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_delta` keeps the backing API behavior. |

### `get_fixed_delta`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_fixed_delta(&mut self) -> f32`                                                                         |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `get_fixed_delta` to get fixed delta from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_fixed_delta` keeps the backing API behavior. |

### `get_elapsed`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_elapsed(&mut self) -> f32`                                                                             |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `get_elapsed` to get elapsed from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_elapsed` keeps the backing API behavior. |

### `get_simulation_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_simulation_time(&mut self) -> Duration`                                                                |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `get_simulation_time` to get simulation time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_simulation_time` keeps the backing API behavior. |

### `get_graphics_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_graphics_time(&mut self) -> Duration`                                                                  |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `get_graphics_time` to get graphics time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_graphics_time` keeps the backing API behavior. |

### `get_frame_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_frame_time(&mut self) -> Duration`                                                                     |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `get_frame_time` to get frame time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_frame_time` keeps the backing API behavior. |

### `get_fps`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_fps(&mut self) -> f32`                                                                                 |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `get_fps` to get fps from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_fps` keeps the backing API behavior. |

### `get_profiling`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `pub fn get_profiling(&mut self) -> ProfilingSnapshot`                                                             |
| Params                     | `&mut self`                                                                                                        |
| Returns                    | `ProfilingSnapshot`                                                                                                |
| Use when | Use `get_profiling` to get profiling from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `get_profiling` keeps the backing API behavior. |

### `delta_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `delta_time!(ctx.run)`                                                                                             |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `delta_time` to delta time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `delta_time` keeps the backing API behavior. |

### `delta_time_capped`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `delta_time_capped!(ctx.run, max)`                                                                                 |
| Params                     | `ctx, max`                                                                                                         |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `delta_time_capped` to delta time capped from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `delta_time_capped` keeps the backing API behavior. |

### `delta_time_clamped`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `delta_time_clamped!(ctx.run, min, max)`                                                                           |
| Params                     | `ctx, min, max`                                                                                                    |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `delta_time_clamped` to delta time clamped from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `delta_time_clamped` keeps the backing API behavior. |

### `fixed_delta_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `fixed_delta_time!(ctx.run)`                                                                                       |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `fixed_delta_time` to fixed delta time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `fixed_delta_time` keeps the backing API behavior. |

### `elapsed_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `elapsed_time!(ctx.run)`                                                                                           |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `elapsed_time` to elapsed time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `elapsed_time` keeps the backing API behavior. |

### `simulation_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `simulation_time!(ctx.run)`                                                                                        |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `simulation_time` to simulation time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `simulation_time` keeps the backing API behavior. |

### `graphics_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `graphics_time!(ctx.run)`                                                                                          |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `graphics_time` to graphics time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `graphics_time` keeps the backing API behavior. |

### `frame_time`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `frame_time!(ctx.run)`                                                                                             |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `Duration`                                                                                                         |
| Use when | Use `frame_time` to frame time from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `frame_time` keeps the backing API behavior. |

### `fps`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `fps!(ctx.run)`                                                                                                    |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `f32`                                                                                                              |
| Use when | Use `fps` to fps from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `fps` keeps the backing API behavior. |

### `profiling`

| Field                      | Detail                                                                                                             |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Access                     | `ctx.run.Time()`                                                                                                   |
| Signature                  | `profiling!(ctx.run)`                                                                                              |
| Params                     | `ctx`                                                                                                              |
| Returns                    | `ProfilingSnapshot`                                                                                                |
| Use when | Use `profiling` to profiling from the runtime clock; choose frame, fixed, elapsed, or profiling time by behavior semantics. |
| Fails when / edge behavior | Has no separate failure value in this wrapper; `profiling` keeps the backing API behavior. |
