# `.panim` Format

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`.panim` is a keyframe animation clip authored as text. It solves the problem of moving nodes, bones, and cameras over time without hardcoding transforms in `on_update`: you write "at frame N this object has these field values" and the runtime interpolates between them. Clips also carry frame-timed gameplay hooks (`emit_signal`, `call_method`, `set_var`), so a hit frame or a footstep fires exactly when the pose reaches it.

## Use Cases

- Looping character locomotion: keyframe `@Hero { position, rotation, scale }` across `[FrameN]` blocks and play the clip through an `AnimationPlayer` with `playback = loop`.
- Skeletal attack swing: drive `Skeleton3D` with rest-relative bone tracks like `bones["Spine"].rotation` and `bone[3].rotation_deg`.
- Cutscene camera move: animate a `Camera3D` `position` plus `perspective_fovy_degrees` with `default_ease = "ease_in_out"` for a smooth dolly.
- Frame-timed combat and VFX hooks: fire `emit_signal = { name = "hit", params = [1] }` on the contact frame and `call_method = { name = "spawn_trail" }` on a follow-up frame.
- Blend-friendly additive layers: mark aim/recoil poses with open keyframes `[FrameN?]` so the segment interpolates from the live runtime value instead of snapping to an authored one.
- Modded or downloaded clips: build an `AnimationID` at runtime from raw bytes with `animation_create_from_bytes!`.

## Example

Author `res://animations/door_open.panim`:

```ini
[Animation]
name = "DoorOpen"
fps = 30
default_ease = "ease_out"
[/Animation]

[Objects]
Door = Node3D
[/Objects]

[Frame0]
@Door {
    rotation_deg = (0, 0, 0)
}
[/Frame0]

[Frame20]
@Door {
    rotation_deg = (0, 95, 0)
}
emit_signal = { name = "door_opened", params = [] }
[/Frame20]
```

Bind it to a scene node through an `AnimationPlayer`:

```ini
[Door]
    [Node3D/]
[/Door]

[DoorAnim]
    [AnimationPlayer]
        animation = "res://animations/door_open.panim"
        bindings = { Door = @Door }
        playback = once
    [/AnimationPlayer]
[/DoorAnim]
```

Load and play it from a script:

```rust
let clip = animation_load!(res, "res://animations/door_open.panim");
let _ = anim_player_set_clip!(ctx, player, clip);
let _ = anim_player_bind!(ctx, player, "Door", door_node);
let _ = anim_player_play!(ctx, player);
```

## Reference

# `.panim` Format

`*.panim` is a Perro animation clip resource.

It is keyframe-based and authored with scene-style value syntax (`vec`, `object`, arrays, bools, numbers, strings).

## File Structure

```ini
[Animation]
name = "RunForward"
fps = 60
default_interp = "interpolate"
default_ease = "linear"
[/Animation]

[Objects]
Hero = Node3D
MainCam = Camera3D
[/Objects]

[Frame0]
@Hero {
    position = (0,0,0)
    rotation = (0,0,0,1)
    scale = (1,1,1)
}
@MainCam {
    position = (0,2,-1)
}
[/Frame0]

[Frame10]
@Hero {
    position = (3,0,0)
}
@MainCam {
    position = (3,0,2)
}
emit_signal = { name="step", params=[0] }
[/Frame10]
```

## Blocks

- `[Animation] ... [/Animation]`
- `[Objects] ... [/Objects]`
- `[FrameN] ... [/FrameN]` where `N` is a frame index (`u32`)
- `[FrameN?] ... [/FrameN]` where `N` is a frame index (`u32`) and `?` marks an **open frame**

`total_frames` is derived from the largest frame index: `max_frame + 1`.

## `[Animation]` Keys

- `name` (text, default `"Animation"`)
- `fps` (positive float, default `60`)
- `default_interp` or `default_interpolation` (default `"interpolate"`)
- `default_ease` or `default_easing` (default `"linear"`)

Interpolation values:

- `step`
- `interpolate`
- `linear`
- `lerp`
- `slerp`

Ease values:

- `linear`
- `ease_in`, `easein`, `in`
- `ease_out`, `easeout`, `out`
- `ease_in_out`, `easeinout`, `in_out`

## `[Objects]` Block

Declare animation clip objects and their node type:

```ini
[Objects]
Hero = Node3D
Weapon = MeshInstance3D
[/Objects]
```

Object names (`Hero`) are the track keys used for `AnimationPlayer` bindings.
Scene bindings map object name `Hero` to a scene node ref like `@PlayerRoot`.
Use `@Hero` only when referring to the declared object in frame blocks or event params.

## Frame Entries

Inside `[FrameN]`:

- object blocks: `@ObjectName { ... }`
- global event: `emit_signal = { ... }`

Inside object blocks:

- field keyframes (`position`, `visible`, `mesh`, ...)
- object-scoped event authoring keys (`emit_signal`, `set_var`, `call_method`)
- track controls (`field.interp`, `field.ease`)

Inside `[FrameN?]`:

- same authoring syntax as `[FrameN]`
- all keys authored in that frame are marked **Open** mode
- open mode means the key is a runtime continuity marker, not an authoritative sampled pose

### Open vs Closed Keyframes

- **Closed keyframe** (`[FrameN]`): authoritative authored value
- **Open keyframe** (`[FrameN?]`): interpolation-origin policy from runtime/current value

Open key behavior:

- open keys preserve continuity (no forced snap to authored value)
- interpolation segment starts from the runtime value at playback time
- open keys are runtime-dependent and not deterministic pose samples by themselves
- open keys may still carry interpolation/easing metadata (`.interp`, `.ease`)

Example:

```ini
[Frame0?]
@Hand {
    rotation = 0 // not authoritative if open; runtime start is used
}
[/Frame0]

[Frame20]
@Hand {
    rotation_deg = 90
}
[/Frame20]
```

If runtime rotation at frame 0 is `13deg`, playback interpolates `13deg -> 90deg` over 20 frames.

Authoring model:

- each frame is authored on declared animation objects (`@Hero`, `@Camera`, ...)
- for each object block, you write the same field names you already use in scene node authoring for that node type
- think in terms of "at frame N, this object has these field values"

## Persistent Per-Track Controls

Track controls are stateful and persist until changed:

```ini
[Frame0]
@Hero {
    position.interp = "interpolate"
    position.ease = "ease_in"
    position = (0,0,0)
}
[/Frame0]

[Frame25]
@Hero {
    position.ease = "ease_out"
    position = (5,0,0)
}
[/Frame25]

[Frame40]
@Hero {
    position.interp = "step"
    position = (10,0,0)
}
[/Frame40]
```

Semantics:

- control lines affect subsequent keys for that track
- if a control is written after a keyed value in the same frame, it does not retroactively change that earlier key
- no reset happens automatically between frames

## `interp` vs `ease`

- `interp` chooses interpolation mode:
- `step`: hold previous value until next key
- `interpolate`: blend across key interval (type-aware lerp/slerp where supported)
- `ease` shapes interpolation time:
- `linear`: constant rate
- `ease_in`: slow start
- `ease_out`: slow end
- `ease_in_out`: slow start + slow end

## Supported Animatable Fields

`Node2D`:

- `position`, `rotation`, `scale`, `visible`, `z_index`
- `rotation_deg` is accepted anywhere `rotation` is accepted.

`Node3D`:

- `position`, `rotation`, `scale`, `visible`
- `rotation_deg` is accepted anywhere `rotation` is accepted.

`Sprite2D`:

- `texture`

`MeshInstance3D`:

- `mesh`, `material`

`Camera3D`:

- `zoom`
- `perspective_fovy_degrees`
- `perspective_near`, `perspective_far`
- `orthographic_size`
- `orthographic_near`, `orthographic_far`
- `frustum_left`, `frustum_right`, `frustum_bottom`, `frustum_top`, `frustum_near`, `frustum_far`
- `active`

`Light3D`:

- `color`, `intensity`, `cast_shadows`, `shadow_strength`, `shadow_depth_bias`, `shadow_normal_bias`, `active`

`PointLight3D`:

- `range`

`SpotLight3D`:

- `range`, `inner_angle_radians`, `outer_angle_radians`

`Skeleton2D` / `Skeleton3D`:

- `bones[index].position`, `bones[index].rotation`, `bones[index].scale`
- `bone[index].position`, `bone[index].rotation`, `bone[index].scale`
- `bones["name"].position`, `bones["name"].rotation`, `bones["name"].scale`
- `bone["name"].position`, `bone["name"].rotation`, `bone["name"].scale`
- `rotation_deg` is accepted in the same bone paths, for example `bone[0].rotation_deg`.

Notes:

- Bone tracks target `pose` transforms on `Skeleton2D.bones` and `Skeleton3D.bones`.
- Bone `rotation` values are **rest-relative deltas**: playback composes
  `rest * keyed` (3D) / `rest + keyed` (2D). Identity (`(0, 0, 0, 1)` / `0`)
  keeps the rest rotation. Bone `position`/`scale` values are absolute.
- `Skeleton2D` uses `Transform2D`; `rotation` is radians.
- `Skeleton3D` uses `Transform3D`; `rotation` is quaternion or Euler vec3.
- `position/rotation/scale` share one transform track per targeted bone.
- Track controls are supported on bone channels, for example:
  `bones[0].position.interp = "step"` and `bones[0].position.ease = "ease_in"`.

## Retarget Bake

Place `walk.pretarget` beside `walk.panim` to retarget during static builds.

The generated static clip contains the target rig tracks.

The source `.panim` stays unchanged.

```ini
source = Rig
target = HeroRig
keep_unmapped = false
translation = root_only
root_bone = hips

bone hips => Hips
bone arm_l => Arm.L

source_rest arm_l = (0.2, 1.4, 0) | (0, 0, 0, 1) | (1, 1, 1)
target_rest Arm.L = (0.25, 1.5, 0) | (0, 0, 0.7071068, 0.7071068) | (1, 1, 1)
```

Rules:

- Exact names need no `bone` row when `keep_unmapped = true`.
- Alias rows use `bone source => target`.
- Rest rows use local `position | rotation quaternion | scale`.
- Scale may be omitted; `(1, 1, 1)` is used.
- `translation = all` keeps old map behavior.
- `translation = root_only` needs `root_bone`.
- `translation = none` removes all bone position channels.
- Source + target rest rows align position and scale deltas.
- Rotation keys remain rest-relative pose deltas.

## Events

Global event in frame:

```ini
emit_signal = { name="hit", params=[1, "light"] }
```

Object-scoped events:
-Target variables/methods on this runtime node

```ini
@Hero {
    set_var = { name="combo", value=2 }
    call_method = { name="spawn_trail", params=[0.2] }
}
```

Event notes:

- params/value support direct object references:
- `@Object` resolves to that object's bound runtime `NodeID`.
- `@Object.field` resolves to the current frame value of that field on the bound runtime node.
- reference params are supported in `emit_signal.params`, `call_method.params`, and `set_var.value`.

Example:

```ini
[Frame20]
@Hero {
    call_method = { name="aim_at", params=[@Target, @Target.position] }
    set_var = { name="tracked_target", value=@Target }
}
[/Frame20]
```

## Variables

Top-level variables are supported:

```ini
@mesh_a = "res://meshes/hero.glb:mesh[0]"

[Frame0]
@HeroMesh {
    mesh = @mesh_a
}
[/Frame0]
```

## Runtime Notes

- `.panim` is loaded into an `AnimationClip`.
- Numeric/vector/transform tracks interpolate with easing when `interp = interpolate`.
- bool/asset-like values behave as step values.
- open keys are treated as runtime-originated continuity points:
  - `AnimationObjectKey.mode = Open` marks the key
  - open keys are not directly deterministic sampled values (`sampled_value()` returns `None`)
  - deterministic optimization/simplification should only run on fully closed tracks.
