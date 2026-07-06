# Materials Guide

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Materials Guide` when this feature, type group, file format, or workflow appears in game code or assets.

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

# Materials Guide

This guide shows the normal material path:

1. Author a `.pmat` file or use a glTF material.
2. Load or reserve a `MaterialID` when scripts need one.
3. Assign material sources in `MeshInstance3D` or `MultiMeshInstance3D`.
4. Create or mutate material data at runtime when needed.
5. Let static builds bake supported material sources for release.

For exact file syntax, see [`.pmat` Format](pmat.md).
For API details, see [Materials Module](../scripting/contexts/resource_modules/materials.md).
For custom shader authoring, see [Shaders](shaders.md).

## glTF / GLB Sub-Assets

A `.gltf` or `.glb` file can hold many resources in one container.
Perro treats those inner resources like addressable sub-assets.
Use a suffix after the file path to pick the item you want:

```txt
res://models/robot.glb:mesh[0]
res://models/robot.glb:mat[0]
res://models/robot.glb:skeleton[0]
```

This means a model file can act like a small resource folder.
You do not need to extract each mesh or material into separate files before using it.
Keep the `.glb` together, then point scene fields or resource APIs at the specific sub-asset.

Common suffixes:

- `:mesh[index]` targets a mesh.
- `:mat[index]` targets a material.
- `:material[index]` also targets a material.
- `:skeleton[index]` targets a skeleton/skin.
- `:tex[index]`, `:texture[index]`, and `:img[index]` target glTF texture/image data through texture loading.

If a mesh or material source omits the suffix, Perro uses index `0` for that resource type.
For textures, prefer `.glb` or embedded glTF image data; external `.gltf` image dependency support is not the main documented path.

## Author `.pmat`

`type` must be the first non-empty entry.
Comments can appear above it.

Standard material:

```txt
type = "standard"

base_color_factor = (0.8, 0.2, 0.2, 1.0)
metallic_factor = 0.1
roughness_factor = 0.7
alpha_mode = "OPAQUE"
double_sided = false
```

Unlit material:

```txt
type = "unlit"

base_color_factor = (0.2, 0.8, 1.0, 1.0)
emissive_factor = (0.1, 0.2, 0.3)
alpha_mode = "OPAQUE"
double_sided = false
```

Toon material:

```txt
type = "toon"

base_color_factor = (0.4, 1.0, 0.4, 1.0)
band_count = 3
rim_strength = 0.35
outline_width = 0.02
alpha_mode = "OPAQUE"
double_sided = false
```

Custom material:

```txt
type = "custom"
shader_path = "res://shaders/custom.wgsl"
# default: standard lighting wraps shader output
# use lighting = "raw" to opt out

params = {
    glow = 1.25
    tint = (1.0, 0.2, 0.4, 1.0)
}

images = {
    mask = "res://textures/mask.png"
    noise = "res://textures/noise.png"
}
```

## Assign In Scenes

Use a `.pmat` source on `material`:

```scn
[Crate]
    [MeshInstance3D]
        mesh = "res://models/crate.glb:mesh[0]"
        material = "res://materials/crate.pmat"
    [/MeshInstance3D]
[/Crate]
```

Use a glTF material sub-asset:

```scn
[Crate]
    [MeshInstance3D]
        mesh = "res://models/crate.glb:mesh[0]"
        material = "res://models/crate.glb:mat[0]"
    [/MeshInstance3D]
[/Crate]
```

Use per-surface material sources:

```scn
[Robot]
    [MeshInstance3D]
        mesh = "res://models/robot.glb:mesh[0]"
        surfaces = [
            "res://materials/body.pmat",
            {
                material = "res://materials/eyes.pmat"
                modulate = (1.0, 0.9, 0.9, 1.0)
                overrides = [
                    { name = "roughness", value = 0.25 },
                    { name = "shade_flat", value = true }
                ]
            }
        ]
    [/MeshInstance3D]
[/Robot]
```

Inline materials also work in scenes.
String values must be quoted in `.scn` material objects:

```scn
material = {
    type = "standard"
    base_color_factor = (0.8, 0.2, 0.2, 1.0)
    metallic_factor = 0.1
    roughness_factor = 0.7
    alpha_mode = "OPAQUE"
    double_sided = false
}
```

## Load Or Create In Scripts

Use source-backed calls for authored materials:

```rust
let mat = material_load!(res, "res://materials/crate.pmat");
let reserved = material_reserve!(res, "res://models/crate.glb:mat[0]");
let _ = material_drop!(res, "res://materials/old.pmat");
```

Use `material_create!` for generated or transient materials:

```rust
let mat_id = material_create!(
    res,
    Material3D::Standard(StandardMaterial3D {
        base_color_factor: [0.8, 0.2, 0.2, 1.0],
        roughness_factor: 0.4,
        metallic_factor: 0.1,
        ..StandardMaterial3D::default()
    })
);
```

Use `material_get_data!` and `material_write!` to replace an existing material value:

```rust
if let Some(mut mat_data) = material_get_data!(res, mat_id) {
    if let Material3D::Standard(params) = &mut mat_data {
        params.roughness_factor = 0.2;
    }
    let _ = material_write!(res, mat_id, mat_data);
}
```

`load` and `reserve` use a source cache.
`create` bypasses the source cache and returns a new material id.
`get_data` returns a copied value.
`write` replaces the full material data for that id.

## Static Builds

Static builds bake supported material sources into generated lookup data.
Release runtime loading can resolve baked `.pmat` sources and glTF material sub-assets without reparsing the original text path at runtime.

Generic files that do not have static bake support still belong in `assets.perro`.
See [Performance + Flexibility Philosophy](../project/performance_philosophy.md).

## Caveats

- `.pmat` `type` must be the first non-empty entry.
- `.scn` inline material strings must be quoted.
- glTF material refs use `res://path/to/model.glb:mat[index]`.
- glTF mesh refs use `res://path/to/model.glb:mesh[index]`.
- a single `.glb` can provide both mesh and material refs for the same scene node.
- texture slots are material-local/glTF texture indices, not global texture IDs.
- custom `images` bind up to 8 `res://` texture paths for `custom_image_sample(in, index, uv)`.
- custom materials use standard lighting by default; set `lighting = "raw"` for exact shader output.
- custom material parameter order binds shader indices: `custom_f_param(in, 0u)` reads the first param.
- custom param names are metadata for humans and tooling; order controls shader access.
