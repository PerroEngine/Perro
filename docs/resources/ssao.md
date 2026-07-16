# SSAO

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Config | [Config](#config) |
| Render Path | [Render Path](#render-path) |
| Surface Scope | [Surface Scope](#surface-scope) |
| Resize + Fallback | [Resize + Fallback](#resize-fallback) |

## Purpose

SSAO (screen-space ambient occlusion) darkens contact areas such as corners, crevices, and where objects meet the ground, using the scene depth buffer. It only dims ambient light, so direct lights and emissive surfaces stay unchanged and the effect reads as subtle grounding rather than a color shift. It adds depth and weight to a scene without any extra authored geometry.

## Use Cases

- Grounding props and characters: a soft contact darkening where a crate or a foot meets the floor, reconstructed from the opaque depth prepass.
- Readable interiors: wall junctions and corners gain shading so rooms do not look flat.
- Scalable quality: pick `ssao = "off" | "low" | "medium" | "high" | "ultra"` in `project.toml` to trade cost for resolution and sample count.
- Low-end GPU targets: `"off"` binds a white fallback so ambient light stays unchanged.
- Consistent across draw types: standard meshes and custom multimesh standard-lit surfaces both sample the result, while unlit and emissive terms are left alone.

## Config

Set quality in `project.toml`:

```toml
[graphics]
ssao = "medium"
```

| Value | Resolution | Samples | Bilateral blur |
| --- | --- | ---: | --- |
| `"off"` | none | 0 | no |
| `"low"` | half | 4 | no |
| `"medium"` | half | 8 | yes |
| `"high"` | half | 12 | yes |
| `"ultra"` | full | 16 | yes |

Default = `"medium"`.

Use `"off"` for low-end GPU targets or exact pre-SSAO output.

## Render Path

Opaque depth prepass runs first.

SSAO reconstructs world position from depth.

Per-pixel rotated horizon samples reduce fixed ring bands.

Medium + higher tiers run depth-aware bilateral blur.

Standard mesh + multimesh ambient terms sample result.

## Surface Scope

Opaque standard surfaces receive SSAO.

Custom multimesh standard-lighting surfaces receive SSAO.

Unlit + emissive terms do not receive SSAO.

Water does not write opaque prepass depth.

Water top-surface light does not sample SSAO from geometry behind it.

Water refraction still captures opaque scene after SSAO shading.

## Resize + Fallback

Resize rebuilds depth-linked SSAO targets + bind groups.

Quality pipelines stay cached across resize.

Disabled SSAO binds white `1x1` texture.

White fallback keeps ambient light unchanged.
