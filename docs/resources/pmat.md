# `.pmat` Format

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`.pmat` is a text material profile for `MeshInstance3D` surfaces. It picks a shading preset (`standard`, `unlit`, `toon`, `hand_drawn`, `pixel_surface`, or `custom`) and sets the factors and texture slots that give a surface its look.

## Use Cases

- Physically based props: a `standard` material with `base_color_factor`, `metallic_factor`, and `roughness_factor` for crates, metal, stone.
- Glowing UI holograms and skyboxes: an `unlit` material so lighting never darkens `base_color_factor`/`emissive_factor`.
- Stylized/cel-shaded characters: a `toon` material with `band_count`, `rim_strength`, and `outline_width`.
- Ink and sketch surfaces: `hand_drawn` with toon bands, crosshatch, and paper grain.
- Low-resolution texture detail: `pixel_surface` with per-surface texel count, color levels, and dither.
- Portals, force fields, dissolves: a `custom` material with `shader_path` plus `params` and up to eight `images` sampled via `custom_image_sample`.
- Cutout foliage and glass: `alpha_mode = "MASK"` with `alpha_cutoff`, or `alpha_mode = "BLEND"`, plus `double_sided = true`.
- Imported model materials: reference a glTF sub-asset like `res://models/crate.glb:mat[0]` instead of a `.pmat` file.

## Choice Guide

Use `standard` for PBR, `unlit` for exact color, `toon` for cel light,
`hand_drawn` for ink texture, and `pixel_surface` for snapped mesh texture.
Use `custom` when presets cannot express the look.

## Example

Author `res://materials/crate.pmat` (`type` must be the first non-empty line):

```txt
type = "standard"

base_color_factor = (0.55, 0.38, 0.20, 1.0)
metallic_factor = 0.0
roughness_factor = 0.8
alpha_mode = "OPAQUE"
double_sided = false
```

Assign it to a mesh surface in a scene:

```scn
[Crate]
    [MeshInstance3D]
        mesh = "res://models/crate.glb:mesh[0]"
        material = "res://materials/crate.pmat"
    [/MeshInstance3D]
[/Crate]
```

Load or mutate it from a script:

```rust
let mat = material_load!(res, "res://materials/crate.pmat");
if let Some(mut data) = material_get_data!(res, mat) {
    if let Material3D::Standard(params) = &mut data {
        params.roughness_factor = 0.3;
    }
    let _ = material_write!(res, mat, data);
}
```

## Reference

# `.pmat` Format

`*.pmat` is a **Perro Material** resource and defines a material profile used by `MeshInstance3D`.

You can reference it in scene/scripts like:

```scn
material = "res://materials/mat.pmat"
```

## Material Type (Required First Entry)

`.pmat` now declares a **material preset** as the first entry:

```txt
type = "standard"
```

Valid values:

- `standard`
- `unlit`
- `toon`
- `hand_drawn`
- `pixel_surface`
- `custom`

The `type` entry **must be the first non-empty line** (comments are allowed above it).

## Recommended Syntax (Key/Value)

`.pmat` supports a clean line-based format:

```txt
type = "standard"

base_color_factor = (0.1, 0.5, 0.2, 1.0)
metallic_factor = 1.0
roughness_factor = 0.3

base_color_texture = 0
metallic_roughness_texture = 1
normal_texture = 2
occlusion_texture = 3
emissive_texture = 4

occlusion_strength = 1.0
emissive_factor = (0.0, 0.0, 0.0)
normal_scale = 1.0

alpha_mode = "OPAQUE"
alpha_cutoff = 0.5
double_sided = false
```

Comments:

- `# comment`
- `// comment`

## Supported Keys

### Standard

- `base_color_factor` (alias: `baseColorFactor`, `color`) vec3/vec4
- `metallic_factor` (alias: `metallicFactor`) float
- `roughness_factor` (alias: `roughnessFactor`) float
- `occlusion_strength` (alias: `occlusionStrength`) float
- `emissive_factor` (alias: `emissiveFactor`) vec3/vec4
- `normal_scale` (alias: `normalScale`) float
- `alpha_mode` (alias: `alphaMode`) `OPAQUE | MASK | BLEND`
- `alpha_cutoff` (alias: `alphaCutoff`) float
- `double_sided` (alias: `doubleSided`) bool
- `flat_shading` (alias: `flatShading`) bool (`false` = smooth, `true` = flat)
- `base_color_texture` (alias: `baseColorTexture`) int
- `metallic_roughness_texture` (alias: `metallicRoughnessTexture`) int
- `normal_texture` (alias: `normalTexture`) int
- `occlusion_texture` (alias: `occlusionTexture`) int
- `emissive_texture` (alias: `emissiveTexture`) int

Standard textures use glTF metallic-roughness rules:

- base color and emissive textures are sampled as sRGB color
- metallic-roughness textures are linear data: G = roughness, B = metallic, R is ignored
- occlusion textures are linear data: R = ambient occlusion; `occlusion_strength` blends from 1 to R
- normal textures are linear tangent-space normals; `normal_scale` scales X/Y before normalization
- all slots use UV0; the same sampling path applies to meshes and multimeshes
- missing slots bind neutral white data; missing normal slots bind a flat `(+Z)` normal

Custom materials also accept `images`:

```txt
type = "custom"
shader_path = "res://shaders/portal.wgsl"

images = {
    mask = "res://textures/portal_mask.png"
    noise = "res://textures/noise.png"
}
```

Image order is the shader index.
Names are metadata for tools and humans.
Use `custom_image_sample(in, 0u, in.uv)` for `mask`.
Use `custom_image_sample(in, 1u, in.uv)` for `noise`.
Max custom images: 8.

Note:
- When `base_color_texture` is unset (`MATERIAL_TEXTURE_NONE` internally), the renderer skips the
  base-color texture sample in Standard shading and uses factor-only color.

### Unlit

- `base_color_factor` (alias: `baseColorFactor`, `color`) vec3/vec4
- `emissive_factor` (alias: `emissiveFactor`) vec3/vec4
- `alpha_mode` (alias: `alphaMode`) `OPAQUE | MASK | BLEND`
- `alpha_cutoff` (alias: `alphaCutoff`) float
- `double_sided` (alias: `doubleSided`) bool
- `flat_shading` (alias: `flatShading`) bool
- `base_color_texture` (alias: `baseColorTexture`) int

### Toon

- `base_color_factor` (alias: `baseColorFactor`, `color`) vec3/vec4
- `emissive_factor` (alias: `emissiveFactor`) vec3/vec4
- `alpha_mode` (alias: `alphaMode`) `OPAQUE | MASK | BLEND`
- `alpha_cutoff` (alias: `alphaCutoff`) float
- `double_sided` (alias: `doubleSided`) bool
- `flat_shading` (alias: `flatShading`) bool
- `base_color_texture` (alias: `baseColorTexture`) int
- `ramp_texture` (alias: `rampTexture`) int
- `band_count` (alias: `bandCount`) int
- `rim_strength` (alias: `rimStrength`) float
- `outline_width` (alias: `outlineWidth`) float

### Hand Drawn

- all common color, alpha, side, flat-shading, and base-texture keys from `toon`
- `band_count` (alias: `bandCount`) int
- `hatch_scale` (alias: `hatchScale`) float
- `grain_strength` (alias: `grainStrength`) float

```txt
type = "hand_drawn"
base_color_factor = (0.9, 0.8, 0.65, 1.0)
band_count = 4
hatch_scale = 24.0
grain_strength = 0.06
```

### Pixel Surface

- common color, alpha, side, flat-shading, and base-texture keys
- `pixel_count` (alias: `pixelCount`) int; square texture grid per mesh UV
- `color_levels` (alias: `colorLevels`) int
- `dither_strength` (alias: `ditherStrength`) float

```txt
type = "pixel_surface"
base_color_texture = 0
pixel_count = 32
color_levels = 8
dither_strength = 0.08
```

`pixel_surface` snaps surface texture detail. Use camera `pixel_art` post-processing
for low-resolution mesh silhouettes and whole-frame pixels.

### Vertex Modifiers

All material types accept an ordered `vertex_modifiers` array.
The renderer applies up to 16 modifiers before a custom shader's `shade_vertex` hook.

```txt
type = "toon"

vertex_modifiers = [
    {
        type = "wind"
        direction = (1.0, 0.0, 0.0)
        strength = 0.2
        speed = 1.5
        frequency = 2.0
        mask = { axis = "y", start = 0.0, end = 2.0 }
    },
    {
        type = "pixel_snap"
        virtual_height = 32
        strength = 1.0
    }
]
```

Supported modifier types:

- `wind`: animated displacement along `direction`; keys `strength`, `speed`, `frequency`, optional `mask`
- `wave`: animated displacement; keys `axis`, `direction`, `amplitude`, `speed`, `frequency`, `phase`, optional `mask`
- `bend`: progressive rotation; keys `along_axis`, `bend_axis`, `angle_degrees` or `angle_radians`, `start`, `end`
- `twist`: progressive axial rotation; keys `axis`, `angle_degrees` or `angle_radians`, `start`, `end`
- `inflate`: normal displacement; keys `amount`, optional `mask`
- `jitter`: stepped random displacement; keys `amount`, `scale`, `rate`, `seed`, optional `mask`
- `pixel_snap`: clip-space vertex snapping; keys `virtual_height`, `strength`

Axes use `"x"`, `"y"`, or `"z"`.
Axis ranges and masks use world-space mesh positions.
Modifiers run in array order, so changing order changes the result.
Depth, cutout, and shadow passes apply the same stack.
Modifiers change rendered vertices only; collision and navigation geometry stay unchanged.
`pixel_snap` snaps vertices, not triangles; dense meshes produce a more sprite-like silhouette.

### Custom

Custom materials define a shader path and optional custom parameters:

```txt
type = "custom"
shader_path = "res://shaders/custom.wgsl"
# "surface" = Perro adds standard lighting
# "final" = exact shader result
output = "final"

params = {
    glow = 1.25
    tint = (1.0, 0.2, 0.4, 1.0)
}
```

#### Static Shader Baking

Custom materials can turn a procedural WGSL result into a texture during a static build:

```txt
type = "custom"
shader_path = "res://shaders/background.wgsl"
release_bake = true
bake_resolution = (1920, 1080)
output = "final"
```

- `release_bake = false` is the default and keeps runtime WGSL everywhere.
- `release_bake = true` keeps runtime WGSL in dev and selects a baked texture in release.
- `bake_resolution` sets the baked texture size and clamps each dimension to `1..8192`.
- `release_bake = true` defaults to `(1024, 1024)` when no resolution is set.
- Dynamic/dev loading keeps the runtime shader for fast iteration.

A mesh or individual surface can select the generated variant in a static scene:

```scn
[Backdrop]
    [MeshInstance3D]
        material = "res://materials/background.pmat"
        shader_use = "baked"
    [/MeshInstance3D]
[/Backdrop]
```

Use `shader_use = "runtime"` on the mesh or surface to force the runtime variant in a static build.
Use `shader_use = "baked"` to force its baked variant.
Surface entries use the same field beside their `material`/`source` field.
See `docs/resources/shaders.md` for the bake WGSL entry point.

## Inline Materials (Scene)

When defining materials inline in a `.scn` file, **string values must be quoted**:

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

```scn
material = {
    type = "unlit"
    base_color_factor = (0.2, 0.8, 1.0, 1.0)
    emissive_factor = (0.1, 0.2, 0.3)
    alpha_mode = "OPAQUE"
    double_sided = false
}
```

```scn
material = {
    type = "toon"
    base_color_factor = (0.4, 1.0, 0.4, 1.0)
    band_count = 3
    rim_strength = 0.35
    outline_width = 0.02
    alpha_mode = "OPAQUE"
    double_sided = false
}
```

```scn
material = {
    type = "custom"
    shader_path = "res://shaders/custom.wgsl"
    output = "final"
    alpha_mode = "OPAQUE"
    double_sided = false
    params = {
        glow = 1.25
        tint = (1.0, 0.2, 0.4, 1.0)
    }
}
```

Supported custom param value types:

- `float`, `int`, `bool`
- `vec2`, `vec3`, `vec4`

See also: `docs/resources/shaders.md` for WGSL authoring notes and current limitations.

## Types

- `float`: `0.5`
- `int`: `2`
- `bool`: `true | false`
- `vec2`: `(x, y)`
- `vec3`: `(x, y, z)`
- `vec4`: `(x, y, z, w)`
- `string/bare token`: used by `alpha_mode` (for example `OPAQUE`)
