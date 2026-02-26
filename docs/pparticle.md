# `.pparticle` Format

`*.pparticle` defines **per-particle motion math over lifetime**.

Emitter-level controls (count, emission, lifetime range, speed range, etc.) are fields on `ParticleEmitter3D` in scene.

## Where It Is Used

On a `ParticleEmitter3D` node:

```scn
[ParticleEmitter3D]
    particle = "res://particles/fire_spiral.pparticle"
    params = (3.0, 2.0, 8.0, 0.0)
[/ParticleEmitter3D]
```

`params` is a float slice exposed to expressions as `params[index]`.

## Supported Keys

- `mode`
- `param_a`
- `param_b`
- `expr_x` (custom mode)
- `expr_y` (custom mode)
- `expr_z` (custom mode)

## Modes

### `ballistic`

Default path; no extra keys required.

### `spiral`

```txt
mode = spiral
param_a = 8.0    # angular velocity
param_b = 1.5    # radius
```

### `orbit_y`

```txt
mode = orbit_y
param_a = 4.0    # angular velocity
param_b = 2.0    # radius
```

### `noise_drift`

```txt
mode = noise_drift
param_a = 1.2    # amplitude
param_b = 6.0    # frequency
```

### `custom`

Define per-axis offset equations over lifetime.

```txt
mode = custom
expr_x = sin(t * params[0] * pi * 2.0) * params[1]
expr_y = t * params[2]
expr_z = cos(t * params[0] * pi * 2.0) * params[1]
```

## Expression Inputs

- `t` = normalized lifetime in `[0, 1]`
- `life` = elapsed seconds since spawn
- `pi`
- `params[i]` = emitter parameter slice value (out-of-range returns `0.0`)

## Expression Operators

- `+`, `-`, `*`, `/`, `^`
- Parentheses: `( ... )`

## Expression Functions

- `sin(x)`
- `cos(x)`
- `tan(x)`
- `abs(x)`
- `sqrt(x)`
- `min(a, b)`
- `max(a, b)`
- `clamp(x, lo, hi)`

## Inline Particle Definition

You can inline instead of using a file:

```scn
[ParticleEmitter3D]
    particle = {
        mode: custom,
        expr_x: "sin(t * params[0])",
        expr_y: "t * params[1]",
        expr_z: "0.0"
    }
    params = (8.0, 4.0)
[/ParticleEmitter3D]
```
