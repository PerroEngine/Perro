# `.ppart` Format

`*.ppart` is a **Perro Particle** resource and defines mathematical per-particle profile behavior used by `ParticleEmitter3D`.

For full emitter + runtime behavior, read [Particle System Guide](../particles.md).

## Usage

```scn
[ParticleEmitter3D]
    profile = "res://particles/fire_spiral.ppart"
    params = (3.0, 2.0, 8.0, 0.0)
[/ParticleEmitter3D]
```

## Keys

Core path/expression keys:

- `preset`
- `preset_param_a`
- `preset_param_b`
- `preset_param_c`
- `preset_param_d`
- `x`
- `y`
- `z`

Profile keys:

- `lifetime_min`
- `lifetime_max`
- `speed_min`
- `speed_max`
- `spread_radians`
- `size`
- `size_min`
- `size_max`
- `force` or `force_x`, `force_y`, `force_z`
- `color_start`
- `color_end`
- `emissive`
- `spin`

## Defaults

```txt
lifetime_min = 0.6
lifetime_max = 1.4
speed_min = 1.0
speed_max = 3.0
spread_radians = 1.0471976
size = 6.0
size_min = 0.65
size_max = 1.35
force = (0.0, 0.0, 0.0)
color_start = (1.0, 1.0, 1.0, 1.0)
color_end = (1.0, 0.4, 0.1, 0.0)
emissive = (0.0, 0.0, 0.0)
spin = 0.0
```

## Presets

Supported `preset` values:

- `ballistic`
- `spiral`
- `orbit_y`
- `noise_drift`
- `flat_disk`

If omitted, no preset path is applied.

Preset mappings:

- `spiral`: `preset_param_a = angular_velocity`, `preset_param_b = radius`
- `orbit_y`: `preset_param_a = angular_velocity`, `preset_param_b = radius`
- `noise_drift`: `preset_param_a = amplitude`, `preset_param_b = frequency`
- `flat_disk`: `preset_param_a = radius`

`x`, `y`, `z` expressions are additive offsets on top of preset output.

## Expressions

Operators:

- `+`, `-`, `*`, `/`, `^`, unary `-`

Functions:

- `sin`, `cos`, `tan`, `abs`, `sqrt`, `min`, `max`, `clamp`
- `hash(x)`: deterministic pseudo-random scalar in `[0,1)` derived from input `x`.

Constants/inputs:

- `pi`: constant `3.14159265...`.
- `tau`: constant `6.28318530...` (`2*pi`).
- `params[i]`: emitter-provided parameter array value at index `i` (out-of-range -> `0.0`).
- `t`: normalized particle age in `[0,1]` (`0` at spawn, `1` at death).
- `life`: elapsed seconds since this particle spawned.
- `lifetime`: this particle's sampled total lifetime in seconds.
- `age_left`: remaining life in seconds (`max(lifetime - life, 0)`).
- `spawn_time`: emitter simulation time when this particle spawned.
- `emitter_time`: current emitter simulation time.
- `speed`: particle sampled initial speed (`speed_min..speed_max`).
- `id`: stable particle id/key (float form).
- `dir_x`, `dir_y`, `dir_z`: initial unit direction components.
- `vel_x`, `vel_y`, `vel_z`: initial velocity components (`dir * speed`).
- `rand`, `rand2`, `rand3`: three stable random channels in `[0,1]` per particle.
- `seed`: stable per-particle seed-derived value.
- `ring_u`: stable low-discrepancy scalar in `[0,1)`, useful for ring/circle layouts.
- `emitter_x`, `emitter_y`, `emitter_z`: emitter world position components.
- `prev_x`, `prev_y`, `prev_z`: previous-frame base world position components for this particle
