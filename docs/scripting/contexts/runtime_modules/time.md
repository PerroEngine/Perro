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

