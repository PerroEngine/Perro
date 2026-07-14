# SSAO

## Page Map

- [Purpose](#purpose)
- [Config](#config)
- [Render Path](#render-path)
- [Surface Scope](#surface-scope)
- [Resize + Fallback](#resize--fallback)

## Purpose

SSAO adds short-range ambient shadow from visible scene depth.

It affects ambient light only.

Direct lights + emissive output stay unchanged.

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
