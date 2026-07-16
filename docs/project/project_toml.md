# `project.toml`

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Full Example | [Full Example](#full-example) |
| Tables | [Tables](#tables) |
| Graphics | [Graphics](#graphics) |
| Rendering UI | [Rendering UI](#rendering-ui) |
| Texture Filter | [Texture Filter](#texture-filter) |
| Runtime | [Runtime](#runtime) |
| Physics | [Physics](#physics) |
| Audio | [Audio](#audio) |
| Localization | [Localization](#localization) |
| Steam | [Steam](#steam) |
| Web | [Web](#web) |
| Rules | [Rules](#rules) |

## Purpose

`project.toml` lives at the project root and declares your game's identity plus its runtime defaults. Both `perro dev` and `perro build` read it, so one file drives the boot scene, window shape, render quality, frame pacing, physics, audio, localization, Steam, and web metadata. Only `[project]` and `[graphics]` are required; every other table falls back to built-in defaults. Invalid enum strings or out-of-range numbers fail the parse hard, so a typo surfaces at load instead of silently changing behavior.

## Use Cases

- **Pick the boot scene and app identity.** `[project]` sets `main_scene`, plus `name`, `icon`, and `startup_splash`.
- **Lock the game's shape for any window size.** `[graphics] aspect_ratio = "16:9"` derives the virtual canvas the runtime renders into.
- **Trade render quality against cost.** `[graphics]` tunes `msaa`, `ssao`, `occlusion_culling`, `texture_filter`, `particle_sim_default`, and the meshlet switches.
- **Control frame pacing and the fixed step.** `[runtime] frame_rate_cap` caps or uncaps FPS, and `target_fixed_update` sets the fixed-update rate.
- **Set world physics defaults.** `[physics] gravity` and `coef` seed the physics world.
- **Ship to Steam or the web with correct metadata.** `[steam]` enables Steamworks with `app_id`/`input`, `[web]` sets page `title`/`description`/`keywords`, and `[metadata]` fills Windows executable version info.

## Full Example

```toml
[project]
name = "My Game"
main_scene = "res://main.scn"
icon = "res://icon.png"
startup_splash = "res://icon.png"

[metadata]
description = "My Game"
company = "Studio Name"
version = "0.1.0"
copyright = "Copyright (c) 2026 Studio Name"
trademark = ""

[web]
title = "My Game"
description = "My Game"
keywords = ["game", "perro"]

[graphics]
aspect_ratio = "16:9"
vsync = false
msaa = true
ssao = "medium"
meshlets = false
dev_meshlets = false
release_meshlets = true
meshlet_debug_view = false
occlusion_culling = "gpu"
particle_sim_default = "gpu"
texture_filter = "linear_mipmap"

[rendering]
default_font = "default"

[rendering.ui]
pixel_snapping = true

[runtime]
frame_rate_cap = "unlimited"
target_fixed_update = 60

[physics]
gravity = -9.81
coef = 1.0

[audio]
listener_max_distance = 500.0
propagation_tick_hz = 20
energy_cutoff = 0.02
debug_rays = false

[audio.propagation_2d]
max_bounces = 4
rays_per_tick = 64
max_ray_distance = 500.0

[audio.propagation_3d]
max_bounces = 4
rays_per_tick = 128
max_ray_distance = 500.0

[localization]
default_locale = "en"

[steam]
enabled = false
app_id = 480
input = "off"
```

## Tables

| Table                    | Need | Use                                 |
| ------------------------ | ---- | ----------------------------------- |
| `[project]`              | yes  | name + entry assets                 |
| `[graphics]`             | yes  | render defaults                     |
| `[rendering]`            | no   | shared text render defaults         |
| `[rendering.ui]`         | no   | UI render defaults                  |
| `[runtime]`              | no   | frame timing                        |
| `[physics]`              | no   | world physics defaults              |
| `[audio]`                | no   | audio propagation defaults          |
| `[audio.propagation_2d]` | no   | 2D ray audio defaults               |
| `[audio.propagation_3d]` | no   | 3D ray audio defaults               |
| `[metadata]`             | no   | native export metadata              |
| `[web]`                  | no   | web page metadata                   |
| `[localization]`         | no   | locale default + sibling csv enable |
| `[steam]`                | no   | Steamworks cfg                      |

## `[rendering]`

| Field | Type | Default | Note |
| --- | --- | --- | --- |
| `default_font` | string | `"default"` | Font for UI, 2D text, 3D labels, and text decals when node `font` stays default. Accepts `system://Name` or `res://path.ttf`. Node font overrides this value. Missing fonts use the built-in fallback chain. |

## `[project]`

| Field            | Type            | Default          | Note           |
| ---------------- | --------------- | ---------------- | -------------- |
| `name`           | string          | need             | project name   |
| `main_scene`     | `res://` string | need             | first scene    |
| `icon`           | `res://` string | `res://icon.png` | app icon       |
| `startup_splash` | `res://` string | `res://icon.png` | startup splash |

`main_scene`, `icon`, `startup_splash` must start w/ `res://`.

## `[metadata]`

| Field         | Type   | Default | Note                 |
| ------------- | ------ | ------- | -------------------- |
| `description` | string | none    | Windows version info |
| `company`     | string | none    | Windows version info |
| `version`     | string | none    | Windows version info |
| `copyright`   | string | none    | Windows version info |
| `trademark`   | string | none    | Windows version info |

Empty string = none.

## Graphics

| Field                  | Type   | Default           | Values                       |
| ---------------------- | ------ | ----------------- | ---------------------------- |
| `aspect_ratio`         | string | `"16:9"`          | `"WIDTH:HEIGHT"`             |
| `vsync`                | bool   | `false`           | `true` / `false`             |
| `msaa`                 | bool   | `true`            | `true` / `false`             |
| `ssao`                 | string | `"medium"`        | `"off"`, `"low"`, `"medium"`, `"high"`, `"ultra"` |
| `meshlets`             | bool   | `false`           | master meshlet switch        |
| `dev_meshlets`         | bool   | `false`           | dev meshlet draw             |
| `release_meshlets`     | bool   | `true`            | export meshlet bake          |
| `meshlet_debug_view`   | bool   | `false`           | debug draw path              |
| `occlusion_culling`    | string | `"gpu"`           | `"cpu"`, `"gpu"`, `"off"`    |
| `particle_sim_default` | string | `"cpu"`           | `"cpu"`, `"hybrid"`, `"gpu"` |
| `texture_filter`       | string | `"linear_mipmap"` | see below                    |

`aspect_ratio` sets game shape.

Runtime derives internal canvas from it:

- `"16:9"` => `1920x1080`
- `"9:16"` => `1080x1920`
- `"4:3"` => `1440x1080`
- `"3:4"` => `1080x1440`

Window opens at 75% monitor size, fit to this aspect.

Render surface uses native window resolution.

WASM forces some graphics features off when platform lacks support.

See [SSAO](../resources/ssao.md) for quality cost + render scope.

## Rendering UI

| Field            | Type | Default | Note                         |
| ---------------- | ---- | ------- | ---------------------------- |
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

`audio.propagation_2d`:

| Field              | Type   | Default |
| ------------------ | ------ | ------- |
| `max_bounces`      | int    | `4`     |
| `rays_per_tick`    | int    | `64`    |
| `max_ray_distance` | number | `500.0` |

`audio.propagation_3d`:

| Field              | Type   | Default |
| ------------------ | ------ | ------- |
| `max_bounces`      | int    | `4`     |
| `rays_per_tick`    | int    | `128`   |
| `max_ray_distance` | number | `500.0` |

All audio numbers must be `>= 0`.

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

## Rules

- Use TOML syntax.
- Use `res://` for project asset refs.
- Keep bools as `true` / `false`.
- Keep invalid graphics strings out; parser errors hard.
- Prefer `aspect_ratio = "16:9"` over exact virtual size.
- Put localization csv next to `project.toml`, not inside `res/`.
