# Shaders (WGSL)

## Page Map

| Header             | Link                                                   |
| ------------------ | ------------------------------------------------------ |
| Purpose            | [Purpose](#purpose)                                    |
| Use Cases          | [Use Cases](#use-cases)                                |
| Custom 3D Material | [Custom 3D Material Shaders](#custom-3d-material-shaders) |
| Custom Sky3D       | [Custom Sky3D Shaders](#custom-sky3d-shaders)          |
| Checking Shaders   | [Checking Shaders](#checking-shaders)                  |
| Limits             | [Current Limitations](#current-limitations)            |
| Reference          | [Reference](#reference)                                |

## Purpose

Perro uses WGSL (`.wgsl`) for GPU shaders. Custom materials and `Sky3D` passes reference a shader by path; you implement one entry function and the engine injects the scene structs, lighting, vertex wiring, and bind groups around it. Reach for a shader when you need surface or sky effects the built-in presets cannot express, such as dissolves, force fields, animated water, or procedural skies.

## Use Cases

- Animated surfaces: a custom material `shade_material(in)` driven by `perro_time()` / `perro_time_phase()` for pulsing, scrolling, or shimmering looks.
- Force fields, portals, and dissolves: sample `.pmat` `images` with `custom_image_sample(in, index, uv)` and read tunables with `custom_f_param(in, index)`.
- Custom-lit props: return base color and let standard lighting wrap it, or call `perro_standard(...)` to supply your own roughness, metallic, ao, and emissive.
- Raw glows and holograms: set `lighting = "raw"` to bypass standard lighting and return exact color.
- Vertex deformation: a `shade_vertex(out)` hook to bend, wave, or wobble geometry in the vertex stage.
- Procedural skies: a `sky_shader(in)` pass adding clouds, stars, sun, or horizon bands over the built-in day/evening/night gradient.

## Checking Shaders

A `.wgsl` file is a fragment: it only becomes a full module once the engine
wraps a prelude and entry points around it, so a typo in it normally surfaces
on the first frame that draws with it. `perro doctor` composes the same module
offline and reports errors at your file's own line and column:

```powershell
perro doctor --path <project_dir>
```

```txt
err: shader res://shaders/portal.wgsl:12:18: [rigid mesh] no definition in scope for identifier: `custom_image_smaple`
```

Notes:

- The entry function picks the prelude: `shade_material`/`shade_vertex` for a
  custom 3D material, `sky_shader` for a `Sky3D` pass, `post_process` for a
  post-process pass.
- Custom materials are checked against the rigid and the skinned prelude, since
  either mesh kind can use the material. Multimesh exposes a subset of the same
  helpers and needs no separate pass.
- A `.wgsl` with none of those functions is not loadable; `doctor` warns.
- `perro build --static` runs the same check and fails the build on a bad
  shader.

## Choice Guide

Start with built-in materials, sky fields, and post effects. Add WGSL when the
required vertex or pixel behavior cannot be expressed there. Standard-lit
custom materials keep Perro lighting; `lighting = "raw"` trades that integration
for exact shader output. Keep gameplay rules outside shaders.

## Reference

# Shaders (WGSL)

Perro uses **WGSL** (`.wgsl`) for GPU shaders. Shaders are referenced by custom materials via a `shader_path`.

## Custom 3D Material Shaders

Custom 3D materials are declared as:

```txt
type = "custom"
shader_path = "res://shaders/custom.wgsl"
# output = "surface" (default) lets Perro add standard lighting
# output = "final" uses exact shader output
params = {
    glow = 1.25
    tint = (1.0, 0.2, 0.4, 1.0)
}
images = {
    mask = "res://textures/mask.png"
    noise = "res://textures/noise.png"
}
```

### Static Texture Baking

Static builds can run a custom shader once and embed its result as a PTEX texture.
Mark the material with `release_bake = true` and add this WGSL function:

```wgsl
fn bake_texture(in: BakeInput) -> vec4<f32> {
    let glow = bake_param(in, 0u).x;
    let grid = step(0.98, fract(in.uv.x * 24.0))
        + step(0.98, fract(in.uv.y * 24.0));
    return vec4<f32>(vec3<f32>(0.02, 0.08, 0.14) + grid * glow, 1.0);
}
```

`BakeInput` fields:

- `uv`: normalized texture coordinate.
- `pixel`: pixel coordinate.
- `resolution`: output size in pixels.
- `bake_param(in, index)`: material `params` entry packed as `vec4<f32>`.

The build renders a full-screen triangle into an sRGB RGBA texture, reads it back,
encodes PTEX, and generates a small material that samples it. The original material
path resolves to the baked variant when `release_bake = true`; generated runtime and baked
aliases support per-mesh and per-surface selection.

Current bake limits:

- Pure WGSL plus material params only; custom `images` are rejected.
- `shade_vertex` output cannot be represented by a texture and is rejected.
- Runtime-only inputs such as time, camera, world position, lights, and scene state
  should not drive `bake_texture`; bake from `uv`, `pixel`, `resolution`, and params.
- Baking requires a Windows, Linux, or macOS build host with a WebGPU adapter.

```rust
let custom_id = material_create!(
    res,
    Material3D::Custom(CustomMaterial3D::with_params(
        "res://shaders/custom.wgsl",
        vec![
            CustomMaterialParam3D::named("glow", CustomMaterialParamValue3D::F32(1.25)),
            CustomMaterialParam3D::named(
                "tint",
                CustomMaterialParamValue3D::Vec4([1.0, 0.2, 0.4, 1.0]),
            ),
        ],
    ))
);
```

### How Custom Shaders Are Composed

Custom material shaders are composed at runtime:

1. The engine injects a **shared prelude** (scene/lighting structs, vertex wiring, helpers).
2. Your WGSL file is appended.
3. The engine appends tiny wrapper entry points:

```wgsl
@vertex
fn vs_main(v: VertexInput, inst: InstanceInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_index: u32) -> VertexOutput {
    return perro_vs_main_base(v, inst, vertex_index, instance_index); // or shade_vertex(...) if you define it
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    return shade_material(in);
}
```

### What You Need To Implement

Your `.wgsl` must define:

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    // use packed material fields and custom_f_param(...)
}
```

Optional vertex hook in same file:

```wgsl
fn shade_vertex(out: VertexOutput) -> VertexOutput {
    let wobble = custom_v_param(out, 0u).x;
    // modify out.clip_pos / out.world_pos / out.normal_ws / out.uv
    return out;
}
```

Material `vertex_modifiers` run before `shade_vertex`.
The hook receives the built-in stack result.

You **do not** need to define `vs_main`, `fs_main`, bind groups, or scene structs.

Notes for custom shaders:

- Custom shaders use standard lighting by default. The engine treats `shade_material(in)` as base color, then applies standard lighting.
- Add `output = "final"` when `shade_material` returns final shaded color.
- Legacy `lighting = "raw"` remains an alias for `output = "final"`.
- `perro_standard(in, base_color, roughness, metallic, ao, emissive)` applies the built-in standard material path.
- `perro_toon(...)`, `perro_unlit(...)`, `perro_hand_drawn(...)`, and `perro_pixel_surface(...)` match their material presets.
- `perro_material_alpha(in, alpha)` applies alpha cutoff, opaque alpha, and mesh blend alpha.
- Calling a canonical shade helper prevents the default standard wrapper.
- Legacy `perro_lit_standard` and `perro_lit_toon` aliases remain valid.
- If a scene has no `Sky3D`, no `AmbientLight3D`, and no 3D lights, standard materials render black except for `emissive_factor`.
- Use `custom_f_param(in, index)` to read custom params in fragment stage.
- Use `custom_v_param(out, index)` inside `shade_vertex` for same params in vertex stage.
- Use `custom_image_sample(in, index, uv)` to sample custom `images` from `.pmat`.
- Legacy aliases `custom_param` and `custom_param_vertex` stay valid.

### Vertex Modifier Helpers

Custom vertex hooks may call the same built-in operations directly:

```wgsl
out = perro_vertex_wind(out, direction, strength, speed, frequency, mask);
out = perro_vertex_wave(out, axis, direction, amplitude, speed, frequency, phase, mask);
out = perro_vertex_bend(out, along_axis, bend_axis, angle, start, end);
out = perro_vertex_twist(out, axis, angle, start, end);
out = perro_vertex_inflate(out, amount, mask);
out = perro_vertex_jitter(out, amount, scale, rate, seed, mask);
out = perro_vertex_pixel_snap(out, virtual_height, strength);
```

`mask` ranges from `0.0` to `1.0`.
Use `perro_vertex_axis_mask(out.world_pos, axis, start, end)` to build it;
axis codes are `0.0` for X, `1.0` for Y, and `2.0` for Z.
Angles use radians.
Each helper updates dependent clip position, and rotation helpers update normals.
Direct helper calls live only in the color pass; use material `vertex_modifiers`
when depth and shadow parity matters.

### Stylized Helpers

The 3D prelude includes helpers for toon, hand-drawn, and pixel-art materials.
They work on rigid, skinned, and dense multimesh custom materials.

```wgsl
fn perro_toon(
    in: FragmentInput,
    base_color: vec4<f32>,
    band_count: f32,
    rim_strength: f32,
    rim_width: f32,
    emissive: vec3<f32>,
) -> vec4<f32>

fn perro_unlit(
    in: FragmentInput,
    base_color: vec4<f32>,
    emissive: vec3<f32>,
) -> vec4<f32>

fn perro_hand_drawn(
    in: FragmentInput,
    base_color: vec4<f32>,
    band_count: f32,
    hatch_scale: f32,
    grain_strength: f32,
    emissive: vec3<f32>,
) -> vec4<f32>

fn perro_pixel_surface(
    in: FragmentInput,
    base_color: vec4<f32>,
    color_levels: f32,
    dither_strength: f32,
    emissive: vec3<f32>,
) -> vec4<f32>

fn perro_posterize(color: vec3<f32>, level_count: f32) -> vec3<f32>
fn perro_pixel_uv(uv: vec2<f32>, pixel_count: vec2<f32>) -> vec2<f32>
fn perro_bayer_dither(color: vec3<f32>, frag_coord: vec2<f32>, strength: f32) -> vec3<f32>
fn perro_palette_snap(
    in: FragmentInput,
    color: vec3<f32>,
    image_index: u32,
    color_count: u32,
) -> vec3<f32>
fn perro_hatch(
    coords: vec2<f32>,
    shade: f32,
    scale: f32,
    angle: f32,
    line_width: f32,
) -> f32
fn perro_crosshatch(
    coords: vec2<f32>,
    shade: f32,
    scale: f32,
    angle: f32,
    line_width: f32,
) -> f32
fn perro_paper_grain(coords: vec2<f32>, scale: f32, amount: f32) -> f32
fn perro_distance_lod(
    world_pos: vec3<f32>,
    near_distance: f32,
    far_distance: f32,
    level_count: u32,
) -> u32
```

`perro_toon` applies scene light, material alpha, decals, mesh fade, toon bands, and rim light.
Rigid and skinned draws also apply supported light shadows.
Set `output = "final"` for explicit final-shader output.

Example hand-drawn material body:

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let pixel_uv = perro_pixel_uv(in.uv, vec2<f32>(32.0));
    var color = custom_image_sample(in, 0u, pixel_uv).rgb;

    let lod = perro_distance_lod(in.world_pos, 5.0, 50.0, 4u);
    let grain = perro_paper_grain(in.uv, 128.0 / f32(lod + 1u), 0.08);
    let ink = perro_crosshatch(in.uv, 0.65, 24.0, 0.785398, 0.08);
    color = mix(color + vec3<f32>(grain), vec3<f32>(0.05), ink);
    color = perro_bayer_dither(color, in.frag_pos.xy, 0.08);
    color = perro_posterize(color, 5.0);

    return perro_hand_drawn(
        in,
        vec4<f32>(color, 1.0),
        4.0,
        24.0,
        0.08,
        vec3<f32>(0.0),
    );
}
```

Helper notes:

- `perro_pixel_uv` snaps texture detail inside a mesh. It does not pixelate the mesh silhouette.
- use camera `pixelate` post-processing or a low-resolution render target for whole-frame pixels.
- `perro_palette_snap` reads a horizontal palette from a custom image and supports up to 64 colors.
- use UV or world-space coordinates for hatch and grain to avoid screen-space swimming.
- `perro_distance_lod` returns `0` near the camera and `level_count - 1` at the far distance.
- `rim_width` controls rim falloff. It does not draw a geometry outline.

## Custom Sky3D Shaders

See also: [`Sky3D`](sky3d.md) for full sky authoring docs.

`Sky3D` shaders are ordered passes:

```txt
shaders = [
    { path = "res://shaders/sky.wgsl", params = [0.5, (1.0, 0.8, 0.6)] }
]
```

Each WGSL file defines one function:

```wgsl
fn sky_shader(in: SkyFragment) -> vec4<f32> {
    return in.color;
}
```

`SkyFragment` fields:

- `ray`: normalized camera ray through skybox point.
- `uv`: fullscreen sky uv.
- `time_of_day`, `time_seconds`.
- `day_weight`, `evening_weight`, `night_weight`.
- `horizon_weight`.
- `color`: current stack color.
- `custom_param(in, index)`: custom pass params packed as `vec4<f32>`.
- `custom_f_param(in, index)`: same alias as material fragment params.

Passes run in array order. Built-in Sky3D only provides day/evening/night gradients and horizon color fade; clouds, stars, sun, and moon come from custom sky shaders if needed.

### FragmentInput Fields

`FragmentInput` provides the following fields:

- `world_pos`: world-space position of the fragment.
- `normal_ws`: world-space normal.
- `packed_color`: packed base color, decode with `unpack_rgba8`.
- `packed_emissive`: packed emissive RGB, decode with `unpack_rgba8(...).xyz`.
- `packed_pbr_params_0`: packed preset params, decode with `decode_standard_pbr_params` or `decode_toon_params`.
- `packed_pbr_params_1`: packed secondary params; standard currently uses it for future data, mesh blend uses it for blend params.
- `packed_material_params`: packed alpha, side, and flags, decode with `decode_material_params`.
- `custom_range`: `(offset, length)` for the custom params block.
- `uv`: mesh UV0.
- `paint_uv`: mesh UV1 for paint/mask atlases; falls back to UV0 when UV1 is missing.

Decoded material flags:

- `alpha_mode`: `0` opaque, `1` mask, `2` blend.
- `alpha_cutoff`: mask cutoff.
- `double_sided`: double-sided normal handling.
- `meshlet_debug_view`: debug output.
- `flat_shading`: derive face normal in fragment shader.
- `has_base_color_texture`: base color texture bound.
- `mesh_blend`: screen blend alpha enabled.
- `normal_blend`: contact normal blend enabled.
- `mirrored_winding`: mirrored transform winding.
- `receive_shadows`: receive shadows enabled.

Packed preset params:

- Standard: `decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1)` returns `(roughness, metallic, occlusion_strength, normal_scale)`.
- Toon: `decode_toon_params(in.packed_pbr_params_0, in.packed_pbr_params_1)` returns `(band_count, rim_strength, outline_width)`.

Example usage:

```wgsl
let color = unpack_rgba8(in.packed_color);
let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
let material = decode_material_params(in.packed_material_params);
let alpha = perro_material_alpha(in, color.a);
let glow = custom_f_param(in, 0u).x;
```

Custom param packing:

- Runtime stores params in packed metadata + float payload buffers.
- `custom_f_param(...)` / `custom_v_param(...)` return logical `vec4` values:
  - `F32`, `I32`, `Bool` -> `vec4(x, 0, 0, 0)`
  - `Vec2` -> `vec4(x, y, 0, 0)`
  - `Vec3` -> `vec4(x, y, z, 0)`
  - `Vec4` -> `vec4(x, y, z, w)`

Custom param ordering:

- `custom_f_param(in, 0u)` maps to the **first** entry in `CustomMaterial3D::params`.
- Names are metadata only; ordering is what binds to indices.

Custom image ordering:

- `.pmat images` order maps to `custom_image_sample(in, index, uv)`.
- Max images per custom material: 8.
- Missing images sample the white fallback texture.
- Custom image sampling is for single-mesh/skinned custom material shaders.

### Default Lit Custom Example

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let tint = custom_f_param(in, 0u);
    return vec4<f32>(color.rgb * tint.rgb, color.a * tint.a);
}
```

The engine lights this return value with standard lighting.

### Raw Custom Example

Use `lighting = "raw"` in the material:

```txt
type = "custom"
shader_path = "res://shaders/custom.wgsl"
lighting = "raw"
```

Then return final color directly:

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let glow = custom_f_param(in, 0u).x;
    let alpha = perro_material_alpha(in, color.a);
    return vec4<f32>(color.rgb + glow, alpha);
}
```

### Custom Image Example

```txt
type = "custom"
shader_path = "res://shaders/portal.wgsl"
lighting = "raw"

images = {
    mask = "res://textures/portal_mask.png"
    noise = "res://textures/noise.png"
}
```

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let mask = custom_image_sample(in, 0u, in.uv);
    let noise = custom_image_sample(in, 1u, in.uv * 4.0);
    return vec4<f32>(noise.rgb, mask.r);
}
```

### Manual Lit Custom Example

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let tint = custom_f_param(in, 0u);
    return perro_standard(
        in,
        vec4<f32>(color.rgb * tint.rgb, color.a * tint.a),
        pbr.x,
        pbr.y,
        pbr.z,
        emissive,
    );
}
```

This form is useful when the shader wants custom roughness, metallic, ao, or emissive values.
The engine detects the helper call and skips automatic lighting.
In a scene with no sky and no lights, lit custom output returns black unless `emissive` is non-zero.
A material like `emissive_factor = (0.01, 0.08, 0.12)` stays visible because emissive is added after lighting.

### Frame Globals

Custom material shaders (single-mesh and multimesh, vertex and fragment stage) can read
engine frame globals through these helpers:

- `perro_time() -> f32`: seconds since app start. Wraps every hour so `f32`
  stays sub-millisecond precise; use `perro_time_phase()` or `sin(perro_time())`
  style math that tolerates the wrap.
- `perro_delta_time() -> f32`: seconds covered by the previous frame.
- `perro_frame_index() -> f32`: frames rendered since app start.
- `perro_time_phase() -> f32`: normalized `0..1` sawtooth over 60 seconds —
  a precision-safe driver for looping animation
  (`sin(perro_time_phase() * TAU * cycles_per_minute)`).
- `perro_resolution() -> vec2<f32>`: viewport size in pixels.
- `perro_inv_resolution() -> vec2<f32>`: `1.0 / viewport size` (e.g.
  `in.frag_pos.xy * perro_inv_resolution()` gives normalized screen UV).

Example — a pulsing, screen-aware effect with a vertex wobble:

```wgsl
fn shade_vertex(out_in: VertexOutput) -> VertexOutput {
    var out = out_in;
    out.world_pos.y += sin(perro_time() * 2.0 + out.world_pos.x) * 0.1;
    return out;
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let pulse = 0.5 + 0.5 * sin(perro_time_phase() * 6.28318 * 12.0);
    let screen_uv = in.frag_pos.xy * perro_inv_resolution();
    return vec4<f32>(color.rgb * pulse, color.a);
}
```

### Current Limitations

- Custom shaders can implement any shading model; the built-in inputs are the fields in
  `FragmentInput`, `custom_f_param(in, index)`, and the frame globals above.

### Runtime Performance Notes

- Custom material parameter blocks are interned by value and reused across frames.
- New unique custom param blocks append once and upload incrementally instead of re-uploading the
  entire custom param buffer each frame.
- Static builds strip comments and normalize whitespace before embedding WGSL.
- Rigid and skinned built-in materials automatically select lazy, cached shader variants from
  texture presence, alpha mode, shadow receiving, and vertex-modifier use. No author flags are
  required.
- Custom shaders loaded from static resources or disk keep the same composition and lazy pipeline
  cache path.

#### Built-in Shader Variant GPU Benchmark

`ShaderVariantMode::Auto` is the default. Games do not need to opt in or tag materials.
`ShaderVariantMode::Generic` exists as an A/B baseline for tests and benchmarks.

Run the three paired scenes with 120 warm-up frames and 2,000 measured frames per case:

```powershell
$env:PERRO_GPU_BENCH = "shader_variant"
$env:PERRO_GPU_WARMUP_FRAMES = "120"
$env:PERRO_GPU_SAMPLE_FRAMES = "2000"
$env:PERRO_GPU_BENCH_CSV = "target/shader-variant-gpu.csv"
cargo bench -p perro_graphics --bench gpu_frame
```

Set `PERRO_GPU_BENCH_REVERSE=1` for a second pass in reverse case order. Output includes median and
p95 GPU-main time, CPU prepare time, CPU encode time, and 3D pipeline switches. Negative deltas
mean the automatic variant is faster.

Local RX 7800 XT snapshot at 1280x720, no MSAA, no vsync, three runs per case:

| Scene | GPU median delta | GPU p95 delta | CPU/encode | Pipeline switches |
| ----- | ---------------- | ------------- | ---------- | ----------------- |
| Plain, 100k instances | ~+0.2% | ~+0.5% | no material change | 1 -> 1 |
| Full-screen, five textures | ~-2.1% | ~-3.2% | no material change | 1 -> 1 |
| Full-screen, ray shadow | ~-9.8% | ~-2.5% | no material change | 1 -> 1 |

These are local measurements, not a cross-GPU guarantee. The plain case shows the expected neutral
result when removed branches do not dominate. The fragment-heavy and shadow-heavy cases show where
specialization reduces GPU work.

### Breaking Change

- Shaders that directly accessed the old prelude symbol `custom_params` as
  `array<vec4<f32>>` must be updated.
- Use `custom_f_param(...)` / `custom_v_param(...)` helpers instead of raw storage access.
