# Scene Node Templates

These are copy/paste authoring templates for `.scn` files.
Every node template lists the fields it exposes, including fields that default to nil/empty.

Conventions used below:

- `$root = @NODEKEY` is required. It marks the scene root and defines `$root` as a reusable node ref.
- `parent = @PARENTKEY` is a placeholder parent node key. Use `$root` for the scene root, or `@OtherNode` for a direct node ref.
- Use `@NodeKey` whenever a scene value needs a node ref, including `script_vars` and animation bindings.
- A node ref is `@` plus the full scene key. Examples: `[Name]` -> `@Name`, `[@Name]` -> `@@Name`, `[@@Name]` -> `@@@Name`.
- In `.panim` and `.panimtree`, `@Name` marks animation object refs or graph refs, then scene bindings map object names to `@NodeKey`.
- `script = "res://path/to/script.rs"` is an example script path.
- `res://path/to/...` placeholders show expected path shape.

Root example:

```text
$root = @Main

[Main]
    [Node2D]
    [/Node2D]
[/Main]

[Child]
parent = $root
    [Node2D]
    [/Node2D]
[/Child]
```

`$root` is a special scene variable. It must be assigned to a node ref with `@`.
Use `$root` later anywhere a node ref is accepted, such as `parent = $root`.

Escaped `@` key example:

```text
$root = @@@Main

[@@Main]
    [Node2D]
    [/Node2D]
[/@@Main]

[Child]
parent = @@@Main
    [Node2D]
    [/Node2D]
[/Child]
```

Node ref examples:

```text
script_vars = {
    target = @Enemy
}

[Anim]
    [AnimationPlayer]
        animation = "res://animations/idle.panim"
        bindings = { Hero = @PlayerRoot }
    [/AnimationPlayer]
[/Anim]
```

General wrapper (it might look like this):

```text
[name]
parent = @PARENTKEY
script = "res://path/to/script.rs"

    [Type]
        field = value
        [AncestorType]
            field = value
        [/AncestorType]
    [/Type]
[/name]
```

## Base Template

```text
[node]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Node]
    [/Node]
[/node]
```

## Scenes Inside Scenes (`root_of`)

`root_of` lets a node act like an **Imported Scene Instance** layered on top of a **Base Scene Template** root.
Think of it as:

- `final node = Base Scene Template root + Imported Scene Instance overrides`
- Base Scene Template children are imported under the host node
- Imported Scene Instance children still work normally

### Example

```text
$root = @Main

[Main]
root_of = "res://shared/player_base.scn"
script_vars = {
    speed = 9.5
    debug_only = __unset__
    tuning = { sprint = 2.2 }
}
    [Node2D]
        position = (16, 48)
    [/Node2D]
[/Main]

[ExtraHat]
parent = $root
    [Sprite2D]
        texture = "res://cosmetics/hat.png"
        [Node2D]
            position = (0, -6)
        [/Node2D]
    [/Sprite2D]
[/ExtraHat]
```

When you only want defaults from the Base Scene Template, you can omit the node type block entirely:

```text
[Main]
root_of = "res://shared/player_base.scn"
[/Main]
```

### Merge Rules

- `script`:
  - default: inherit from Base Scene Template root
  - if Imported Scene Instance sets `script = "..."`: instance replaces template script
  - if Imported Scene Instance sets `script = null`: inherited template script is removed
- `script_vars`:
  - default: map merge
  - Imported Scene Instance key wins on conflicts
  - use `__unset__` to remove inherited keys
  - nested objects are merged by key
- normal properties (position, rotation, etc.):
  - if Imported Scene Instance defines field: instance value wins
  - if Imported Scene Instance omits field: template value is kept
- arrays/lists:
  - Imported Scene Instance value replaces Base Scene Template value

### Notes

- Base Scene Template root type and Imported Scene Instance node type should match for field-level merging.
  - If types differ, Imported Scene Instance node data is used as-is.
- `root_of` expansion supports nesting (a Base Scene Template can itself use `root_of`).
- Cycles are rejected (`A` includes `B` includes `A`).

## 2D Templates

```text
[node2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Node2D]
        position = (0, 0)
        rotation = 0.0
        scale = (1, 1)
        z_index = 0
        visible = true
    [/Node2D]
[/node2d]

[skeleton2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Skeleton2D]
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Skeleton2D]
[/skeleton2d]

[bone2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Bone2D]
        rest = { position = (0, 0), rotation = 0.0, scale = (1, 1) }
        pose = { position = (0, 0), rotation = 0.0, scale = (1, 1) }
        inv_bind = { position = (0, 0), rotation = 0.0, scale = (1, 1) }
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Bone2D]
[/bone2d]

[sprite2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Sprite2D]
        texture = "res://path/to/texture.png"
        texture_region = (0, 0, 32, 32)
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/Sprite2D]
[/sprite2d]

[animated_sprite2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [AnimatedSprite2D]
        texture = "res://path/to/texture.png"
        current_animation = "idle"
        current_frame = 0
        fps_scale = 1.0
        playing = true
        looping = true
        animations = [
            { name = "idle", start = (0, 0), frame_size = (32, 32), frame_count = 4, fps = 8 },
            { name = "run", start = (0, 32), frame_size = (32, 32), frame_count = 6, fps = 12 },
            { name = "hurt_grid", start = (0, 64), frame_size = (32, 32), frame_count = 8, columns = 4, fps = 10 }
        ]
        [Node2D]
            position = (0, 0)
            rotation = 0.0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/AnimatedSprite2D]
[/animated_sprite2d]

[camera2d]
parent = @PARENTKEY
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [CollisionShape2D]
        shape = { type = quad width = 1.0 height = 1.0 }
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [StaticBody2D]
        enabled = true
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
    [/StaticBody2D]
[/static_body_2d]

[rigid_body_2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [RigidBody2D]
        enabled = true
        continuous_collision_detection = true
        linear_velocity = (0, 0)
        angular_velocity = 0.0
        gravity_scale = 1.0
        linear_damping = 0.0
        angular_damping = 0.0
        can_sleep = true
        lock_rotation = false
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
    [/RigidBody2D]
[/rigid_body_2d]

[area2d]
parent = @PARENTKEY
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Node3D]
        position = (0, 0, 0)
        rotation = (0, 0, 0, 1)
        scale = (1, 1, 1)
        visible = true
    [/Node3D]
[/node3d]

[mesh_instance_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [MeshInstance3D]
        mesh = "res://path/to/model.glb:mesh[0]"
        material = "res://path/to/material.pmat"
        surfaces = [
            "res://path/to/material0.pmat",
            {
                material = "res://path/to/material1.pmat"
                modulate = (1, 0.9, 0.9, 1)
                overrides = [
                    { name = "roughness", value = 0.25 },
                    { name = "shade_flat", value = true },
                    { name = "rim_color", value = (0.2, 0.6, 1.0, 1.0) }
                ]
            }
        ]
        model = "res://path/to/model.glb"
        skeleton = "SkeletonNodeName"
        meshlets = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/MeshInstance3D]
[/mesh_instance_3d]

[multi_mesh_instance_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [MultiMeshInstance3D]
        mesh = "res://path/to/model.glb:mesh[0]"
        material = "res://path/to/material.pmat"
        surfaces = [
            "res://path/to/material0.pmat",
            {
                material = "res://path/to/material1.pmat"
                modulate = (1, 0.9, 0.9, 1)
                overrides = [
                    { name = "roughness", value = 0.25 },
                    { name = "shade_flat", value = true }
                ]
            }
        ]
        # instance count = instances.len()
        instance_scale = 1.0
        meshlets = true
        instances = [
            {
                position = (6, 0, 0)
                rotation = (0, 0, 0, 1)
            },
            {
                position = (6, 0, 0)
                rotation_deg = (0, 45, 0)
            },
        ]
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/MultiMeshInstance3D]
[/multi_mesh_instance_3d]

[camera3d]
parent = @PARENTKEY
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [CollisionShape3D]
        shape = { type = cube, size = (1, 1, 1) }
        # alt: trimesh = "res://path/to/model.glb:mesh[0]"
        # alt: shape = { type = trimesh source = "res://path/to/model.glb:mesh[0]" }
        debug = false
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/CollisionShape3D]
[/collision_shape_3d]

[static_body_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [StaticBody3D]
        enabled = true
        friction = 0.7
        restitution = 0.0
        density = 1.0
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/StaticBody3D]
[/static_body_3d]

[rigid_body_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [RigidBody3D]
        enabled = true
        continuous_collision_detection = true
        mass = 1.0
        linear_velocity = (0, 0, 0)
        angular_velocity = (0, 0, 0)
        gravity_scale = 1.0
        linear_damping = 0.0
        angular_damping = 0.0
        can_sleep = true
        friction = 0.7
        restitution = 0.0
        density = 1.0
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/RigidBody3D]
[/rigid_body_3d]

[area3d]
parent = @PARENTKEY
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
parent = @PARENTKEY
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

[bone_attachment_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [BoneAttachment3D]
        skeleton = "SkeletonNodeName"
        bone = 0
        # alt: bone_index = 0
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/BoneAttachment3D]
[/bone_attachment_3d]

[physics_bone_chain_3d]
    [PhysicsBoneChain3D]
        skeleton = "SkeletonNodeName"
        bone = 0
        chain_length = 4
        gravity = (0, -9.81, 0)
        damping = 0.08
        stiffness = 0.35
        radius = 0.05
        collisions = true
        iterations = 4
    [/PhysicsBoneChain3D]
[/physics_bone_chain_3d]

[bone_collider_3d]
    [BoneCollider3D]
        enabled = true
        [CollisionShape3D]
            shape = { type="sphere", radius=0.5 }
        [/CollisionShape3D]
    [/BoneCollider3D]
[/bone_collider_3d]

[ik_target_3d]
parent = PARENTKEY
script = "res://path/to/script.rs"
    [IKTarget3D]
        skeleton = "SkeletonNodeName"
        bone = 0
        # alt: bone_index = 0
        chain_length = 2
        iterations = 8
        tolerance = 0.01
        weight = 1.0
        match_rotation = true
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/IKTarget3D]
[/ik_target_3d]

[particle_emitter_3d]
parent = @PARENTKEY
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

[particle_emitter_2d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [ParticleEmitter2D]
        active = true
        looping = true
        prewarm = false
        spawn_rate = 256.0
        seed = 1
        params = []
        profile = "res://path/to/profile.ppart"
        sim_mode = default
        [Node2D]
            position = (0, 0)
            rotation = 0
            scale = (1, 1)
            z_index = 0
            visible = true
        [/Node2D]
    [/ParticleEmitter2D]
[/particle_emitter_2d]
```

## UI Templates

`UiHBox` and `UiVBox` also work as aliases for `UiHLayout` and `UiVLayout`.
`hover` and `pressed` on `UiButton` accept any `UiBox` field plus style fields.
`.uistyle` resources let `style`, `hover.style`, `pressed.style`, and `focused_style` use `res://path/to/style.uistyle`.

UI templates use ratio-only sizing.

- `size_ratio` = size relative to parent.
- `min_size_ratio` + `max_size_ratio` clamp relative to node base size at creation.
- Example: `size_ratio = (0.5, 0.5)` => half parent size.
- Example: `min_size_ratio = (1.0, 1.0)` => never shrink below creation size.
- Example: `min_size_ratio = (0.8, 0.8)` + `max_size_ratio = (1.2, 1.2)` => allow ~20% shrink/grow band.

```text
[ui_box]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiBox]
        visible = true
        input_enabled = true
        mouse_filter = "stop"
        clip_children = false
        anchor = "center"
        position_ratio = (0.5, 0.5)
        size_ratio = (0.5, 0.5)
        pivot_ratio = (0.5, 0.5)

        scale = (1, 1)
        rotation = 0.0
        h_size = "fixed"
        v_size = "fixed"
        h_align = "center"
        v_align = "center"
        min_size_ratio = (1.0, 1.0)
        max_size_ratio = (inf, inf)
        padding = 0
        margin = 0
        z_index = 0
    [/UiBox]
[/ui_box]

[ui_panel]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiPanel]
        fill = (0.11, 0.12, 0.14, 0.92)
        stroke = (0.22, 0.24, 0.28, 1.0)
        stroke_width = 1.0
        radius = 0.2
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiPanel]
[/ui_panel]

[ui_button]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiButton]
        disabled = false
        cursor_icon = "pointer"
        hover_signals = []
        hover_exit_signals = []
        pressed_signals = []
        released_signals = []
        click_signals = []
        style = { fill = (0.18, 0.20, 0.24, 1.0) stroke = (0.32, 0.35, 0.40, 1.0) stroke_width = 1.0 radius = 0.2 shadow = { color = (0, 0, 0, 0) distance = 0 falloff = 0 vector = (0, -1) size = 1 } highlight = { color = (0, 0, 0, 0) distance = 0 falloff = 0 vector = (0, -1) size = 1 } }
        # Planned 1.0 alternative:
        # style = "res://ui/button.uistyle"
        hover = {
            style = { fill = (0.24, 0.27, 0.32, 1.0) stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.2 }
            # Planned 1.0 alternative:
            # style = "res://ui/button_hover.uistyle"
        }
        pressed = {
            style = { fill = (0.12, 0.14, 0.18, 1.0) stroke = (0.42, 0.46, 0.54, 1.0) stroke_width = 1.0 radius = 0.2 }
            # Planned 1.0 alternative:
            # style = "res://ui/button_down.uistyle"
        }
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiButton]
[/ui_button]

[ui_label]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiLabel]
        text = ""
        color = (1, 1, 1, 1)
        font_size = 16.0
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_h_align = "center"
        text_v_align = "center"
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiLabel]
[/ui_label]

[ui_text_box]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTextBox]
        text = ""
        placeholder = ""
        color = (1, 1, 1, 1)
        placeholder_color = (0.58, 0.62, 0.70, 1.0)
        selection_color = (0.25, 0.42, 0.85, 0.55)
        caret_color = (1, 1, 1, 1)
        font_size = 16.0
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_padding = { left = 8 top = 6 right = 8 bottom = 6 }
        editable = true
        hover_signals = []
        hover_exit_signals = []
        focused_signals = []
        unfocused_signals = []
        text_changed_signals = []
        style = { fill = (0.11, 0.12, 0.14, 0.92) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.2 }
        focused_style = { fill = (0.10, 0.11, 0.13, 0.96) stroke = (0.45, 0.58, 0.85, 1.0) stroke_width = 1.0 radius = 0.2 }
        # Planned 1.0 alternatives:
        # style = "res://ui/text_box.uistyle"
        # focused_style = "res://ui/text_box_focus.uistyle"
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTextBox]
[/ui_text_box]

[ui_text_block]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTextBlock]
        text = ""
        placeholder = ""
        color = (1, 1, 1, 1)
        placeholder_color = (0.58, 0.62, 0.70, 1.0)
        selection_color = (0.25, 0.42, 0.85, 0.55)
        caret_color = (1, 1, 1, 1)
        font_size = 16.0
        text_size_ratio = 0.5
        font_relative = false
        font_min_scale = 0.0
        font_max_scale = inf
        text_padding = { left = 8 top = 6 right = 8 bottom = 6 }
        editable = true
        hover_signals = []
        hover_exit_signals = []
        focused_signals = []
        unfocused_signals = []
        text_changed_signals = []
        style = { fill = (0.11, 0.12, 0.14, 0.92) stroke = (0.22, 0.24, 0.28, 1.0) stroke_width = 1.0 radius = 0.2 }
        focused_style = { fill = (0.10, 0.11, 0.13, 0.96) stroke = (0.45, 0.58, 0.85, 1.0) stroke_width = 1.0 radius = 0.2 }
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTextBlock]
[/ui_text_block]

[ui_layout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiLayout]
        mode = "h"
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiLayout]
[/ui_layout]

[ui_hlayout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiHLayout]
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiHLayout]
[/ui_hlayout]

[ui_vlayout]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiVLayout]
        spacing = 0.0
        h_spacing = 0.0
        v_spacing = 0.0
        columns = 1
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiVLayout]
[/ui_vlayout]

[ui_grid]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiGrid]
        columns = 1
        h_spacing = 0.0
        v_spacing = 0.0
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "center"
            v_align = "center"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiGrid]
[/ui_grid]

[ui_tree_list]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [UiTreeList]
        # roots, branches, and collapsed are usually set from script with NodeID values.
        indent = 16.0
        v_spacing = 0.0
        [UiBox]
            visible = true
            input_enabled = true
            mouse_filter = "stop"
            clip_children = false
            anchor = "center"
            position_ratio = (0.5, 0.5)
            size_ratio = (0.5, 0.5)
            pivot_ratio = (0.5, 0.5)

            scale = (1, 1)
            rotation = 0.0
            h_size = "fixed"
            v_size = "fixed"
            h_align = "left"
            v_align = "top"
            min_size_ratio = (1.0, 1.0)
            max_size_ratio = (inf, inf)
            padding = 0
            margin = 0
            z_index = 0
        [/UiBox]
    [/UiTreeList]
[/ui_tree_list]
```

## Bone Attachment Example

`BoneAttachment3D` binds to a `Skeleton3D` node plus bone index.
Children then inherit that bone transform.
Use it for socket nodes, like a sword in a hand.

```text
[CharacterSkeleton]
parent = @Character
    [Skeleton3D]
        skeleton = "res://characters/hero.glb:skeleton[0]"
    [/Skeleton3D]
[/CharacterSkeleton]

[RightHandSocket]
parent = @Character
    [BoneAttachment3D]
        skeleton = "CharacterSkeleton"
        bone = 15
    [/BoneAttachment3D]
[/RightHandSocket]

[Sword]
parent = @RightHandSocket
    [MeshInstance3D]
        mesh = "res://weapons/sword.glb:mesh[0]"
        material = "res://weapons/sword.pmat"
        [Node3D]
            position = (0.05, 0.0, 0.0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/MeshInstance3D]
[/Sword]
```

## Lights And Resource Templates

```text
[ambient_light_3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [AmbientLight3D]
        color = (1, 1, 1)
        intensity = 0.0
        cast_shadows = true
        active = true
    [/AmbientLight3D]
[/ambient_light_3d]

[sky3d]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Sky3D]
        day_colors = [
            (0.55, 0.82, 1.0),
            (0.38, 0.68, 0.95),
            (0.18, 0.45, 0.82)
        ]
        evening_colors = [
            (1.00, 0.62, 0.40),
            (0.95, 0.42, 0.58),
            (0.42, 0.20, 0.42)
        ]
        night_colors = [
            (0.01, 0.02, 0.06),
            (0.04, 0.06, 0.15),
            (0.09, 0.12, 0.25)
        ]
        sky_angle = 0.0
        time = { time_of_day = 0.25 paused = false scale = 1.0 }
        time_of_day = 0.25
        time_paused = false
        time_scale = 1.0
        cloud_size = 0.85
        cloud_density = 0.72
        cloud_variance = 0.28
        wind_vector = (0.1, 0.1)
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [RayLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        cast_shadows = true
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [PointLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        range = 10.0
        cast_shadows = true
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [SpotLight3D]
        color = (1, 1, 1)
        intensity = 1.0
        range = 12.0
        inner_angle_radians = 0.34906584
        outer_angle_radians = 0.5235988
        cast_shadows = true
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
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [AnimationPlayer]
        animation = "res://path/to/clip.panim"
        bindings = []
        speed = 1.0
        paused = false
        playback = loop
    [/AnimationPlayer]
[/animation_player]

[animation_tree]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [AnimationTree]
        tree = "res://path/to/tree.panimtree"
        animations = [
            { animation = "res://path/to/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://path/to/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://path/to/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang, speed = 1.0, paused = false },
        ]
        speed = 1.0
        paused = false
    [/AnimationTree]
[/animation_tree]
```

## Physics Parity Templates

Current body layer/mask fields:

```text
[body]
    [RigidBody2D]
        collision_layer = 1
        collision_mask = 4294967295
        [CollisionShape2D]
            shape = { type = "quad" width = 1 height = 1 }
        [/CollisionShape2D]
    [/RigidBody2D]
[/body]
```

2D joints:

```text
[rope_pin]
    [PinJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0.5)
        enabled = true
        collide_connected = false
    [/PinJoint2D]
[/rope_pin]

[distance_link]
    [DistanceJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0)
        min_distance = 0
        max_distance = 2
        enabled = true
        collide_connected = false
    [/DistanceJoint2D]
[/distance_link]

[fixed_link_2d]
    [FixedJoint2D]
        body_a = @AnchorBody
        body_b = @SwingBody
        anchor_a = (0, 0)
        anchor_b = (0, 0)
        enabled = true
        collide_connected = false
    [/FixedJoint2D]
[/fixed_link_2d]
```

3D joints:

```text
[ball_socket]
    [BallJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 1, 0)
        anchor_b = (-0.5, 1, 0)
        enabled = true
        collide_connected = false
    [/BallJoint3D]
[/ball_socket]

[door_hinge]
    [HingeJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 1, 0)
        anchor_b = (-0.5, 1, 0)
        axis = (0, 1, 0)
        enabled = true
        collide_connected = false
    [/HingeJoint3D]
[/door_hinge]

[fixed_link_3d]
    [FixedJoint3D]
        body_a = @FrameBody
        body_b = @DoorBody
        anchor_a = (0, 0, 0)
        anchor_b = (0, 0, 0)
        enabled = true
        collide_connected = false
    [/FixedJoint3D]
[/fixed_link_3d]
```

## TileMap2D Template

```text
[level]
    [TileMap2D]
        tileset = "res://tiles/world.ptileset"
        width = 8
        height = 4
        empty_tile = -1
        tiles = [
            1, 1, 1, 1, 1, 1, 1, 1,
            1, -1, -1, -1, -1, -1, -1, 1,
            1, -1, -1, -1, -1, -1, -1, 1,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]
        collision_enabled = true
        collision_layer = 1
        collision_mask = 4294967295
    [/TileMap2D]
[/level]
```
