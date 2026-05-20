# Time Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

## Overview

This runtime module belongs to `ctx.run` and documents time calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Time()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let fps_now = fps!(ctx.run);
        let _ = (dt, fps_now);
    }
});
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
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_delta();
        let _ = value;
    }
});
```

### `get_fixed_delta`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_fixed_delta(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_fixed_delta();
        let _ = value;
    }
});
```

### `get_elapsed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_elapsed(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_elapsed();
        let _ = value;
    }
});
```

### `get_simulation_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_simulation_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_simulation_time();
        let _ = value;
    }
});
```

### `get_graphics_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_graphics_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_graphics_time();
        let _ = value;
    }
});
```

### `get_frame_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_frame_time(&mut self) -> Duration` |
| Params | `&mut self` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_frame_time();
        let _ = value;
    }
});
```

### `get_fps`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_fps(&mut self) -> f32` |
| Params | `&mut self` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_fps();
        let _ = value;
    }
});
```

### `get_profiling`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `pub fn get_profiling(&mut self) -> ProfilingSnapshot` |
| Params | `&mut self` |
| Returns | `ProfilingSnapshot` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Time().get_profiling();
        let _ = value;
    }
});
```

### `delta_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = delta_time!(ctx.run);
        let _ = value;
    }
});
```

### `delta_time_capped`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time_capped!(ctx.run, max)` |
| Params | `ctx, max` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = delta_time_capped!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `delta_time_clamped`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `delta_time_clamped!(ctx.run, min, max)` |
| Params | `ctx, min, max` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = delta_time_clamped!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```

### `fixed_delta_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `fixed_delta_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = fixed_delta_time!(ctx.run);
        let _ = value;
    }
});
```

### `elapsed_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `elapsed_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = elapsed_time!(ctx.run);
        let _ = value;
    }
});
```

### `simulation_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `simulation_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = simulation_time!(ctx.run);
        let _ = value;
    }
});
```

### `graphics_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `graphics_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = graphics_time!(ctx.run);
        let _ = value;
    }
});
```

### `frame_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `frame_time!(ctx.run)` |
| Params | `ctx` |
| Returns | `Duration` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = frame_time!(ctx.run);
        let _ = value;
    }
});
```

### `fps`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `fps!(ctx.run)` |
| Params | `ctx` |
| Returns | `f32` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = fps!(ctx.run);
        let _ = value;
    }
});
```

### `profiling`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Time()` |
| Signature | `profiling!(ctx.run)` |
| Params | `ctx` |
| Returns | `ProfilingSnapshot` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = profiling!(ctx.run);
        let _ = value;
    }
});
```
