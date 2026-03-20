# Post Processing

Post-processing is configured **per camera** using `post_processing`. This is an **ordered chain**:
effects are applied in sequence (stacked) and run after 3D + particles + 2D.

If multiple cameras are active, the post chain used is the active 3D camera if present, otherwise
the active 2D camera.

## Built-In Effects

- `blur` (`strength`)  
  3x3 blur; higher strength increases sample offset.
- `pixelate` (`size`)  
  Pixel size in screen pixels.
- `warp` (`waves`, `strength`)  
  Horizontal sine‑wave distortion.
- `vignette` (`strength`, `radius`, `softness`)  
  Darkens edges; `radius` is the start of falloff.
- `crt` (`scanlines`, `curvature`, `chromatic`, `vignette`)  
  Scanlines, mild screen curvature, chromatic offset, and a vignette.
- `color_filter` (`color`, `strength`)  
  Multiplies the scene by `color`, mixed by `strength`.
- `reverse_filter` (`color`, `strength`, `softness`)  
  Keeps colors close to `color` while others wash toward grayscale.
- `bloom` (`strength`, `threshold`, `radius`)  
  Bright‑only blur added back into the image.
- `saturate` (`amount`)  
  0 = grayscale, 1 = original, >1 boosts saturation.
- `black_white` (`amount`)  
  0 = original, 1 = full black & white.
- `custom` (`shader`/`shader_path`, optional `params`)

## Scene Authoring

`post_processing` can be an array or an object. Arrays keep order. Objects support two forms:

- Indexed keys (`0`, `1`, `2`, or `p0`, `p1`, ...) to preserve explicit order.
- Named keys (`bloom`, `blur2`, ...) to make effects addressable by name in scripts.

Each entry is an effect object. You can also provide a `name` field inside the effect object.

```
[MainCamera]
    [Camera3D]
        active = true
post_processing = [
    { type = "blur", strength = 2.0 },
    { type = "pixelate", size = 6.0 },
    { type = "warp", waves = 8.0, strength = 3.0 },
    { type = "vignette", strength = 0.6, radius = 0.55, softness = 0.25 },
    { type = "crt", scanlines = 0.35, curvature = 0.15, chromatic = 1.0, vignette = 0.25 },
    { type = "color_filter", color = (1.0, 0.8, 0.6), strength = 0.8 },
    { type = "reverse_filter", color = (0.1, 0.8, 0.2), strength = 0.9, softness = 0.2 },
    { type = "bloom", strength = 0.7, threshold = 0.75, radius = 1.5 },
    { type = "saturate", amount = 1.2 },
    { type = "black_white", amount = 1.0 },
    {
        type = "custom",
        shader = "res://shaders/post_edge.wgsl",
        params = [0.75, 2.0]
    }
]
    [/Camera3D]
[/MainCamera]
```

Named object form (keys become effect names):

```
[MainCamera]
    [Camera3D]
        active = true
post_processing = {
    bloom = { type = "bloom", strength = 0.7, threshold = 0.75, radius = 1.5 },
    blur2 = { type = "blur", strength = 2.0 },
    vignette = { type = "vignette", strength = 0.6, radius = 0.55, softness = 0.25 }
}
    [/Camera3D]
[/MainCamera]
```

## Programmatic (Scripts)

`post_processing` is a `PostProcessSet` that stores an ordered list of effects with optional
names. You can add, remove, rename, and query by name. Rendering still uses the underlying slice.

Borrowed (static slice):

```rust

static FX: &[PostProcessEffect] = &[
    PostProcessEffect::Blur { strength: 2.0 },
    PostProcessEffect::Pixelate { size: 5.0 },
    PostProcessEffect::Vignette {
        strength: 0.6,
        radius: 0.55,
        softness: 0.25,
    },
    PostProcessEffect::ColorFilter {
        color: [1.0, 0.8, 0.6],
        strength: 0.8,
    },
    PostProcessEffect::BlackWhite { amount: 1.0 },
];

with_node_mut!(ctx, Camera3D, cam_id, |cam| {
    cam.post_processing = PostProcessSet::from_effects(FX.to_vec());
});
```

Owned:

```rust

with_node_mut!(ctx, Camera3D, cam_id, |cam| {
    cam.post_processing.add(
        "warp",
        PostProcessEffect::Warp { waves: 6.0, strength: 2.0 },
    );
});
```

Get or mutate by name (Camera3D or Camera2D):

```rust
with_node_mut!(ctx, Camera3D, cam_id, |cam| {
    if let Some(PostProcessEffect::Bloom { strength, .. }) =
        cam.post_processing.get_mut("bloom")
    {
        *strength = 2.0;
    }
});
```

Read-only access with `with_node!`:

```rust
let bloom_strength = with_node!(ctx, Camera3D, cam_id, |cam| {
    cam.post_processing
        .get("bloom")
        .and_then(|fx| match fx {
            PostProcessEffect::Bloom { strength, .. } => Some(*strength),
            _ => None,
        })
});
```

Enumerate names:

```rust
with_node!(ctx, Camera3D, cam_id, |cam| {
    for name in cam.post_processing.names() {
        if let Some(name) = name {
            log::info!("post fx: {name}");
        }
    }
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
- `post` uniform (resolution, inv_resolution, near, far, projection_mode, time)
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
