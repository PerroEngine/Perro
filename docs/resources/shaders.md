# Shaders (WGSL)

Perro uses **WGSL** (`.wgsl`) for GPU shaders. Shaders are referenced by custom materials via a `shader_path`.

## Custom 3D Material Shaders

Custom 3D materials are declared as:

```txt
type = custom
shader_path = "res://shaders/custom.wgsl"
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
3. The engine appends a tiny wrapper fragment function:

```wgsl
@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4<f32> {
    return shade_material(in);
}
```

### What You Need To Implement

Your `.wgsl` only needs to define:

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    // use in.color, in.pbr_params, in.emissive_factor, in.material_params
}
```

You **do not** need to define `vs_main`, bind groups, or scene structs.

Notes for custom shaders:

- `in.material_params` packs: `alpha_mode`, `alpha_cutoff`, `double_sided`, and a debug flag.
- If you want alpha clipping or blending behavior, implement it in `shade_material`.
- Use `custom_param(in, index)` to read custom params (each param is a vec4<f32>).

### FragmentInput Fields

`FragmentInput` provides the following fields:

- `world_pos`: world-space position of the fragment.
- `normal_ws`: world-space normal.
- `color`: base color (from the material preset).
- `pbr_params`: a generic `vec4` for preset-specific params.
  - Standard: `(roughness, metallic, occlusion_strength, normal_scale)`
  - Unlit: `(0, 0, 0, 0)`
  - Toon: `(band_count, rim_strength, outline_width, 0)`
- `emissive_factor`: emissive RGB from the preset.
- `material_params`: `(alpha_mode, alpha_cutoff, double_sided, debug_flag)`
- `custom_range`: `(offset, length)` for the custom params block.

Example usage:

```wgsl
let base = in.color.rgb;
let roughness = in.pbr_params.x; // if using Standard preset
let alpha_mode = u32(in.material_params.x + 0.5);
let glow = custom_param(in, 0u).x;
```

Custom param packing:

- `F32`, `I32`, `Bool` -> `vec4(x, 0, 0, 0)`
- `Vec2` -> `vec4(x, y, 0, 0)`
- `Vec3` -> `vec4(x, y, z, 0)`
- `Vec4` -> `vec4(x, y, z, w)`

Custom param ordering:

- `custom_param(in, 0u)` maps to the **first** entry in `CustomMaterial3D::params`.
- Names are metadata only; ordering is what binds to indices.

### Template Example (Complete File)

```wgsl
fn shade_material(in: FragmentInput) -> vec4<f32> {
    let base = in.color.rgb;
    let alpha_mode = u32(in.material_params.x + 0.5);
    let alpha_cutoff = clamp(in.material_params.y, 0.0, 1.0);
    var alpha = clamp(in.color.a, 0.0, 1.0);
    if alpha_mode == 1u && alpha < alpha_cutoff {
        discard;
    }
    if alpha_mode == 0u {
        alpha = 1.0;
    }

    let glow = custom_param(in, 0u).x;
    let tint = custom_param(in, 1u);

    let color = base * tint.rgb + glow;
    return vec4<f32>(color, alpha);
}
```

### Current Limitations

- Custom params are packed into `vec4<f32>` slots.
- Custom shaders can implement any shading model, but the only built-in inputs are the fields in
  `FragmentInput` plus `custom_param(in, index)`.
