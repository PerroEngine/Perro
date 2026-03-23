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
@Hero = Node3D
@MainCam = Camera3D
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
@Hero = Node3D
@Weapon = MeshInstance3D
[/Objects]
```

Object names (`@Hero`) are the track keys used for `AnimationPlayer` bindings.

## Frame Entries

Inside `[FrameN]`:

- object blocks: `@ObjectName { ... }`
- global event: `emit_signal = { ... }`

Inside object blocks:

- field keyframes (`position`, `visible`, `mesh`, ...)
- object-scoped event authoring keys (`emit_signal`, `set_var`, `call_method`)
- track controls (`field.interp`, `field.ease`)

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

`Node3D`:

- `position`, `rotation`, `scale`, `visible`

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

- `color`, `intensity`, `active`

`PointLight3D`:

- `range`

`SpotLight3D`:

- `range`, `inner_angle_radians`, `outer_angle_radians`

`Skeleton3D`:

- `bones[index].position`, `bones[index].rotation`, `bones[index].scale`
- `bone[index].position`, `bone[index].rotation`, `bone[index].scale`
- `bones["name"].position`, `bones["name"].rotation`, `bones["name"].scale`
- `bone["name"].position`, `bone["name"].rotation`, `bone["name"].scale`

Notes:

- Bone tracks target `rest` transforms on `Skeleton3D.bones`.
- `position/rotation/scale` share one transform track per targeted bone.
- Track controls are supported on bone channels, for example:
  `bones[0].position.interp = "step"` and `bones[0].position.ease = "ease_in"`.

## Events

Global event in frame:

```ini
emit_signal = { name="hit", params=[1, "light"] }
```

Object-scoped events:

```ini
@Hero {
    set_var = { name="combo", value=2 }
    call_method = { name="spawn_trail", params=[0.2] }
}
```

Event notes:

- `emit_signal` should be authored as a frame/global event.
- object-scoped `emit_signal` does not provide object-targeted runtime behavior today.
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
