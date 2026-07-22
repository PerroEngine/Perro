# `project.toml`

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Full Example | [Full Example](#full-example) |
| Tables | [Tables](#tables) |
| Project | [Project](#project) |
| Graphics | [Graphics](#graphics) |
| UI | [UI](#ui) |
| Texture Filter | [Texture Filter](#texture-filter) |
| Runtime | [Runtime](#runtime) |
| Physics | [Physics](#physics) |
| Audio | [Audio](#audio) |
| Localization | [Localization](#localization) |
| Steam | [Steam](#steam) |
| Web | [Web](#web) |
| Legacy Layout | [Legacy Layout](#legacy-layout) |
| Rules | [Rules](#rules) |

## Purpose

`project.toml` lives at the project root and declares your game's identity plus its runtime defaults. Both `perro dev` and `perro build` read it, so one file drives the boot scene, window shape, render quality, frame pacing, physics, audio, localization, Steam, and web metadata. Only `[project]` is required; every other table falls back to built-in defaults. Invalid enum strings or out-of-range numbers fail the parse hard, so a typo surfaces at load instead of silently changing behavior. Unknown tables print a warning and get ignored.

Every table is a flat top-level topic — no dotted subtables in the current layout. Older dotted layouts still parse; see [Legacy Layout](#legacy-layout).

## Use Cases

- **Pick the boot scene and app identity.** `[project]` sets `main_scene`, plus `name`, `icon`, `startup_splash`, and optional `version`/`company`/`copyright` export info.
- **Lock the game's shape for any window size.** `[graphics] aspect_ratio = "16:9"` derives the virtual canvas the runtime renders into.
- **Trade render quality against cost.** `[graphics]` tunes `hdr`, `msaa`, `ssao`, `occlusion_culling`, `texture_filter`, `particle_sim_default`, `default_font`, and the meshlet switches.
- **Control frame pacing and the fixed step.** `[runtime] frame_rate_cap` caps or uncaps FPS, and `target_fixed_update` sets the fixed-update rate.
- **Set world physics defaults.** `[physics] gravity` and `coef` seed the physics world.
- **Tune ray audio once for both dimensions.** `[audio] max_bounces = 4` sets 2D and 3D; add a `_2d`/`_3d` suffix to split them.
- **Ship to Steam or the web with correct metadata.** `[steam]` enables Steamworks with `app_id`/`input`, `[web]` sets page `title`/`description`/`keywords`.

## Config Ownership

Put project-wide defaults here: boot scene, window/render policy, fixed-step
rate, physics defaults, and platform metadata. Put per-scene choices in `.scn`
and per-instance choices in node fields or `script_vars`. Put live mutable game
state in scripts.

This boundary keeps a project setting from becoming an implicit override for
one scene. Tune quality here when every scene should share the tradeoff; use
scene/node settings when the cost or look belongs to one feature.

## Full Example

```toml
[project]
name = "My Game"
main_scene = "res://main.scn"
icon = "res://icon.png"
startup_splash = "res://icon.png"
# Optional identity/export info (Windows exe version info + engine detection).
version = "0.1.0"
description = "My Game"
company = "Studio Name"
copyright = "Copyright (c) 2026 Studio Name"
trademark = ""

[graphics]
aspect_ratio = "16:9"            # "WIDTH:HEIGHT" game shape
vsync = false
hdr = "auto"                     # auto | on | off
msaa = true
ssao = "medium"                  # off | low | medium | high | ultra
occlusion_culling = "gpu"        # cpu | gpu | off
particle_sim_default = "gpu"     # cpu | hybrid | gpu
texture_filter = "linear_mipmap" # nearest | linear | linear_mipmap | anisotropic
default_font = "default"         # default | system://Name | res://path.ttf
meshlets = false
dev_meshlets = false
release_meshlets = true
meshlet_debug_view = false

[ui]
pixel_snapping = true

[runtime]
frame_rate_cap = "unlimited"     # fps number | "unlimited" | "refresh_rate"
target_fixed_update = 60

[physics]
gravity = -9.81
coef = 1.0

[audio]
listener_max_distance = 500.0
propagation_tick_hz = 20
energy_cutoff = 0.02
debug_rays = false
# Ray propagation. Plain key sets 2D + 3D; `_2d` / `_3d` suffix tunes one path.
max_bounces = 4
max_ray_distance = 500.0
rays_per_tick_2d = 64
rays_per_tick_3d = 128

[localization]
default_locale = "en"

[steam]
enabled = false
app_id = 480
input = "off"

[web]
title = "My Game"
description = "My Game"
keywords = ["game", "perro"]
```

## Tables

| Table            | Need | Use                                 |
| ---------------- | ---- | ----------------------------------- |
| `[project]`      | yes  | name + entry assets + identity      |
| `[graphics]`     | no   | render defaults + global font       |
| `[ui]`           | no   | UI render defaults                  |
| `[runtime]`      | no   | frame timing                        |
| `[physics]`      | no   | world physics defaults              |
| `[audio]`        | no   | audio + ray propagation defaults    |
| `[localization]` | no   | locale default + sibling csv enable |
| `[steam]`        | no   | Steamworks cfg                      |
| `[web]`          | no   | web page metadata                   |

## Project

| Field            | Type            | Default          | Note           |
| ---------------- | --------------- | ---------------- | -------------- |
| `name`           | string          | need             | project name   |
| `main_scene`     | `res://` string | need             | first scene    |
| `icon`           | `res://` string | `res://icon.png` | app icon       |
| `startup_splash` | `res://` string | `res://icon.png` | startup splash |
| `version`        | string          | none             | Windows version info |
| `description`    | string          | none             | Windows version info |
| `company`        | string          | none             | Windows version info |
| `copyright`      | string          | none             | Windows version info |
| `trademark`      | string          | none             | Windows version info |

`main_scene`, `icon`, `startup_splash` must start w/ `res://`.

Empty identity string = none. Legacy `[metadata]` table still parses; `[project]` keys win when both set.

## Graphics

| Field                  | Type   | Default           | Values                       |
| ---------------------- | ------ | ----------------- | ---------------------------- |
| `aspect_ratio`         | string | `"16:9"`          | `"WIDTH:HEIGHT"`             |
| `vsync`                | bool   | `false`           | `true` / `false`             |
| `hdr`                  | string | `"auto"`          | `"auto"`, `"on"`, `"off"` |
| `msaa`                 | bool   | `true`            | `true` / `false`             |
| `ssao`                 | string | `"medium"`        | `"off"`, `"low"`, `"medium"`, `"high"`, `"ultra"` |
| `occlusion_culling`    | string | `"gpu"`           | `"cpu"`, `"gpu"`, `"off"`    |
| `particle_sim_default` | string | `"cpu"`           | `"cpu"`, `"hybrid"`, `"gpu"` |
| `texture_filter`       | string | `"linear_mipmap"` | see below                    |
| `default_font`         | string | `"default"`       | `"default"`, `system://Name`, `res://path.ttf` |
| `meshlets`             | bool   | `false`           | master meshlet switch        |
| `dev_meshlets`         | bool   | `false`           | dev meshlet draw             |
| `release_meshlets`     | bool   | `true`            | export meshlet bake          |
| `meshlet_debug_view`   | bool   | `false`           | debug draw path              |

`aspect_ratio` sets game shape.

`hdr = "auto"` picks native HDR when the surface, display path, and float scene target support it.
`"on"` requests HDR with safe SDR fallback. `"off"` forces SDR. Scripts can override the startup
choice with `hdr_set!(ctx.res, HdrMode::...)`.

Runtime derives internal canvas from it:

- `"16:9"` => `1920x1080`
- `"9:16"` => `1080x1920`
- `"4:3"` => `1440x1080`
- `"3:4"` => `1080x1440`

Window opens at 75% monitor size, fit to this aspect.

Render surface uses native window resolution.

WASM forces some graphics features off when platform lacks support.

`default_font` is the font for UI, 2D text, 3D labels, and text decals when node `font` stays default. Node font overrides this value. Missing fonts use the built-in fallback chain.

See [SSAO](../resources/ssao.md) for quality cost + render scope.

## UI

| Field            | Type | Default | Note                          |
| ---------------- | ---- | ------- | ----------------------------- |
| `pixel_snapping` | bool | `true`  | round final computed UI rects |

When enabled, UI computed rects round to physical pixels after float layout solve.

Child layout then reads rounded parent rects.

## Texture Filter

`texture_filter` sets global sampler + mip policy.

Applies now:

- 2D sprites
- UI images
- 3D material textures
- startup splash through sprite/UI texture path

| Value             | Mag/min | Mips | Aniso | Use                                   |
| ----------------- | ------- | ---- | ----- | ------------------------------------- |
| `"nearest"`       | nearest | no   | no    | pixel art                             |
| `"linear"`        | linear  | no   | no    | smooth art, exact-ish size            |
| `"linear_mipmap"` | linear  | yes  | no    | default, sprites + splash that shrink |
| `"anisotropic"`   | linear  | yes  | 16    | 3D angled surfaces                    |

Effects:

- mips cut shimmer when texture shrinks on screen
- mips add about 33% texture VRAM
- nearest keeps pixel art crisp
- linear smooths scale but may blur pixel art
- anisotropic helps floors/walls at angle

Current limit:

- global only
- node override planned for `Sprite2D`, `AnimatedSprite2D`, `MeshInstance3D`

## Runtime

| Field                 | Type          | Default       | Values                                      |
| --------------------- | ------------- | ------------- | ------------------------------------------- |
| `frame_rate_cap`      | string/number | `"unlimited"` | `"unlimited"`, `"refresh_rate"`, fps number |
| `target_fixed_update` | number        | `60`          | hz, `<= 0` disables fixed target            |

Aliases for unlimited:

- `"unlimited"`
- `"uncapped"`
- `"off"`
- `"none"`

Aliases for refresh:

- `"refresh_rate"`
- `"refresh"`
- `"display"`
- `"monitor"`

## Physics

| Field     | Type   | Default | Note     |
| --------- | ------ | ------- | -------- |
| `gravity` | number | `-9.81` | finite   |
| `coef`    | number | `1.0`   | positive |

## Audio

| Field                   | Type   | Default | Note                  |
| ----------------------- | ------ | ------- | --------------------- |
| `listener_max_distance` | number | `500.0` | max listen dist       |
| `propagation_tick_hz`   | number | `20`    | ray audio update rate |
| `energy_cutoff`         | number | `0.02`  | stop quiet rays       |
| `debug_rays`            | bool   | `false` | show debug rays       |

Ray propagation keys live flat in `[audio]`. Plain key sets both 2D + 3D. `_2d` / `_3d` suffix overrides one path and wins over the plain key.

| Field              | Type   | 2D default | 3D default | Suffix forms                                |
| ------------------ | ------ | ---------- | ---------- | ------------------------------------------- |
| `max_bounces`      | int    | `4`        | `4`        | `max_bounces_2d`, `max_bounces_3d`           |
| `rays_per_tick`    | int    | `64`       | `128`      | `rays_per_tick_2d`, `rays_per_tick_3d`       |
| `max_ray_distance` | number | `500.0`    | `500.0`    | `max_ray_distance_2d`, `max_ray_distance_3d` |

Example: shared distance, split ray counts.

```toml
[audio]
max_ray_distance = 250.0
rays_per_tick_2d = 32
rays_per_tick_3d = 96
```

All audio numbers must be `>= 0`. `max_bounces` caps at `32`.

## Localization

```toml
[localization]
default_locale = "en"
```

Enable localization table.

`default_locale` sets the startup language.

Use a locale code defined in [Localization](../scripting/contexts/resource_modules/localization.md).

Need one sibling csv next to `project.toml`:

- `localization.csv`
- `locale.csv`
- `translations.csv`

If sibling csv exists w/o `[localization]`, default locale = `en`.

## Steam

```toml
[steam]
enabled = false
app_id = 480
input = "off"
```

| Field     | Type   | Default | Note                              |
| --------- | ------ | ------- | --------------------------------- |
| `enabled` | bool   | `false` | Steamworks on/off                 |
| `app_id`  | int    | none    | need when enabled                 |
| `input`   | string | `"off"` | Steam Input mode: off/metadata/actions |

`app_id` must fit `u32`.

Use `input = "off"` to keep native Perro input only.
Use `input = "metadata"` to read Steam controller type/glyph/origin data without Steam Input action reads.
Use `input = "actions"` only when the game opts into Steam Input action maps.

## Web

```toml
[web]
title = "My Game"
description = "My Game"
keywords = ["game", "perro"]
```

| Field         | Type         | Default | Note              |
| ------------- | ------------ | ------- | ----------------- |
| `title`       | string       | none    | web title         |
| `description` | string       | none    | web meta desc     |
| `keywords`    | string/array | none    | web meta keywords |

## Legacy Layout

Older projects keep working. All legacy forms parse; the flat form wins when both appear.

| Legacy                                    | Current                                        |
| ----------------------------------------- | ---------------------------------------------- |
| `[metadata]` identity fields               | same fields in `[project]`                     |
| `[rendering] default_font`                 | `[graphics] default_font`                      |
| `[rendering.ui] pixel_snapping`            | `[ui] pixel_snapping`                          |
| `[audio.propagation_2d]` `max_bounces` etc | `[audio]` `max_bounces` / `max_bounces_2d` etc |
| `[audio.propagation_3d]` `max_bounces` etc | `[audio]` `max_bounces` / `max_bounces_3d` etc |

## Rules

- Use TOML syntax.
- Use `res://` for project asset refs.
- Keep bools as `true` / `false`.
- Keep invalid graphics strings out; parser errors hard.
- Prefer `aspect_ratio = "16:9"` over exact virtual size.
- Put localization csv next to `project.toml`, not inside `res/`.
- Unknown tables warn + get ignored; check spelling when a setting seems dead.
