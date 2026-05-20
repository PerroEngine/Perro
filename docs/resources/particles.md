# Particle System Guide

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Particle System Guide` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
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
