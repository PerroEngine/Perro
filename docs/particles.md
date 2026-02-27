# Particle System Guide

Perro exposes a flexible 3D particle system centered around:

- `ParticleEmitter3D` for spawning/controlling particles
- `.pparticle` profiles for per-particle mathematical behavior

`.pparticle` authoring details are documented here:

- [`.pparticle` Format](resources/pparticle.md)

## Overview

Perro particles are math-driven. You author equations per particle (`x`, `y`, `z`) and combine them with presets and built-in variables/functions.

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

`ParticleEmitter3D` defines when/how many particles spawn.  
`.pparticle` defines what each particle does over its lifetime.

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
