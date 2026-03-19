# `.pmat` Format

`*.pmat` is a **Perro Material** resource and defines a material profile used by `MeshInstance3D`.

You can reference it in scene/scripts like:

```scn
material = "res://materials/mat.pmat"
```

## Material Type (Required First Entry)

`.pmat` now declares a **material preset** as the first entry:

```txt
type = standard
```

Valid values:

- `standard`
- `unlit`
- `toon`
- `custom`

The `type` entry **must be the first non-empty line** (comments are allowed above it).

## Recommended Syntax (Key/Value)

`.pmat` supports a clean line-based format:

```txt
type = standard

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

alpha_mode = OPAQUE
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
- `base_color_texture` (alias: `baseColorTexture`) int
- `metallic_roughness_texture` (alias: `metallicRoughnessTexture`) int
- `normal_texture` (alias: `normalTexture`) int
- `occlusion_texture` (alias: `occlusionTexture`) int
- `emissive_texture` (alias: `emissiveTexture`) int

### Unlit

- `base_color_factor` (alias: `baseColorFactor`, `color`) vec3/vec4
- `emissive_factor` (alias: `emissiveFactor`) vec3/vec4
- `alpha_mode` (alias: `alphaMode`) `OPAQUE | MASK | BLEND`
- `alpha_cutoff` (alias: `alphaCutoff`) float
- `double_sided` (alias: `doubleSided`) bool
- `base_color_texture` (alias: `baseColorTexture`) int

### Toon

- `base_color_factor` (alias: `baseColorFactor`, `color`) vec3/vec4
- `emissive_factor` (alias: `emissiveFactor`) vec3/vec4
- `alpha_mode` (alias: `alphaMode`) `OPAQUE | MASK | BLEND`
- `alpha_cutoff` (alias: `alphaCutoff`) float
- `double_sided` (alias: `doubleSided`) bool
- `base_color_texture` (alias: `baseColorTexture`) int
- `ramp_texture` (alias: `rampTexture`) int
- `band_count` (alias: `bandCount`) int
- `rim_strength` (alias: `rimStrength`) float
- `outline_width` (alias: `outlineWidth`) float

### Custom

Custom materials define a shader path and optional custom parameters:

```txt
type = custom
shader_path = "res://shaders/custom.wgsl"

params = {
    glow = 1.25
    tint = (1.0, 0.2, 0.4, 1.0)
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
