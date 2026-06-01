# `project.toml`

## Page Map

- [Purpose](#purpose)
- [Full Example](#full-example)
- [Tables](#tables)
- [Graphics](#graphics)
- [Texture Filter](#texture-filter)
- [Runtime](#runtime)
- [Physics](#physics)
- [Audio](#audio)
- [Localization](#localization)
- [Steam](#steam)
- [Web](#web)
- [Rules](#rules)

## Purpose

`project.toml` sets project metadata + runtime defaults.

Put file at project root.

Dev + export both read it.

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
meshlets = false
dev_meshlets = false
release_meshlets = true
meshlet_debug_view = false
occlusion_culling = "gpu"
particle_sim_default = "gpu"
texture_filter = "linear_mipmap"

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
```

## Tables

| Table                    | Need | Use                                 |
| ------------------------ | ---- | ----------------------------------- |
| `[project]`              | yes  | name + entry assets                 |
| `[graphics]`             | yes  | render defaults                     |
| `[runtime]`              | no   | frame timing                        |
| `[physics]`              | no   | world physics defaults              |
| `[audio]`                | no   | audio propagation defaults          |
| `[audio.propagation_2d]` | no   | 2D ray audio defaults               |
| `[audio.propagation_3d]` | no   | 3D ray audio defaults               |
| `[metadata]`             | no   | native export metadata              |
| `[web]`                  | no   | web page metadata                   |
| `[localization]`         | no   | locale default + sibling csv enable |
| `[steam]`                | no   | Steamworks cfg                      |

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

Need one sibling csv next to `project.toml`:

- `localization.csv`
- `locale.csv`
- `translations.csv`

First column must be key.

Other columns use locale codes.

If sibling csv exists w/o `[localization]`, default locale = `en`.

## Steam

```toml
[steam]
enabled = false
app_id = 480
```

| Field     | Type | Default | Note              |
| --------- | ---- | ------- | ----------------- |
| `enabled` | bool | `false` | Steamworks on/off |
| `app_id`  | int  | none    | need when enabled |

`app_id` must fit `u32`.

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
