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

### Current Status

Custom shader execution is **not yet wired into the 3D renderer**. The engine currently renders using the standard 3D pipeline and does not bind custom shader code or parameters. The `shader_path` and `params` are stored for future use.

### Planned Interface (TBD)

When custom materials are fully supported, the shader is expected to:

- Provide `vs_main` and `fs_main` entry points (consistent with other 3D shaders).
- Consume the standard 3D instance data (model matrix, base color, PBR params).
- Bind custom params supplied by `CustomMaterial3D::params`.

The exact binding layout and param mapping will be documented once the runtime pipeline is implemented.
