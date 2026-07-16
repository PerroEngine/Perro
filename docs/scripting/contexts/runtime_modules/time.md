# Time Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| Named Timers | [Named Timers](#named-timers) |
| API Reference | [API Reference](#api-reference) |
| `get_delta` | [`get_delta`](#get_delta) |
| `get_fixed_delta` | [`get_fixed_delta`](#get_fixed_delta) |
| `get_elapsed` | [`get_elapsed`](#get_elapsed) |
| `get_simulation_time` | [`get_simulation_time`](#get_simulation_time) |
| `get_graphics_time` | [`get_graphics_time`](#get_graphics_time) |
| `get_frame_time` | [`get_frame_time`](#get_frame_time) |
| `get_fps` | [`get_fps`](#get_fps) |
| `get_profiling` | [`get_profiling`](#get_profiling) |
| `delta_time` | [`delta_time`](#delta_time) |
| `delta_time_capped` | [`delta_time_capped`](#delta_time_capped) |
| `delta_time_clamped` | [`delta_time_clamped`](#delta_time_clamped) |
| `fixed_delta_time` | [`fixed_delta_time`](#fixed_delta_time) |
| `elapsed_time` | [`elapsed_time`](#elapsed_time) |
| `simulation_time` | [`simulation_time`](#simulation_time) |
| `graphics_time` | [`graphics_time`](#graphics_time) |
| `frame_time` | [`frame_time`](#frame_time) |
| `fps` | [`fps`](#fps) |
| `profiling` | [`profiling`](#profiling) |

## Purpose

The time module gives scripts the frame clock the whole game runs on. Movement,
cooldowns, timers, and animations all need to know how much wall-clock time has
passed since the last frame so behaviour stays identical at 30 or 240 FPS.
It also exposes elapsed clocks for HUD timers and a per-frame profiling snapshot
for performance overlays, plus named one-shot timers that fire a signal after a
delay without any per-frame countdown bookkeeping.

## Use Cases

- Frame-rate-independent movement: scale a speed by `delta_time!(ctx.run)` so a body travels the same distance per second regardless of FPS.
- Spike-proof stepping: after a stall or window drag, clamp the jump with `delta_time_capped!(ctx.run, 0.1)` or `delta_time_clamped!(ctx.run, min, max)` so nothing tunnels through walls.
- Deterministic gameplay in fixed steps: read `fixed_delta_time!(ctx.run)` inside `on_fixed_update` for stable physics-tick logic.
- Survival-mode / speedrun HUD clock: display total run time from `elapsed_time!(ctx.run)`.
- Ability cooldowns and staged delays: start `timer_start!(ctx.run, Duration::from_secs(2), "dash_cd")` and react to `timer_finished!("dash_cd")` instead of counting down a state field each frame.
- Performance overlay: read `fps!(ctx.run)` and `profiling!(ctx.run)` to show FPS, frame time, and draw-call counts.

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

        with_state_mut!(ctx.run, DashState, ctx.id, |state| {
            state.cooldown = (state.cooldown - dt).max(0.0);
            if state.dash_left > 0.0 {
                state.dash_left -= dt;
                let mut pos = get_global_pos_3d!(ctx.run, ctx.id);
                pos.z -= 20.0 * dt; // 20 units/second forward
                set_global_pos_3d!(ctx.run, ctx.id, pos);
            }
        });
    }
});
```

## Named Timers

Use named timers for one-shot delays without script state vars or per-frame countdown code.

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

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_delta(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_fixed_delta`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_fixed_delta(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_elapsed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_elapsed(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_simulation_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_simulation_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_graphics_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_graphics_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_frame_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_frame_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_fps`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_fps(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_profiling`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_profiling(&mut self) -> ProfilingSnapshot` |
| Params | `&mut self` |
| Returns | `ProfilingSnapshot` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `delta_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `delta_time_capped`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time_capped!(ctx.run, max)` |
| Params | `ctx, max` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `delta_time_clamped`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time_clamped!(ctx.run, min, max)` |
| Params | `ctx, min, max` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `fixed_delta_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `fixed_delta_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `elapsed_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `elapsed_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `simulation_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `simulation_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `graphics_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `graphics_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `frame_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `frame_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `fps`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `fps!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `profiling`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `profiling!(ctx.run)` |
| Params | `ctx` |
| Returns | `ProfilingSnapshot` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

