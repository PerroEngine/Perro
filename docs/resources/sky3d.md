# Sky3D

## Page Map

| Header          | Link                                |
| --------------- | ----------------------------------- |
| Purpose         | [Purpose](#purpose)                 |
| Scene Fields    | [Scene Fields](#scene-fields)       |
| Color Model     | [Color Model](#color-model)         |
| Custom Shaders  | [Custom Shaders](#custom-shaders)   |
| Time In Shaders | [Time In Shaders](#time-in-shaders) |
| Example         | [Example](#example)                 |
| Shader Input    | [Shader Input](#shader-input)       |

## Purpose

`Sky3D` draws the 3D sky.

It is a camera-relative skybox/dome.

## Scene Fields

```txt
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
    horizon_colors = [
        (0.62, 0.64, 0.66),
        (0.42, 0.43, 0.45),
        (0.28, 0.29, 0.31)
    ]
    time = { time_of_day = 0.25 paused = false scale = 1.0 }
    shaders = [
        { path = "res://shaders/sky.wgsl", params = [0.5, (1.0, 0.8, 0.6)] }
    ]
    active = true
[/Sky3D]
```

| Field              | Type        | Meaning                           |
| ------------------ | ----------- | --------------------------------- |
| `day_colors`       | color array | Top sky gradient for day.         |
| `evening_colors`   | color array | Top sky gradient for dawn/dusk.   |
| `night_colors`     | color array | Top sky gradient for night.       |
| `horizon_colors`   | color array | Gray/fog band under horizon.      |
| `time.time_of_day` | `f32`       | Normalized day clock. `0.0..1.0`. |
| `time.paused`      | `bool`      | Stop automatic day clock.         |
| `time.scale`       | `f32`       | Day clock speed.                  |
| `shaders`          | array       | Ordered custom sky shader passes. |
| `active`           | `bool`      | Use this sky when visible.        |
| `render_layers`    | bit mask    | Camera layer filter.              |

## Color Model

`Sky3D` starts with a base color.

Base color blends between:

- `day_colors`
- `evening_colors`
- `night_colors`

Blend weights come from `time.time_of_day`.

The top half uses the sky gradients.

The horizon/down half fades to `horizon_colors`.

The base color becomes `in.color` for the first custom shader.

Each custom shader receives the color from the previous pass.

## Custom Shaders

Each custom shader file uses `sky_shader`.

```wgsl
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    return in.color;
}
```

No `@vertex`.

No `@fragment`.

No bind groups.

The engine wraps the shader and runs passes in `shaders` order.

Use shader params like post-process params:

```txt
shaders = [
    {
        path = "res://shaders/sky_horizon_band.wgsl"
        params = [0.12, 0.32, (0.86, 0.88, 0.94)]
    }
]
```

Read params with `custom_param(in, index)`.

`custom_f_param(in, index)` also works.

Packing:

- `f32` -> `.x`
- `i32` -> `.x`
- `bool` -> `.x` as `0.0` or `1.0`
- `vec2` -> `.xy`
- `vec3` -> `.xyz`
- `vec4` -> `.xyzw`

## Time In Shaders

Use `in.time_of_day` for authored day phase.

It is normalized.

- `0.0` = start of day cycle
- `0.25` = noon-ish default
- `0.5` = opposite side
- `0.75` = dusk-ish
- wraps at `1.0`

Use weights for simpler color logic:

- `in.day_weight`
- `in.evening_weight`
- `in.night_weight`

Use `in.time_seconds` for animation.

It is a running sky clock.

Use it for drift, pulse, twinkle, rotation, or noise offsets.

Example:

```wgsl
let drift = vec2<f32>(in.time_seconds * 0.02, 0.0);
```

## Example

```wgsl
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    let width = max(custom_param(in, 0u).x, 0.001);
    let strength = clamp(custom_param(in, 1u).x, 0.0, 1.0);
    let tint = custom_param(in, 2u).rgb;
    let band = (1.0 - smoothstep(0.0, width, abs(in.ray.y))) * strength;
    let pulse = 0.5 + 0.5 * sin(in.time_seconds * 0.8);
    let color = mix(in.color.rgb, tint, band * pulse);
    return vec4<f32>(color, in.color.a);
}
```

Scene:

```txt
shaders = [
    { path = "res://shaders/sky_band.wgsl", params = [0.12, 0.35, (0.85, 0.88, 0.95)] }
]
```

## Shader Input

```wgsl
struct SkyFragment {
    ray: vec3<f32>,
    uv: vec2<f32>,
    time_of_day: f32,
    time_seconds: f32,
    day_weight: f32,
    evening_weight: f32,
    night_weight: f32,
    horizon_weight: f32,
    color: vec4<f32>,
};
```

| Field            | Meaning                                                                 |
| ---------------- | ----------------------------------------------------------------------- |
| `ray`            | Normalized direction from camera through sky. Use this for dome coords. |
| `uv`             | Fullscreen uv. Use this for screen-space sky effects.                   |
| `time_of_day`    | Normalized day clock.                                                   |
| `time_seconds`   | Running sky animation clock.                                            |
| `day_weight`     | Day blend weight from current time.                                     |
| `evening_weight` | Evening blend weight from current time.                                 |
| `night_weight`   | Night blend weight from current time.                                   |
| `horizon_weight` | `0.0` above horizon, `1.0` under horizon fade.                          |
| `color`          | Current sky color from base sky or prior pass.                          |

Param helpers:

```wgsl
let first = custom_param(in, 0u);
let same = custom_f_param(in, 0u);
```
