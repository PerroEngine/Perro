# Scene Node Templates

These are copy/paste authoring templates for `.scn` files.
Every node template lists the fields it exposes, including fields that default to nil/empty.

Conventions used below:

- `parent = PARENTKEY` is a placeholder parent node key (for example `@root` or another node name).
- `script = "res://path/to/script.rs"` is an example script path.
- `res://path/to/...` placeholders show expected path shape.

General wrapper (it might look like this):

```text
[name]
parent = PARENTKEY
script = "res://path/to/script.rs"

    [Type]
        field = value
        [AncestorType]
            field = value
        [/AncestorType]
    [/Type]
[/name]
```

## 2D Templates

```text
[node2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Node2D]
        position = (0, 0)
        rotation = 0.0
        scale = (1, 1)
        z_index = 0
        visible = true
    [/Node2D]
[/node2d]

[sprite2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Sprite2D]
        texture = "res://path/to/texture.png"
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Sprite2D]
[/sprite2d]

[camera2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Camera2D]
        zoom = 0.0
        post_processing = []
        active = false
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Camera2D]
[/camera2d]

[collision_shape_2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [CollisionShape2D]
        shape = { type = quad width = 1.0 height = 1.0 }
        sensor = false
        friction = 0.7
        restitution = 0.0
        density = 1.0
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/CollisionShape2D]
[/collision_shape_2d]

[static_body_2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [StaticBody2D]
        enabled = true
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/StaticBody2D]
[/static_body_2d]

[rigid_body_2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [RigidBody2D]
        enabled = true
        linear_velocity = (0, 0)
        angular_velocity = 0.0
        gravity_scale = 1.0
        linear_damping = 0.0
        angular_damping = 0.0
        can_sleep = true
        lock_rotation = false
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/RigidBody2D]
[/rigid_body_2d]

[area2d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Area2D]
        enabled = true
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Area2D]
[/area2d]
```

## 3D Templates

```text
[node3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Node3D]
        position = (0, 0, 0)
        rotation = (0, 0, 0, 1)
        scale = (1, 1, 1)
        visible = true
    [/Node3D]
[/node3d]

[mesh_instance_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [MeshInstance3D]
        mesh = "res://path/to/model.glb:mesh[0]"
        material = "res://path/to/material.pmat"
        model = "res://path/to/model.glb"
        skeleton = "SkeletonNodeName"
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/MeshInstance3D]
[/mesh_instance_3d]

[camera3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Camera3D]
        zoom = 0.0
        projection = perspective
        perspective_fov_y_degrees = 60.0
        perspective_near = 0.1
        perspective_far = 1000.0
        orthographic_size = 10.0
        orthographic_near = 0.1
        orthographic_far = 1000.0
        frustum_left = -1.0
        frustum_right = 1.0
        frustum_bottom = -1.0
        frustum_top = 1.0
        frustum_near = 0.1
        frustum_far = 1000.0
        post_processing = []
        active = false
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/Camera3D]
[/camera3d]

[collision_shape_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [CollisionShape3D]
        shape = { type = cube size = (1, 1, 1) }
        sensor = false
        friction = 0.7
        restitution = 0.0
        density = 1.0
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/CollisionShape3D]
[/collision_shape_3d]

[static_body_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [StaticBody3D]
        enabled = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/StaticBody3D]
[/static_body_3d]

[rigid_body_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [RigidBody3D]
        enabled = true
        linear_velocity = (0, 0, 0)
        angular_velocity = (0, 0, 0)
        gravity_scale = 1.0
        linear_damping = 0.0
        angular_damping = 0.0
        can_sleep = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/RigidBody3D]
[/rigid_body_3d]

[area3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Area3D]
        enabled = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/Area3D]
[/area3d]

[skeleton3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Skeleton3D]
        skeleton = "res://path/to/model.glb:skeleton[0]"
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/Skeleton3D]
[/skeleton3d]

[terrain_instance_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [TerrainInstance3D]
        show_debug_vertices = true
        show_debug_edges = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/TerrainInstance3D]
[/terrain_instance_3d]

[particle_emitter_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [ParticleEmitter3D]
        active = true
        looping = true
        prewarm = false
        spawn_rate = 256.0
        seed = 1
        params = []
        profile = "res://path/to/profile.ppart"
        sim_mode = default
        render_mode = point
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/ParticleEmitter3D]
[/particle_emitter_3d]
```

## Lights And Resource Templates

```text
[ambient_light_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [AmbientLight3D]
        color = (1, 1, 1)
        intensity = 0.0
        active = true
    [/AmbientLight3D]
[/ambient_light_3d]

[sky3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [Sky3D]
        day_colors = [
        (0.55, 0.82, 1.0),
        (0.38, 0.68, 0.95),
        (0.18, 0.45, 0.82)
        ]
        night_colors = [(0.01, 0.02, 0.06), (0.04, 0.06, 0.15), (0.09, 0.12, 0.25)]
        sky_angle = 0.0
        time = { time_of_day = 0.25 paused = false scale = 1.0 }
        time_of_day = 0.25
        time_paused = false
        time_scale = 1.0
        cloud_size = 0.85
        cloud_density = 0.72
        cloud_variance = 0.28
        wind_vector = (0.06, 0.015)
        star_size = 1.0
        star_scatter = 0.25
        star_gleam = 0.4
        moon_size = 0.6
        sun_size = 1.0
        sky_shader = "res://path/to/sky.wgsl"
        active = true
    [/Sky3D]
[/sky3d]

[ray_light_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [RayLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        active = true
        visible = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/RayLight3D]
[/ray_light_3d]

[point_light_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [PointLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        range = 10.0
        active = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/PointLight3D]
[/point_light_3d]

[spot_light_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [SpotLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        range = 12.0
        inner_angle_radians = 0.34906584
        outer_angle_radians = 0.5235988
        active = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/SpotLight3D]
[/spot_light_3d]

[animation_player]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [AnimationPlayer]
        animation = "res://path/to/clip.panim"
        bindings = []
        speed = 1.0
        paused = false
        playback = loop
    [/AnimationPlayer]
[/animation_player]
```
