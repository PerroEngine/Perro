# Water Bodies

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
        resolution = (128, 64)
        render_resolution = (192, 96)
        lod_min_resolution = (32, 16)
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
        resolution = (128, 128)
        render_resolution = (256, 256)
        lod_min_resolution = (32, 32)
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

- `shape`: water bounds. 2D accepts `rect`/`quad` and `circle`. 3D accepts `cube`/`box`, `cylinder`, or `sphere` as a cylinder shortcut.
- 2D quad/rect surface axes are local `x/y`.
- 3D box/cylinder surface axes are local `x/z`; height/depth is local/world `y`.
- `vertices_per_meter` or `verts_per_meter`: legacy direct density. It sets both simulation and render resolution from shape surface size.
- `sim_cells_per_meter`: simulation density only. Runtime derives `sim_resolution` from shape surface size.
- `render_vertices_per_meter`: render mesh density only. Runtime derives `render_resolution` from shape surface size.
- `resolution` or `sim_resolution`: absolute simulation grid size. Accepts one number or `(x, y)`. Scene load clamps to `1..4096`; GPU simulation clamps effective grid to `1..256` per axis.
- `render_resolution`: absolute render mesh grid size. 3D visual tessellation clamps to a fixed GPU cap and drops with camera distance while simulation stays stable.
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
- `sample_readback_rate` or `readback_rate`: target GPU sample readback rate. Renderer uses the max requested rate across visible water bodies.
- `deep_color` and `shallow_color`: water color/opacity endpoints. Surface color derives between them from depth, waves, Fresnel, and refraction tint. Shallow alpha should usually be lower than deep alpha, but default water stays mostly opaque.
- `shallow_depth`: visual depth cutoff where water finishes fading from shallow color/alpha toward deep color/alpha. `-1` uses the automatic old scale. Use larger values for fish tanks or clear pools that should stay see-through.
- `sky_bias`: optional active `Sky3D` color pull. Use `sky_bias = "none"`, `sky_bias = 0.0`, or `sky_bias = { ratio=0.35 }`. `optics = { ... }` accepts the same color, `shallow_depth`, and sky fields.
- `material` or `visual`: WaterMaterial-style render knobs: `transparency`, `reflectivity`, `roughness`, `fresnel_power`, `normal_strength`, `ripple_scale`, `foam_color`, `foam_amount`, `crest_foam_threshold`, `caustic_strength`, `refraction_strength`, `scattering_strength`, and `distance_fog_strength`.
- `lod_near_distance`/`lod_near`, `lod_mid_distance`/`lod_mid`, `lod_far_distance`/`lod_far`: camera distance thresholds for render mesh detail and physics force detail.
- `lod_min_resolution` or `min_resolution`: lowest 2D effective simulation resolution inside `lod_far`. GPU clamps it to `1..256`. 3D keeps simulation resolution stable and only LODs render detail.
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

- `WaterBody2D`: `shape = { type="quad", width=32, height=32 }`, `resolution = (17, 17)`, `render_resolution = (33, 33)`, `depth = 4`.
- `WaterBody3D`: `shape = { type="cube", size=(500, 35, 500) }`, mid-quality ocean defaults, `sim_resolution = (4096, 4096)`, `render_resolution = (4096, 4096)`.
- Shared defaults: `shallow_depth = -1`, `sky_bias = "none"`, `sample_readback_rate = 30`, `lod_near = 128`, `lod_mid = 384`, `lod_far = 896`, `min_resolution = (32, 32)`, `collision_layers = all`, `collision_mask = []`, `link_layers = all`, `link_mask = []`, `blend_width = 0`, `wave_transfer = 1`, `flow_transfer = 1`.

## Runtime Work

The GPU simulates water cells inside the water shape bounds.
3D water keeps its simulation grid active so XZ height samples stay stable while render LOD changes.
Intersecting water bodies auto-link when link layers/masks allow it.
Linked bodies keep separate simulation grids, but overlap samples use a cubic blend for surface height, flow, buoyancy, and wake transfer.
2D effective simulation and render grid resolution drop separately with quadratic camera distance falloff, then turn off beyond `lod_far`.
3D render grid resolution drops with distance, but 3D simulation resolution stays camera-stable for height samples and buoyancy.
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

Physics LOD:

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
- Add static collider nodes for solid banks, floor, rocks, docks, and islands.
- Tune `buoyancy`, `drag`, and `flow` for feel without editing body shapes.
