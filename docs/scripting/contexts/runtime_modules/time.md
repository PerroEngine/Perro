# Time Module

Macros:

- `delta_time!(ctx) -> f32`
- `delta_time_capped!(ctx, max) -> f32`
- `delta_time_clamped!(ctx, min, max) -> f32`
- `fixed_delta_time!(ctx) -> f32`
- `elapsed_time!(ctx) -> f32`
- `simulation_time!(ctx) -> Duration`
- `graphics_time!(ctx) -> Duration`
- `frame_time!(ctx) -> Duration`
- `fps!(ctx) -> f32`
- `profiling!(ctx) -> ProfilingSnapshot`

Notes:

- `delta_time!`: frame delta seconds.
- `delta_time_capped!`: frame delta clamped to `[0, max]`.
- `delta_time_clamped!`: frame delta clamped to `[min, max]`.
- `fixed_delta_time!`: fixed-step delta seconds.
- `elapsed_time!`: total runtime seconds.
- `simulation_time!`: last measured simulation time.
- `graphics_time!`: last measured graphics time.
- `frame_time!`: last measured full frame time.
- `fps!`: last measured frames per second.
- `profiling!`: last measured timing bundle with `simulation_time`, `graphics_time`, `frame_time`, and `fps`.

Duration fields support normal Rust time helpers, like `.as_micros()`, `.as_millis()`, and `.as_secs_f32()`.
