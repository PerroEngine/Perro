# Script Utility Modules

Perro exposes utility modules through `perro::modules`.

Import from `prelude`:

```rust
use perro::prelude::*;
```

Or import specific module:

```rust
use perro::modules::random;
```

## `modules::file`

File IO helpers backed by project path resolver.

- `set_project_root_disk(root: &str, name: &str)`
- `load_bytes(path: &str) -> io::Result<Vec<u8>>`
- `load_string(path: &str) -> io::Result<String>`
- `save_bytes(path: &str, data: &[u8]) -> io::Result<()>`
- `save_string(path: &str, data: &str) -> io::Result<()>`
- `exists(path: &str) -> bool`
- `resolve_path_string(path: &str) -> String`

Write restriction:

- write paths must be `user://...` or absolute paths
- relative non-`user://` writes return permission error

## `modules::json`

JSON <-> `Variant` conversion helpers.

- `parse(json_str: &str) -> Result<Variant, serde_json::Error>`
- `stringify(value: &Variant) -> Result<String, serde_json::Error>`

## `modules::log`

Log helpers + macros.

Functions:

- `print(message: impl Display)`
- `info(message: impl Display)`
- `warn(message: impl Display)`
- `error(message: impl Display)`

Macros:

- `log_print!(...)`
- `log_info!(...)`
- `log_warn!(...)`
- `log_error!(...)`

## `modules::math`

Math helpers:

- `deg_to_rad(degrees: f32) -> f32`
- `rad_to_deg(radians: f32) -> f32`
- `clamp01(value: f32) -> f32`
- `lerp(start: f32, end: f32, t: f32) -> f32`
- `ilerp(start: f32, end: f32, value: f32) -> f32`
- `slerp(start: f32, end: f32, t: f32) -> f32`
- `islerp(start: f32, end: f32, value: f32) -> f32`
- `remap(in_min: f32, in_max: f32, out_min: f32, out_max: f32, value: f32) -> f32`
- `smoothstep(edge0: f32, edge1: f32, value: f32) -> f32`
- `ismoothstep(edge0: f32, edge1: f32, value: f32) -> f32`
- `angle_diff_rad(from: f32, to: f32) -> f32`
- `angle_diff_deg(from: f32, to: f32) -> f32`
- `lerp_angle_rad(from: f32, to: f32, t: f32) -> f32`
- `lerp_angle_deg(from: f32, to: f32, t: f32) -> f32`
- `wrap_angle_rad(angle: f32) -> f32` in `[-PI, PI)`
- `wrap_angle_deg(angle: f32) -> f32` in `[-180, 180)`
- `approach(current: f32, target: f32, max_delta: f32) -> f32`
- `damp(current: f32, target: f32, lambda: f32, delta_time: f32) -> f32`
- `smooth_damp(current, target, current_velocity, smooth_time, max_speed, delta_time) -> (f32, f32)`
- `repeat(value: f32, length: f32) -> f32`
- `ping_pong(value: f32, length: f32) -> f32`
- `nearly_eq(a: f32, b: f32, epsilon: f32) -> bool`
- macros: `deg_to_rad!(x)`, `rad_to_deg!(x)`

## `modules::random`

Deterministic helpers for seeded random generation and stable hashing.

### Hash helpers

- `hash<T: HashToU32>(value: T) -> u32`
- trait: `HashToU32` (`u32`, `i32`, `u64`, `i64`, `u128`, `bool`, `f32`)
- `hash_u32(value: u32) -> u32`
- `hash_i32(value: i32) -> u32`
- `hash_u64(value: u64) -> u32`
- `hash_i64(value: i64) -> u32`
- `hash_u128(value: u128) -> u32`
- `hash_bool(value: bool) -> u32`
- `hash_f32(value: f32) -> u32`
- `hash_bytes(bytes: &[u8]) -> u32`
- `hash_str(value: &str) -> u32`
- `hash_combine(a: u32, b: u32) -> u32`
- `hash_combine3(a: u32, b: u32, c: u32) -> u32`
- `hash_combine4(a: u32, b: u32, c: u32, d: u32) -> u32`
- `hash2_u32(x: u32, y: u32) -> u32`
- `hash3_u32(x: u32, y: u32, z: u32) -> u32`

### 64-bit hash helpers

- `hash64_u32(value: u32) -> u64`
- `hash64_u64(value: u64) -> u64`
- `hash64_u128(value: u128) -> u64`
- `hash64_bytes(bytes: &[u8]) -> u64`
- `hash64_str(value: &str) -> u64`

### Seed -> random value

- `rand_range<T: RandRangeValue>(min: T, max: T, seed: u32) -> T`
- trait: `RandRangeValue` (`f32`, `i32`, `u32`)
- `rand_u32(seed: u32) -> u32`
- `rand01(seed: u32) -> f32` in `[0, 1]`
- `rand11(seed: u32) -> f32` in `[-1, 1]`
- `rand_range_f32(min: f32, max: f32, seed: u32) -> f32`
- `rand_range_i32(min: i32, max: i32, seed: u32) -> i32`
- `rand_range_u32(min: u32, max: u32, seed: u32) -> u32`
- `chance(probability: f32, seed: u32) -> bool`
- `choose_index(len: usize, seed: u32) -> Option<usize>`

### Seed + index stream

- `rand_u32_stream(seed: u32, index: u32) -> u32`
- `rand01_stream(seed: u32, index: u32) -> f32` in `[0, 1]`
- `rand11_stream(seed: u32, index: u32) -> f32` in `[-1, 1]`
- `rand_unit_vec2(seed: u32) -> (f32, f32)`
- `rand_unit_vec3(seed: u32) -> (f32, f32, f32)`
- `rand_in_circle(seed: u32) -> (f32, f32)`
- `shuffle(seed: u32, values: &mut [T])`

Use stream helpers when you need multiple stable random values from one base seed.

### Stateful generator

`SeededRng` gives deterministic sequence with internal state:

- `SeededRng::new(seed: u32) -> SeededRng`
- `seed(&self) -> u32`
- `reseed(&mut self, seed: u32)`
- `next_u32(&mut self) -> u32`
- `next_01(&mut self) -> f32` in `[0, 1]`
- `next_11(&mut self) -> f32` in `[-1, 1]`
- `next_range<T: RandRangeValue>(&mut self, min: T, max: T) -> T`
- `next_range_f32(&mut self, min: f32, max: f32) -> f32`
- `next_range_i32(&mut self, min: i32, max: i32) -> i32`
- `next_range_u32(&mut self, min: u32, max: u32) -> u32`
- `next_chance(&mut self, probability: f32) -> bool`
- `next_index(&mut self, len: usize) -> Option<usize>`

Example:

```rust
use perro::prelude::*;

let base_seed = hash_str("enemy_wave_01");

let jitter = rand11_stream(base_seed, 0);
let speed_scale = 0.8 + rand01_stream(base_seed, 1) * 0.4;

let mut rng = SeededRng::new(base_seed);
let color_pick = rng.next_u32() % 4;
```
