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

### Current Limitations

- Custom params from `CustomMaterial3D::params` are **not yet bound** to the shader.
- Custom shaders must use the existing `FragmentInput` fields (color, pbr params, etc.).

Once custom param binding is implemented, this doc will be expanded with the exact layout.
