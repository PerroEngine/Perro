# Post Processing

Post-processing is configured **per camera** using `post_processing`. This is an **ordered chain**:
effects are applied in sequence (stacked) and run after 3D + particles + 2D.

If multiple cameras are active, the post chain used is the active 3D camera if present, otherwise
the active 2D camera.

## Built-In Effects

- `blur` (`strength`)
- `pixelate` (`size`)
- `warp` (`waves`, `strength`)
- `custom` (`shader`/`shader_path`, optional `params`)

## Scene Authoring

`post_processing` can be an array or an object keyed by indices (`0`, `1`, `2`, or
`p0`, `p1`, ...). Each entry is an effect object.

```
[MainCamera]
    [Camera3D]
        active = true
post_processing = [
    { type = "blur", strength = 2.0 }
    { type = "pixelate", size = 6.0 }
    { type = "warp", waves = 8.0, strength = 3.0 }
    {
        type = "custom",
        shader = "res://shaders/post_edge.wgsl",
        params = [0.75, 2.0]
    }
]
    [/Camera3D]
[/MainCamera]
```

## Programmatic (Scripts)

`post_processing` is `Cow<'static, [PostProcessEffect]>`, so you can use borrowed static slices or
owned vectors.

Borrowed (static slice):

```rust

static FX: &[PostProcessEffect] = &[
    PostProcessEffect::Blur { strength: 2.0 },
    PostProcessEffect::Pixelate { size: 5.0 },
];

with_node_mut!(ctx, Camera3D, cam_id, |cam| {
    cam.post_processing = Cow::Borrowed(FX);
});
```

Owned:

```rust

with_node_mut!(ctx, Camera3D, cam_id, |cam| {
    cam.post_processing = Cow::Owned(vec![
        PostProcessEffect::Warp { waves: 6.0, strength: 2.0 },
    ]);
});
```

## Custom Post Shaders (`.wgsl`)

Custom post effects mirror the custom material workflow and are defined by a `.wgsl` file. Your
shader must implement:

```
fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32>
```

The engine provides a prelude with:

- `input_tex` + `input_sampler` (color)
- `depth_tex` (depth, if your shader uses it)
- `post` uniform (resolution, inv_resolution, near, far, projection_mode)
- `custom_params` (optional `vec4<f32>` array from `params`)

Use `type = "custom"` with `shader = "res://path/to/shader.wgsl"` in scenes, or
`PostProcessEffect::Custom { shader_path, params }` in code.

### Example `post_process`

This example does a simple edge highlight using color differences and a user-controlled strength
(`params[0].x`).

```wgsl
fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    let strength = max(custom_params[0].x, 0.0);
    let off = post.inv_resolution;
    let c0 = textureSample(input_tex, input_sampler, uv + vec2<f32>(off.x, 0.0)).rgb;
    let c1 = textureSample(input_tex, input_sampler, uv + vec2<f32>(-off.x, 0.0)).rgb;
    let c2 = textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, off.y)).rgb;
    let c3 = textureSample(input_tex, input_sampler, uv + vec2<f32>(0.0, -off.y)).rgb;
    let edge = length(c0 - c1) + length(c2 - c3);
    return vec4<f32>(color.rgb + edge * strength, color.a);
}
```
