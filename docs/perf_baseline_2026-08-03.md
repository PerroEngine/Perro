# Perf baseline, 2026-08-03

GPU-side frame + boot measurements taken while landing the shadow, LOD, boot and
idle-frame work. Machine: AMD Radeon RX 7800 XT, Vulkan backend, 2560x1440
display, window 1920x1080. Numbers are the MEDIAN of 800-1500 frame samples with
the first 300 rows dropped as warmup (`PERRO_TIMING_CSV`).

Reproduce any row with:

```bash
PERRO_BOOT_SCENE="res://scenes/demos/lights.scn" PERRO_TIMING_CSV=out.csv PERRO_EXIT_AFTER_FRAMES=800 ./target/release/perro_dev_runner.exe --path .
```

## Per-pass GPU split

`gpu_timestamp_*` columns. `mesh` is the 3D block minus the shadow bracket;
`post+ui` is the post chain, UI composite and tonemap/present tail; `other` is
whatever the scene chain encodes besides those (sky, particles, 2D, water
render passes).

| scene | main | mesh | shadow | post+ui | other |
|---|---|---|---|---|---|
| lights | 557us | 287us | 122us | 132us | 16us |
| water | 2343us | 113us | 49us | 124us | **2057us** |
| particles | 328us | 67us | 2us | 121us | 138us |

Three things this says:

- **`post+ui` is ~125us and nearly scene-independent.** It is the fixed
  full-screen tail at 1920x1080. Fusing passes out of it is the only way to
  move it; it does not shrink with scene content.
- **Water rendering dominates any frame it appears in** -- 2057us, ~4x
  everything else combined. Note this is the water RENDER passes in the scene
  chain, not the sim (`gpu_timestamp_water`, a separate bracket, is 44us).
- **`mesh` is the largest content-driven cost.** `render_scale` is the lever
  there; shadows are the lever for the shadow column.

Static scenes (`mesh_materials`, `multimesh`) measure 0us across the board --
they are fully idle-skipped, see below.

## Shadow cost is per-PASS, not per-pixel

Measured ~7-10us per shadow depth layer, **flat in atlas resolution**. Scaling
stream shadow atlases down 16x in texels bought 3%. Cutting layer COUNT is what
pays:

| sv_nested_heavy | gpu main | stream shadow layers |
|---|---|---|
| before | 999us | 72 |
| after light budget + frustum cull | 553us | 30 |

`PERRO_DISABLE_SHADOWS=1` on the same scene gives 278us, so shadows were 72% of
that frame with 36 triangles of content.

This is why multiview shadows (11 passes -> ~3) is the next big shadow lever
rather than more per-layer culling.

## `render_scale`

lights.scn, `gpu_timestamp_main`:

| render_scale | scene size | GPU | vs 1.0 | pixels |
|---|---|---|---|---|
| 1.0 | 1920x1080 | 543us | 100% | 100% |
| 0.75 | 1440x810 | 434us | 80% | 56% |
| 0.5 | 960x540 | 356us | 66% | 25% |

NOT quadratic, because the shadow passes are flat in resolution. Model that
fits: `314 fixed + 229 * scale^2`. UI rasters at surface size, so text stays
crisp at any scale.

## Idle-frame skipping

Demo3D static menu, before vs after:

| | before | after |
|---|---|---|
| gpu_acquire | 516us | 0 |
| gpu_submit | 206us | 0 |
| gpu_present | 168us | 0 |
| GPU busy | 133us | 0 |
| draw_total | 973us | 4us |

97% of frames skipped. Safety valve verified: 18/599 frames drawn, gaps
250-291ms, never longer. Negative control -- scenes with real content never
skip: lights.scn 499/499 drawn, sv_nested_heavy 499/499 drawn.

## GPU power is about wake RATE, not work volume

| scene | GPU/frame | busy % @144fps | GPU work per second |
|---|---|---|---|
| Demo3D menu | 133us | 1.9% | 19 ms/s |
| lights.scn | 737us | 10.6% | 106 ms/s |

A menu keeps the GPU 98% idle and still draws power, because a present every
6.9ms never lets it drop power state. Lowering `frame_rate_cap` 144 -> 60 cuts
GPU work per second 2.4x; idle-frame skipping takes idle frames to zero. Neither
is achievable by making the rendering itself cheaper.

## Boot

`PERRO_BOOT_LOG=1`. To first present, ~1000ms:

| phase | cost |
|---|---|
| project parse + runtime + window | ~150ms |
| wgpu instance | ~56ms |
| **adapter request** | **~378ms** |
| **surface.configure** | **~359ms** |
| msaa caps + target | ~0ms |
| PostProcessor::new | ~1ms |
| present processor | ~2ms |
| tonemap + timer + mesh arena | ~9ms |
| first present | ~11ms |

**~740ms of ~1000ms is wgpu/driver init.** Engine-side eager allocation totals
~15ms, so there is nothing meaningful left to make lazy -- post builtin
pipelines, blur/bloom scratch, shadow atlases (grow-from-1x1), mesh-blend
targets (1x1 placeholders) and LUT caches are all already lazy.

The surface is configured exactly once at boot (checked -- a second configure
would have been a free 360ms).

## Pipeline cache: measured, then removed

`load_ready`, the pipeline-warm phase after first present:

| | |
|---|---|
| first-ever launch (driver cache cold too) | +298ms |
| cold perro blob, warm driver cache | +62ms |
| warm perro blob | +47ms / +54ms |
| cache path fully disabled | +59ms / +58ms |

Disabled is indistinguishable from enabled. The 298ms was the driver compiling
for the first time, which a per-app blob cannot reclaim on later runs. Removed;
the thin `pipeline_cache` funnel remains so it can be reinstated at one site if
a driver without its own shader cache ever shows a real difference.

## Correction worth remembering

An earlier pass attributed ~370ms of startup to `PostProcessor::new`. That was
wrong by two orders of magnitude -- the boot mark was too coarse and bundled
`surface.configure`. Always split a mark before optimising what it points at.
