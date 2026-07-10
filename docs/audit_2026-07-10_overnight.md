# Overnight Audit + Fix Pass — 2026-07-10

Four parallel audits (camera stream/webcam perf, API ergonomics, realistic-game
feature gaps, hot-path sweep) followed by three implementation streams, all
merged to main. Full audit reports were generated per-area; this doc is the
durable summary + roadmap.

## Merged work

### 1. Camera stream / webcam pipeline (merge `75334bb0`)

Traced one webcam frame capture -> screen: the pipeline did 3 redundant
full-frame CPU copies, kept a duplicate resident buffer, rebuilt the GPU
texture + mip chain + sampler + bind group every frame, and — worst — every
frame's `WriteTextureRgba` emitted `TextureLoaded`, forcing a **full 2D+3D
scene rescan plus resource-ref recount per webcam frame**.

Landed:
- New `RenderEvent::TextureTexelsUpdated`: repeat same-resolution stream
  writes no longer trigger scene rescans or ref recounts (first load /
  resolution change keeps the full `TextureLoaded` path).
- Persistent GPU stream textures: repeat writes are one in-place
  `queue.write_texture` (single level, non-mip sampler); recreation only on
  resolution change. Covers 2D sprite, UI image, 3D material, custom material
  source slots (with a mip-guard fallback).
- Copy elimination: decoded frame moves owned end-to-end (`StreamRgba`
  Owned/Shared payload), `copy_from_slice` into the resident buffer when
  same-size, `by_source` duplicate dropped for streams.
- Latest-only handoff: bounded `sync_channel(8)` capture channel dropping
  stale frames + newest-per-id coalescing before upload.
- Webcam-source `camera_stream_state` short-circuits the O(all-nodes)
  scratch fill; `mirror_rgba_rows` swaps whole pixels.

Result: steady-state webcam frame = one decode + one GPU write. No scene
rescan, no GPU object churn, ~0 redundant copies.

### 2. Hot-path extraction optimizations (merge `17ef62c9`)

- `effective_self_modulate`: per-call `Vec` + reverse walk -> zero-alloc
  upward fold (bench: -23% at depth 16, width 1024).
- Skeleton->mesh reverse index: dirty skeletons no longer scan the whole
  arena per animating frame (bench: -18% extraction at 16k nodes; win scales
  with node count).
- Gated `rebuild_mesh_blend_receivers`: transform-only frames skip the
  O(sources x batches) rebuild unless a blend-relevant sphere moved.
- Hoisted loop-invariant overlay camera clone + viewport query; empty-list
  guard in `run_internal_update_schedule`; recycled mesh-surface scratch
  (no per-frame `surfaces.clone()` for moving meshes).
- New benches: `modulate_chain`, `skinned_extraction`.

### 3. Script API ergonomics (merge `64907b93`, additive only)

New surface: `find_node!` (index-backed), `descendants!`, `set_tree_visible!`,
`broadcast_var!`, `get_node_var!`, `signal_connect_pairs!`, `spawn!`
(create+configure), `Color::with_alpha()` const + `color!` hex macro,
`Variant::as_node_or_nil()`, `Transform3D::forward/right/up()` + `look_at_3d!`.
Demos refit as proof: net -249 LOC; freecam now moves along facing. Docs
updated in `docs/scripting/`.

## Feature-gap roadmap (audit only — not implemented)

Graded against "performant realistic 3D games". Top-10 by value/effort:

1. Normal mapping + ORM textures — HIGH/M — glTF normal maps currently
   ignored; foundational for everything below.
2. Image-based lighting (prefiltered env + irradiance + BRDF LUT) — HIGH/L —
   replaces the procedural-sky ambient fake.
3. HDR completion (dedicated tonemap pass, auto-exposure, scene-referred
   bloom) — HIGH/M — Rgba16Float target exists but ends in inline ACES +
   fixed exposure.
4. SSAO/GTAO — HIGH/M — depth prepass already exists.
5. Volumetric/height fog — HIGH/M.
6. Asset/texture streaming + residency budgets — HIGH/L — required for
   large worlds.
7. Navigation maturity (geometry auto-bake, dynamic obstacles, funnel
   smoothing) — HIGH/L — current bake only embeds authored .pnav.
8. Terrain system — HIGH/XL.
9. SSR — MED-HIGH/L — after 2/3/4.
10. Vegetation tooling + impostors — MED/L.

Notes: `perro_meshlets` is auto-LOD generation, not cluster rendering; GPU
path is frustum + hi-z occlusion cull. Audio/UI/animation/profiling graded
strong. TAA/FSR needs motion vectors first.

## Known deferred / follow-ups

- Extraction: per-node ~15 redundant `nodes.get` type-dispatch (audit F3);
  camera-stream scene-source extraction walks scratch ~4x per stream (F5);
  `active_render_camera_3d` still full-arena-scans per frame.
- Camera stream: hidden/off-screen streams still capture + upload.
- API: state-field access sugar, timers/tweens/deferred-calls module (biggest
  absolute ergonomics gap), silent-miss debug warnings on set_var/call_method,
  raycast_3d filter unification, naming-alias dedupe, webcam/video API
  consistency (`frame_changed` flag, auto-update parity).
- Housekeeping: 16 merged `Perro_codex_*` worktrees + `perf-audit-0709`
  worktree are clean+merged (~100G+); `demos/Demo3D/target` build cache was
  79G. Disk hit 100% during this pass.

## Verification

- `cargo test --workspace` on merged main (`64907b93`): all green.
- Demo3D release run (`perro_cli dev --timings --release`), idle main menu,
  370-sample steady window: avg sim 65us / gfx 310us / present_wait 603us /
  delta 1015us = **~1121 fps** (pre-pass baseline same scene/conditions:
  ~702 fps). 400 samples crash-free; the pre-existing 0xC0000005 menu crash
  (observed twice before commit `60c45ca0`) did not reproduce.
- Hot-path microbenches (noisy shared machine, back-to-back deltas):
  modulate depth-16 -23%; skinned extraction -7%/-14%/-18% at 512/4k/16k
  nodes.
- Demo script builds verified for Demo3D, Demo2D, DemoUI.
