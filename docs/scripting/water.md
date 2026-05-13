# Water Bodies

`WaterBody2D` and `WaterBody3D` define simulated water surfaces.

They render water, run a GPU height/foam simulation, and feed buoyancy forces into rigid bodies during fixed physics.

Use water bodies for pools, rivers, lakes, ocean patches, or gameplay zones where bodies should float and slow down.

## Authoring

2D water uses `Node2D` transform data.
The water plane covers `size.x` by `size.y` around the node position.
Height is along world `y`.

```text
[Pond]
    [WaterBody2D]
        size = (64, 24)
        resolution = (256, 128)
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
        sample_readback_rate = 30
        shoreline_mask = false
        static_body_wakes = true
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
The water plane covers local `x/z`.
Height is world `y`.
`size.x` maps to world `x`; `size.y` maps to world `z`.

```text
[Lake]
    [WaterBody3D]
        size = (128, 128)
        resolution = 256
        depth = 12.0
        flow = (0, 0.25)
        wind = (1, 0)
        idle_mode = "chop"
        wave_speed = 1.0
        wave_scale = 1.0
        damping = 0.985
        buoyancy = 1.5
        drag = 0.35
        wake_strength = 1.0
        foam_strength = 0.65
        lod_near = 128
        lod_mid = 384
        lod_far = 896
        min_resolution = 32
        [Node3D]
            position = (0, 0, 0)
            visible = true
        [/Node3D]
    [/WaterBody3D]
[/Lake]
```

## Fields

- `size`: surface width/depth in world units.
- `resolution` or `sim_resolution`: authored simulation grid size. Accepts one number or `(x, y)`. Scene load clamps to `1..4096`; GPU simulation clamps the effective grid to `8..256` per axis.
- `depth`: visual/physics water depth hint.
- `flow`: water current in surface-local axes.
- `wind`: wave direction for idle modes.
- `idle_mode` or `idle`: `"calm"`, `"sine"`, `"chop"`/`"choppy"`, `"storm"`, or `"river"`.
- `wave_speed`: idle wave time scale.
- `wave_scale`: idle wave height scale.
- `damping`: simulation damping, clamped to `0..1`.
- `buoyancy`: upward force multiplier for rigid bodies inside the surface bounds.
- `drag`: vertical velocity damping applied while submerged.
- `wake_strength`: wake impulse scale used by the water simulation.
- `foam_strength`: foam response scale.
- `sample_readback_rate` or `readback_rate`: target GPU sample readback rate. Renderer uses the max requested rate across visible water bodies.
- `lod_near_distance`/`lod_near`, `lod_mid_distance`/`lod_mid`, `lod_far_distance`/`lod_far`: camera distance thresholds for lower simulation resolution and lower physics force detail.
- `lod_min_resolution` or `min_resolution`: lowest effective simulation resolution. GPU clamps it to `8..256`.
- `shoreline_mask` or `coastline`: enable shoreline/coast masking path.
- `static_body_wakes`: allow static bodies to shape wakes.
- `debug`: enable debug water view.

Defaults:

- `WaterBody2D`: `size = (32, 32)`, `resolution = (128, 128)`, `depth = 4`.
- `WaterBody3D`: `size = (128, 128)`, `resolution = (128, 128)`, `depth = 12`.
- Shared defaults: `idle_mode = "calm"`, `wave_speed = 1`, `wave_scale = 1`, `damping = 0.985`, `buoyancy = 1`, `drag = 0.35`, `wake_strength = 1`, `foam_strength = 0.65`, `sample_readback_rate = 30`, `lod_near = 128`, `lod_mid = 384`, `lod_far = 896`, `min_resolution = (32, 32)`.

## Runtime Work

The GPU simulates water cells for all visible water bodies.
Effective grid resolution drops with camera distance: full near, half mid, quarter far, and eighth beyond far.
Ripples also fade with distance.

Water samples are read back from the GPU for physics.
If no GPU sample is ready, physics uses an analytic idle wave fallback from the same water settings.
This keeps physics deterministic enough to run even when GPU readback lags.

## Physics Interaction

Water bodies do not create colliders.
They do not block raycasts, shape casts, contact pairs, or area signals by themselves.

1. Runtime finds all `WaterBody2D` and `WaterBody3D` nodes.
2. Runtime tests rigid body centers against each water rectangle.
3. Runtime samples surface height at the body local point.
4. Runtime scales the force by water LOD distance from the active camera.
5. If the body center is below the sampled surface, runtime queues an upward force plus vertical drag when force is above the LOD deadzone.
6. Normal physics force/impulse application and world stepping run after that.

Physics LOD:

- Near: full force, no deadzone.
- Mid: force fades to `0.75x`, small deadzone.
- Far: force fades to `0.4x`, larger deadzone.
- Beyond far: `0.25x` force, `0.5` deadzone.

2D water affects `RigidBody2D`.
It uses body `density` in the buoyancy calculation.

3D water affects `RigidBody3D`.
It uses body `mass` in the buoyancy calculation.

Static bodies and areas are not moved by buoyancy.
Use separate `StaticBody2D`/`StaticBody3D` or `Area2D`/`Area3D` nodes if water needs collision walls, sensor triggers, audio occlusion, or gameplay volumes.

## Design Idea

Water is split from collision on purpose.
The water node owns surface simulation, visual state, wake/foam parameters, LOD, and buoyancy sampling.
Physics bodies keep owning collision, contact, and query behavior.

This keeps common authoring simple:

- Add water node for visual water and float force.
- Add collider nodes only where solid banks, floor, rocks, or triggers are needed.
- Tune `buoyancy`, `drag`, and `flow` for feel without editing body shapes.
