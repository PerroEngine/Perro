# Post Processing

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Post Processing` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# Post Processing

Post-processing can be configured as:

- **Per camera** using `post_processing` on `Camera2D`/`Camera3D`.
- **Global** using `ResourceWindow` post-processing methods/macros.

Each chain is ordered: effects are applied in sequence (stacked) and run after 3D + particles + 2D.

If multiple cameras are active, the post chain used is the active 3D camera if present, otherwise
the active 2D camera.

Visual accessibility settings are separate from post-processing and run as a global final pass after
camera + global post-processing. See [Visual Accessibility](../scripting/contexts/resource_modules/visual_accessibility.md).

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
- `color_grade`  
  Manual grade controls: `exposure`, `contrast`, `brightness`, `saturation`, `gamma`,
  `temperature`, `tint`, `hue_shift`, `vibrance`, `lift`, `gain`, and `offset`.
- `lut2d` (`texture`, optional `lut_size`, `strength`)  
  Uses a flattened 2D LUT texture, usually `N*N` wide by `N` high.
- `lut3d` (`texture`, optional `lut_size`, `strength`)  
  Uploads a flattened LUT texture as a GPU 3D texture before sampling.
- `custom` (`shader`/`shader_path`, optional `params`)

Runtime note:
- Built-in effects do not read depth.
- `custom` effects receive depth from `depth_tex` and can use it in `post_process`.

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
        type = "color_grade",
        exposure = 0.15,
        contrast = 1.1,
        brightness = 0.02,
        saturation = 1.15,
        gamma = 1.0,
        temperature = 0.1,
        tint = -0.02,
        hue_shift = 0.0,
        vibrance = 0.25,
        lift = (0.0, 0.0, 0.0),
        gain = (1.0, 1.0, 1.0),
        offset = (0.0, 0.0, 0.0)
    },
    { type = "lut2d", texture = "res://luts/film_32.png", lut_size = 32, strength = 0.85 },
    { type = "lut3d", texture = "res://luts/print_32.png", lut_size = 32, strength = 1.0 },
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

`post_processing` is a `PostProcessSet` that stores a `Vec<PostProcessEntry>`.
Each entry ties one optional name to one effect. You can add, remove, rename, and query by name.

Build from owned effects:

```rust

let fx = vec![
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

with_node_mut!(ctx.run, Camera3D, cam_id, |cam| {
    cam.post_processing = PostProcessSet::from_effects(fx);
});
```

Add a named effect:

```rust

with_node_mut!(ctx.run, Camera3D, cam_id, |cam| {
    cam.post_processing.add(
        "warp",
        PostProcessEffect::Warp { waves: 6.0, strength: 2.0 },
    );
});
```

Get or mutate by name (Camera3D or Camera2D):

```rust
with_node_mut!(ctx.run, Camera3D, cam_id, |cam| {
    if let Some(PostProcessEffect::Bloom { strength, .. }) =
        cam.post_processing.get_mut("bloom")
    {
        *strength = 2.0;
    }
});
```

Read-only access with `with_node!`:

```rust
let bloom_strength = with_node!(ctx.run, Camera3D, cam_id, |cam| {
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
with_node!(ctx.run, Camera3D, cam_id, |cam| {
    for name in cam.post_processing.names() {
        if let Some(name) = name {
            log::info!("post fx: {name}");
        }
    }
});
```

Read entries:

```rust
with_node!(ctx.run, Camera3D, cam_id, |cam| {
    for entry in cam.post_processing.entries() {
        let name = entry.name.as_deref().unwrap_or("<unnamed>");
        log::info!("post fx: {name}");
    }
});
```

## Color Grade

`color_grade` is the manual color-grading pass. Defaults are neutral:

- `exposure = 0.0`: EV stops. `1.0` doubles light, `-1.0` halves it.
- `contrast = 1.0`: contrast around mid gray.
- `brightness = 0.0`: additive brightness.
- `saturation = 1.0`: `0.0` grayscale, `1.0` original, higher boosts.
- `gamma = 1.0`: output gamma curve.
- `temperature = 0.0`: positive warms, negative cools.
- `tint = 0.0`: positive magenta/green shift.
- `hue_shift = 0.0`: full hue rotation units; `1.0` wraps once.
- `vibrance = 0.0`: saturation boost biased toward low-chroma colors.
- `lift = (0.0, 0.0, 0.0)`: shadow offset per channel.
- `gain = (1.0, 1.0, 1.0)`: highlight gain per channel.
- `offset = (0.0, 0.0, 0.0)`: final additive channel offset.

Scene example:

```toml
post_processing = [
    {
        type = "color_grade",
        exposure = 0.25,
        contrast = 1.12,
        saturation = 1.08,
        temperature = 0.08,
        vibrance = 0.2,
        lift = (-0.01, -0.01, 0.0),
        gain = (1.04, 1.02, 0.98)
    }
]
```

Script example:

```rust
cam.post_processing.add(
    "grade",
    PostProcessEffect::ColorGrade {
        exposure: 0.25,
        contrast: 1.12,
        brightness: 0.0,
        saturation: 1.08,
        gamma: 1.0,
        temperature: 0.08,
        tint: 0.0,
        hue_shift: 0.0,
        vibrance: 0.2,
        lift: [-0.01, -0.01, 0.0],
        gain: [1.04, 1.02, 0.98],
        offset: [0.0, 0.0, 0.0],
    },
);
```

## LUTs

LUT textures use normal texture asset paths. In static builds, `res://` LUT images are packed
through the same static texture pipeline as sprites and materials.

`lut2d` expects a flattened 2D LUT image:

- horizontal layout: width = `N * N`, height = `N`
- red = x within one tile
- green = y
- blue = tile index across x

`lut3d` accepts the same horizontal layout and uploads it as a GPU 3D texture. It also accepts a
vertical layout: width = `N`, height = `N * N`.

If `lut_size` is omitted or `0`, the renderer infers it from image dimensions when possible. Use
`lut_size` when dimensions are ambiguous.

Scene examples:

```toml
post_processing = [
    { type = "lut2d", texture = "res://luts/film_32.png", lut_size = 32, strength = 0.75 },
    { type = "lut3d", texture = "res://luts/print_32.png", lut_size = 32, strength = 1.0 }
]
```

Script examples:

```rust
let film_lut = ResPath::new("res://luts/film_32.png");
let print_lut = ResPath::new("res://luts/print_32.png");

cam.post_processing.add(
    "film_lut",
    PostProcessEffect::Lut2D {
        texture_path: film_lut.as_str().into(),
        size: 32,
        strength: 0.75,
    },
);

cam.post_processing.add(
    "print_lut",
    PostProcessEffect::Lut3D {
        texture_path: print_lut.as_str().into(),
        size: 32,
        strength: 1.0,
    },
);
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
- `lut_2d_tex` / `lut_3d_tex` (available for custom shaders, normally unused)
- `post` uniform (resolution, inv_resolution, near, far, projection_mode, time)
- `custom_params` (optional `vec4<f32>` array from `params`)

Use `type = "custom"` with `shader = "res://path/to/shader.wgsl"` in scenes, or
`PostProcessEffect::Custom { shader_path, params }` in code. `params` is a `Vec<CustomPostParam>`.

Performance notes:
- Post uniforms are written with per-pass dynamic offsets (aligned uniform slots), so each pass
  reads its own params without uniform overwrite hazards.
- LUT textures are cached by path and size after first use.

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
