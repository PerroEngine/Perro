# Shaders (WGSL)

## Page Map

| Header    | Link                    |
| --------- | ----------------------- |
| Purpose   | [Purpose](#purpose)     |
| Use Cases | [Use Cases](#use-cases) |
| Example   | [Example](#example)     |
| Reference | [Reference](#reference) |

## Reference

# Shaders (WGSL)

Perro uses **WGSL** (`.wgsl`) for GPU shaders. Shaders are referenced by custom materials via a `shader_path`.

## Custom 3D Material Shaders

Custom 3D materials are declared as:

```txt
type = "custom"
shader_path = "res://shaders/custom.wgsl"
# optional: lighting = "raw" opts out of automatic standard lighting
params = {
    glow = 1.25
    tint = (1.0, 0.2, 0.4, 1.0)
}
```

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

You **do not** need to define `vs_main`, `fs_main`, bind groups, or scene structs.

Notes for custom shaders:

- Custom shaders use standard lighting by default. The engine treats `shade_material(in)` as base color, then applies standard lighting.
- Add `lighting = "raw"` to a custom material to opt out and return exact shader output.
- `perro_lit_standard(in, base_color, roughness, metallic, ao, emissive)` applies the same standard material light path as built-in standard materials.
- `perro_material_alpha(in, alpha)` applies alpha cutoff, opaque alpha, and mesh blend alpha.
- If a shader calls `perro_lit_standard` itself, the engine does not wrap it a second time.
- If a scene has no `Sky3D`, no `AmbientLight3D`, and no 3D lights, standard materials render black except for `emissive_factor`.
- Use `custom_f_param(in, index)` to read custom params in fragment stage.
- Use `custom_v_param(out, index)` inside `shade_vertex` for same params in vertex stage.
- Legacy aliases `custom_param` and `custom_param_vertex` stay valid.

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
- `uv`: mesh UV.

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

### Manual Lit Custom Example

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let tint = custom_f_param(in, 0u);
    return perro_lit_standard(
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

### Current Limitations

- Custom shaders can implement any shading model, but the only built-in inputs are the fields in
  `FragmentInput` plus `custom_f_param(in, index)`.

### Runtime Performance Notes

- Custom material parameter blocks are interned by value and reused across frames.
- New unique custom param blocks append once and upload incrementally instead of re-uploading the
  entire custom param buffer each frame.

### Breaking Change

- Shaders that directly accessed the old prelude symbol `custom_params` as
  `array<vec4<f32>>` must be updated.
- Use `custom_f_param(...)` / `custom_v_param(...)` helpers instead of raw storage access.
