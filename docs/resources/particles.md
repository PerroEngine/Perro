# Particle System Guide

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

The particle system builds effects like fire, smoke, sparks, magic, and weather from math instead of pre-baked sprite sheets. An emitter node (`ParticleEmitter3D` / `ParticleEmitter2D`) decides when and how many particles spawn; a `.ppart` profile defines what each particle does over its lifetime through presets and per-axis expressions. Splitting the two means one profile powers many emitters, each tuned with its own seed, rate, and `params`.

## Use Cases

- Campfire or torch flame: a `spiral` preset `.ppart` with `force = (0, 2.5, 0)` upward drift and `color_start`/`color_end` fading orange to smoke, spawned at a steady `spawn_rate`.
- Muzzle flash / impact burst: `looping = false`, high one-shot `spawn_rate`, `prewarm = false`, and a short `lifetime_min`/`lifetime_max`.
- Ground dust or embers: `flat_disk` preset seeding particles across a radius, with `size_min`/`size_max` variance.
- Reusable effect tuned per instance: one `.ppart` reads `params[0]` as flame height so each emitter passes a different `params = (...)`.
- GPU-heavy weather (rain, snow, ash): set `sim_mode = "gpu"` and `render_mode = "billboard"` for high particle counts; fall back to `sim_mode = "cpu"` on low-end targets.
- Deterministic layouts (rings, fountains): expressions using `ring_u`, `rand`, and `id` place particles without per-frame CPU work.

## Example

Define `res://particles/campfire.ppart`:

```txt
preset = spiral
preset_param_a = 8.0
preset_param_b = 1.1
lifetime_min = 0.8
lifetime_max = 1.7
speed_min = 1.2
speed_max = 3.4
spread_radians = 0.49
size = 5.0
force = (0.0, 2.8, 0.0)
color_start = (1.0, 0.68, 0.20, 1.0)
color_end = (0.95, 0.08, 0.02, 0.0)
emissive = (1.0, 0.38, 0.05)
spin = 4.0
y = t * params[0]
```

Spawn it from a scene, passing a per-instance flame height through `params[0]`:

```scn
[Campfire]
    [ParticleEmitter3D]
        active = true
        looping = true
        prewarm = true
        spawn_rate = 180.0
        seed = 41
        sim_mode = "gpu"
        render_mode = "billboard"
        profile = "res://particles/campfire.ppart"
        params = (1.8, 0.0, 0.0, 0.0)
    [/ParticleEmitter3D]
[/Campfire]
```

Set a project-wide default backend in `project.toml` (per-emitter `sim_mode` overrides it):

```toml
[graphics]
particle_sim_default = "cpu" # cpu | hybrid | gpu
```

## Reference

# Particle System Guide

Perro exposes a flexible particle system centered around:

- `ParticleEmitter3D` for spawning/controlling 3D particles
- `ParticleEmitter2D` for spawning/controlling 2D particles
- `.ppart` profiles for per-particle mathematical behavior

`.ppart` authoring details are documented here:

- [`.ppart` Format](ppart.md)

## Overview

Perro particles are math-driven. You author equations per particle (`x`, `y`, `z`) and combine them with presets and built-in variables/functions.
`ParticleEmitter2D` reads `x` and `y`; `z`, `force_z`, `dir_z`, `vel_z`, and `emitter_z` do not affect 2D output.

The emitter handles spawn orchestration:

- active/looping/prewarm control
- spawn rate
- random seed
- param injection (`params[i]`) for reusable profile logic
- simulation backend and render mode selection

## Simulation Backends

`ParticleEmitter3D.sim_mode` supports:

- `cpu`: CPU simulation and submission
- `hybrid`: GPU vertex-driven path for supported non-custom workloads
- `gpu`: full GPU compute-driven simulation/render path
- `default`: resolves from `project.toml` (`graphics.particle_sim_default`)

This lets you choose between maximum compatibility (`cpu`) and high-throughput GPU execution (`gpu`) per emitter.

## Render Modes

`ParticleEmitter3D.render_mode` supports:

- `point`
- `billboard`

You can pair render mode with any simulation mode; choose based on visual style and cost.

## How Emitter + Profile Fit Together

`ParticleEmitter3D` / `ParticleEmitter2D` define when/how many particles spawn.
`.ppart` defines what each particle does over its lifetime.

That split keeps effects composable:

- one profile can be reused by many emitters
- emitters can supply different seeds/params/rates to get distinct looks from the same profile

## Project-Level Default

Set a project default backend in `project.toml`:

```toml
[graphics]
particle_sim_default = "cpu" # cpu | hybrid | gpu
```

Per-emitter `sim_mode` can override this.
