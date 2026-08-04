# Water Bodies

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |

## Purpose

`WaterBody2D` and `WaterBody3D` add a simulated water surface that renders, runs a GPU height simulation, pushes buoyancy on rigid bodies, and reports enter/exit overlaps like an area. One node covers the look of the water, the float physics, and the "is the player in the water" question for pools, rivers, lakes, and ocean patches.

## Use Cases

- Floating and drifting props (barrels, boats, debris): drop a `WaterBody3D` and tune `buoyancy`, `drag`, and `flow`; each `RigidBody3D`'s `density` sets how high it rides.
- Swim state, drowning damage, or muffled audio when a character is submerged: connect the water's `Entered` / `Occupied` / `Exited` signals (named `<WaterNodeName>_Entered`, etc., like `Area2D`/`Area3D`) with `signal_connect!`.
- Rivers that carry objects downstream: `idle_mode = "river"` with a non-zero `flow`.
- Splashes from blasts and abilities: a `PhysicsForceEmitter3D` / `PhysicsForceEmitter2D` with `affect_water = true` turns its force events into wakes.
- Natural shorelines and banks: static collision shapes that pass the water mask cut coastline holes and damp waves against the edge.

## Decision Guide

Use a water body when one region needs the surface, overlap state, and buoyancy to agree. Use an `Area2D` / `Area3D` plus a visual effect when gameplay only needs an enter/exit volume; that avoids wave simulation and buoyancy. Keep swim rules in a script that listens to water signals, while the water node owns fluid behavior.

## Practical Example

A lake with a wooden crate that floats. The `WaterBody3D` supplies the surface and buoyancy; the crate is an ordinary `RigidBody3D` whose `density` controls buoyancy.

```text
[Lake]
    [WaterBody3D]
        shape = { type="cube", size=(64, 8, 64) }
        idle_mode = "chop"
        buoyancy = 1.5
        drag = 0.35
        [Node3D/]
    [/WaterBody3D]
[/Lake]

[Crate]
    [RigidBody3D]
        density = 0.6
        [Node3D]
            position = (0, 6, 0)
        [/Node3D]
    [/RigidBody3D]
[/Crate]

[CrateShape]
parent = @Crate
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
    [/CollisionShape3D]
[/CrateShape]
```

The crate drops, sinks until buoyancy balances gravity, then bobs with the surface. Lower `density` floats higher; raise it above the water's effective density and the crate sinks.

## Reference

`WaterBody2D` and `WaterBody3D` define simulated water surfaces.

They render water, run a GPU height simulation, and feed buoyancy forces into rigid bodies during fixed physics.

Use water bodies for pools, rivers, lakes, ocean patches, or gameplay zones where bodies should float and slow down.

## Authoring

2D water uses `Node2D` transform data.
The water surface uses `shape` around the node position.
Height is along world `y`.

```text
[Pond]
    [WaterBody2D]
        shape = { type="quad", width=64, height=24 }
        quality = "medium"
        depth = 5.0
        flow = (0.5, 0)
        wind = (1, 0)
        idle_mode = "sine"
        wave_speed = 1.2
        wave_scale = 0.6
        damping = 0.98
        buoyancy = 2.0
        drag = 0.45
        wake_strength = 1.4
        foam_strength = 0.7
        deep_color = (0.02, 0.16, 0.28, 0.94)
        shallow_color = (0.08, 0.46, 0.62, 0.74)
        shallow_depth = 8.0
        sample_readback_rate = 30
        collision_layers = all
        collision_mask = none
        coastline = { foam_color=(0.9, 0.97, 1.0, 1.0) foam_strength=0.75 foam_width=1.5 cutoff_softness=0.25 wave_reflection=0.45 wave_damping=0.35 edge_noise=0.2 }
        debug = false
        [Node2D]
            position = (0, 0)
            z_index = 0
            visible = true
        [/Node2D]
    [/WaterBody2D]
[/Pond]
```

3D water uses `Node3D` transform data.
The water surface uses `shape` in local `x/z`.
Height is world `y`.

```text
[Lake]
    [WaterBody3D]
        shape = { type="cube", size=(128, 12, 128) }
        quality = "high"
        depth = 12.0
        flow = (0, 0.25)
        wind = (1, 0)
        idle_mode = "chop"
        wave_speed = 1.0
        wave_scale = 1.0
        damping = 0.985
        buoyancy = 1.5
        drag = 0.35
        wake_strength = 1.35
        foam_strength = 0.9
        optics = { deep_color=(0.02, 0.16, 0.28, 0.94) shallow_color=(0.08, 0.46, 0.62, 0.74) sky_bias={ ratio=0.35 } }
        [Node3D]
            position = (0, 0, 0)
            visible = true
        [/Node3D]
    [/WaterBody3D]
[/Lake]
```

## Fields

- `shape`: water bounds. 2D accepts `rect`/`quad` and `circle`. 3D accepts `cube`/`box`, `cylinder`, or `sphere` as a cylinder shortcut.
- 2D quad/rect surface axes are local `x/y`.
- 3D box/cylinder surface axes are local `x/z`; height/depth is local/world `y`.
- `quality` (aliases `water_quality`, `fidelity`): the single fidelity knob. Accepts `"low"`, `"medium"`, `"high"`, or `"ultra"`; value aliases are `fast`/`lowest` for low, `mid`/`med` for medium, and `max`/`highest` for ultra. Defaults to `"low"`, so authors opt in to more detail.
- The tier is a target triangle edge length in screen pixels: low is about 32px, medium about 20px, high about 12px, ultra about 8px. The engine derives tessellation per render chunk each frame from chunk distance, camera projection, and render-target height, so one tier gives the same on-screen triangle density regardless of body size, camera distance, window size, or `render_scale`.
- The tier also sets the GPU simulation grid per axis (low `64x64`, medium `96x96`, high `160x160`, ultra `256x256`) and the default `sample_readback_rate` (low `10`, medium `20`, high `30`, ultra `60`).
- 2D water renders as a screen quad, so in 2D `quality` only changes the simulation grid and readback rate.
- `depth`: visual/physics water depth hint.
- `flow`: water current in surface-local axes.
- `wind`: wave direction for idle modes.
- `idle_mode` or `idle`: `"calm"`, `"sine"`, `"chop"`/`"choppy"`, `"storm"`, or `"river"`. River mode rushes along `flow`; if `flow = (0, 0)`, it falls back to `wind`.
- `wave_speed`: idle wave time scale. `1` is a slow default; old fast motion is closer to `5`.
- `wave_scale`: idle wave height scale.
- `wave_length`, `wavelength`, or `wave_size`: world-space wave profile length in meters. Defaults do not scale wave size from water body bounds.
- `chop` and `storm` layer several world-space wave directions so large water does not become one broad sine sheet. `storm` also adds moving steep swell peaks for rough water.
- `damping`: simulation damping, clamped to `0..1`.
- `buoyancy`: upward force multiplier for rigid bodies inside the surface bounds.
- `drag`: vertical velocity damping applied while submerged.
- `wake_strength`: wake impulse scale used by the water simulation.
- `foam_strength`: simulation foam response scale.
- `sample_readback_rate` or `readback_rate`: target GPU sample readback rate. `quality` picks the default; an explicit value overrides the tier default. Renderer uses the max requested rate across visible water bodies.
- `deep_color` and `shallow_color`: water color/opacity endpoints. Surface color derives between them from depth, waves, Fresnel, and refraction tint. Shallow alpha should usually be lower than deep alpha, but default water stays mostly opaque.
- `shallow_depth`: visual depth cutoff where water finishes fading from shallow color/alpha toward deep color/alpha. `-1` uses the automatic old scale. Use larger values for fish tanks or clear pools that should stay see-through.
- `sky_bias`: optional active `Sky3D` color pull. Use `sky_bias = "none"`, `sky_bias = 0.0`, or `sky_bias = { ratio=0.35 }`. `optics = { ... }` accepts the same color, `shallow_depth`, and sky fields.
- `material` or `visual`: WaterMaterial-style render knobs: `transparency`, `reflectivity`, `roughness`, `fresnel_power`, `normal_strength`, `ripple_scale`, `foam_color`, `foam_amount`, `crest_foam_threshold`, `caustic_strength`, `refraction_strength`, `scattering_strength`, and `distance_fog_strength`.
- `collision_layers`: water sensor tagged layers. Defaults to all layers.
- `collision_mask`: tagged layers water ignores for buoyancy, wakes, and coastline. Defaults to no layers.
- `link_layers`: water link layers. Defaults to all layers.
- `link_mask`: water link layers ignored for automatic cross-body blending. Defaults to no layers.
- `blend_width`: explicit overlap blend width. `0` picks an automatic cubic blend width from the overlap size.
- `wave_transfer`: wave transfer multiplier across linked water. Defaults to `1`. Foam transfer fields stay compatible, but 3D visual foam is disabled.
- `flow_transfer`: flow velocity transfer multiplier across linked water. Defaults to `1`.
- `coastline`: static-body shoreline cut settings. Foam/color outline fields stay compatible, but 3D visual foam/outlines are disabled.
- `debug`: enable debug water view.

Defaults:

- `WaterBody2D`: `shape = { type="quad", width=32, height=32 }`, `quality = "low"`, `depth = 4`.
- `WaterBody3D`: `shape = { type="cube", size=(500, 35, 500) }`, `quality = "low"`, `depth = 35`.
- Shared defaults: `shallow_depth = -1`, `sky_bias = "none"`, `sample_readback_rate = 10` (from the `"low"` tier), `collision_layers = all`, `collision_mask = []`, `link_layers = all`, `link_mask = []`, `blend_width = 0`, `wave_transfer = 1`, `flow_transfer = 1`.

Removed fields:

The old fidelity knobs are gone; `quality` replaces all of them. Scenes that still set one get a `[perro][runtime]` warning at scene load, the field does not resolve, and `perro doctor` reports it as an error.

- Absolute grid sizes: `resolution`, `sim_resolution`, `render_resolution`, `mesh_resolution`.
- Per-meter densities: `vertices_per_meter`, `verts_per_meter`, `vpm`, `resolution_per_meter`, `sim_vertices_per_meter`, `sim_cells_per_meter`, `simulation_cells_per_meter`, `render_vertices_per_meter`, `render_verts_per_meter`, `mesh_vertices_per_meter`.
- LOD distance bands and floors: `lod_near_distance`/`lod_near`, `lod_mid_distance`/`lod_mid`, `lod_far_distance`/`lod_far`, `lod_min_resolution`, `min_resolution`.

Buoyancy force falloff with camera distance is now a fixed engine constant (128/384/896 meter bands) instead of a per-body setting.

## Runtime Work

The GPU simulates water cells inside the water shape bounds, on the grid the `quality` tier picks.
The simulation grid never changes with camera distance, so XZ height samples and buoyancy stay stable while render detail moves.
Intersecting water bodies auto-link when link layers/masks allow it.
Linked bodies keep separate simulation grids, but overlap samples use a cubic blend for surface height, flow, buoyancy, and wake transfer.

Water meshes are split into render chunks derived from body world size: about 12 world units per chunk, up to 8 chunks per axis.
Each chunk picks its own LOD from its own distance to the camera, so the far half of a large body tessellates coarser than the near half.
Adjacent chunks never crack: the LOD ratio between neighbours is capped at 4x, and the finer chunk snaps its boundary vertices onto the coarser neighbour's vertices.
Because the target is a triangle edge in screen pixels, chunk detail already accounts for camera distance, window size, and `render_scale`.
3D mid/far water uses a cheaper shader path for lower GPU cost.

Water samples are read back from the GPU for physics.
If no GPU sample is ready, physics uses an analytic idle wave fallback from the same water settings.
This keeps physics deterministic enough to run even when GPU readback lags.

## Physics Interaction

Water bodies create sensor colliders.
They do not block motion, raycasts, or contact pairs.
They emit `WaterNodeName_Entered`, `WaterNodeName_Occupied`, and `WaterNodeName_Exited` like `Area2D`/`Area3D`.

1. Runtime finds all `WaterBody2D` and `WaterBody3D` nodes.
2. Runtime tests rigid body centers against each water shape.
3. Runtime samples surface height at the body local point.
4. Runtime scales the force by water LOD distance from the active camera.
5. If the body center is below the sampled surface, runtime queues an upward force plus vertical drag when force is above the LOD deadzone.
6. Normal physics force/impulse application and world stepping run after that.

Physics LOD, using fixed engine distance bands of 128, 384, and 896 meters:

- Near: full force, no deadzone.
- Mid: force fades to `0.75x`, small deadzone.
- Far: force fades to `0.4x`, larger deadzone.
- Beyond far: `0.25x` force, `0.5` deadzone.

2D water affects `RigidBody2D`.
It uses body `density` in the buoyancy calculation.

3D water affects `RigidBody3D`.
It uses body `density` in the buoyancy calculation.

Static bodies are not moved by buoyancy.
Static collision shapes that pass the water/body mask test cut coastline holes and damp waves. 3D shoreline foam/outlines are disabled for now.

Physics force emitters also affect water.
`PhysicsForceEmitter2D` and `PhysicsForceEmitter3D` send nearby force events into water when `affect_water = true`.
Water converts those events into wakes and a cavitation scalar. 2D still uses foam; 3D visual foam is disabled for now.
Explosion, lift, current, vortex, and custom force profiles all use the same water interaction path.

## Design Idea

Water owns surface simulation, visual state, sensor overlap, wake parameters, LOD, coastline masking, and buoyancy sampling.
Static/rigid bodies keep owning solid collision and contact behavior.

This keeps common authoring simple:

- Add water node for visual water, sensor overlap, and float force.

## GPU visual capture and benchmark

Run a focused water case with a stable capture window:

```powershell
$env:PERRO_GPU_BENCH = "water_sim_1_64"
$env:PERRO_GPU_BENCH_THROUGHPUT = "1"
$env:PERRO_GPU_CAPTURE_MS = "5000"
$env:PERRO_GPU_BENCH_CSV = "target/water-gpu-bench.csv"
cargo bench -p perro_graphics --bench gpu_frame
```

`PERRO_GPU_CAPTURE_MS` keeps the final rendered frame visible for capture.
`PERRO_GPU_BENCH_THROUGHPUT` queues frames like the uncapped runtime and waits for the GPU once after the sample batch. Without it, the benchmark waits after every frame for isolated latency measurements.
`PERRO_GPU_BENCH_CSV` appends CPU, GPU-main, GPU-water, draw-call, and instance timing data.
Use `water_sim`, `water_idle`, or another case substring to select the workload.
- Add static collider nodes for solid banks, floor, rocks, docks, and islands.
- Tune `buoyancy`, `drag`, and `flow` for feel without editing body shapes.
