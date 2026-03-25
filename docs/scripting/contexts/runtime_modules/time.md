# Time Module

Macros:
- `delta_time!(ctx) -> f32`
- `delta_time_capped!(ctx, max) -> f32`
- `delta_time_clamped!(ctx, min, max) -> f32`
- `fixed_delta_time!(ctx) -> f32`
- `elapsed_time!(ctx) -> f32`

Notes:
- `delta_time!`: frame delta seconds.
- `delta_time_capped!`: frame delta clamped to `[0, max]`.
- `delta_time_clamped!`: frame delta clamped to `[min, max]`.
- `fixed_delta_time!`: fixed-step delta seconds.
- `elapsed_time!`: total runtime seconds.
