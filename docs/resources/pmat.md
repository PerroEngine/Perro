# `.pmat` Format

`*.pmat` defines a material profile used by `MeshInstance3D`.

You can reference it in scene/scripts like:

```scn
material = "res://materials/mat.pmat"
```

## Recommended Syntax (Key/Value)

`.pmat` supports a clean line-based format:

```txt
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

## Types

- `float`: `0.5`
- `int`: `2`
- `bool`: `true | false`
- `vec2`: `(x, y)`
- `vec3`: `(x, y, z)`
- `vec4`: `(x, y, z, w)`
- `string/bare token`: used by `alpha_mode` (for example `OPAQUE`)

## Backward Compatibility

Object-style `.pmat` is still supported for existing content.

