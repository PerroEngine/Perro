# Perro Source Perf Audit - 2026-07-14

## Scope

- scan `perro_source/**`
- 41 crates
- 443 Rust fles
- 277,685 Rust LOC
- 39 cfg Criterion bench targets
- include 17 `perro_runtime --features bench` targets

## Chk Set

- run `cargo clippy --all-targets --all-features -- -W clippy::perf`
- run extra map/clone/collect/alloc lints
- scan front-remove, front-insert, loop clone, temp collect, dup map lookup
- build all default bench targets
- build all feature-gated runtime bench targets
- run focused before/aft Criterion cmp
- run `cargo test` across all 41 default workspace crates

## Crate Ledger

`clean` = no actionable hit aft lint + source review.

| Crate | Benches | Res |
| --- | ---: | --- |
| perro_animation | 0 | fx dup trim eval |
| perro_api | 0 | clean |
| perro_app | 0 | fx dup Joy-Con serial clone |
| perro_asset_formats | 0 | clean |
| perro_assets | 0 | clean |
| perro_builtin_meshes | 0 | clean |
| perro_cli | 0 | clean |
| perro_compiler | 0 | fx 2 dead path clones |
| perro_csv | 1 | clean + bench build |
| perro_dev_runner | 0 | clean |
| perro_graphics | 8 | fx frm clone + add sparse rect bench |
| perro_graphics_assets | 0 | clean |
| perro_headless | 0 | clean |
| perro_ids | 0 | clean |
| perro_input_api | 0 | clean |
| perro_internal_updates | 0 | fx pose clone |
| perro_io | 2 | fx archive lock scope + bench build |
| perro_jobs | 1 | clean + bench build |
| perro_macros | 0 | clean |
| perro_meshlets | 1 | clean + bench build |
| perro_modules | 0 | clean |
| perro_networking | 0 | clean |
| perro_nodes | 0 | clean |
| perro_particle_math | 0 | clean |
| perro_pawdio | 2 | fx dry DSP alloc + PCM clone |
| perro_physics | 3 | clean + bench build |
| perro_project | 0 | fx eager err alloc |
| perro_render_bridge | 0 | clean |
| perro_resource_api | 0 | clean |
| perro_runtime | 17 | fx query alloc + frm clones |
| perro_runtime_api | 1 | clean + bench build |
| perro_runtime_render | 0 | clean |
| perro_scene | 0 | clean |
| perro_scripting | 1 | clean + bench build |
| perro_scripting_macros | 0 | clean |
| perro_static_pipeline | 0 | fx dead str clone |
| perro_steamworks | 0 | clean |
| perro_structs | 2 | fx dup trim eval + bench build |
| perro_ui | 0 | clean |
| perro_variant | 0 | fx eager parse err alloc |
| perro_web | 0 | clean |

## Accepted Fx

### Query Intersect

- rm temp `Vec<&Vec<NodeID>>`
- build mark bitsets straight frm input iter
- keep 1-seed clone fast path
- bench: `query/compile_repr/selective_vec/100000`
- b4: `180.00 us`
- aft: `161.60 us`
- chg: `-8.50%`
- 95% CI: `-10.13%..-6.51%`
- `p = 0.00`

### Dry DSP Mem

- b4: alloc echo + 2 reverb delay buf/source
- aft: alloc only for wet-at-create or 1st wet transition
- 48 kHz stereo dry source: rm 28,608 `f32`
- mem cut: 114,432 B/source (~111.75 KiB)
- add dry/wet alloc state test

### Archive Lock Scope

- b4: hold global `RwLock` thru archive copy + zlib decode
- aft: clone archive `Arc` under lock -> read/decode w/o lock
- apply prj archive + DLC archive load/stream paths

### Clone + Eager Work Sweep

- rm 9 prod redundant clones
- lazy-build config/variant err vals
- rm dup trim + static scene str work

## Bench-Reject Set

### Sparse Rect Range Merge

- add `graphics_2d_rect_sparse_updates/10000_of_100000`
- test `Vec::remove(0)` => iter-first rewrite
- res: `+3.64%`, Criterion noise band
- action: rm prod rewrite
- keep bench + merge behavior test

### Active 3D Camera Scan

- test arena scan => activated-camera index
- 16,384 nodes: no sig chg
- 65,536 nodes: `-0.66%`, `p = 0.86`
- action: rm index rewrite + extra state

## Gates

- pass full `cargo test`
- pass 41 default crates
- pass all configured bench target build
- pass all 17 runtime feature bench target build
- pass focused query/camera/rect/DSP/IO tests
- clippy perf group: 0 perf warn
- extra prod scan: no map-entry/manual-copy/redundant-clone hit

## Limits

- no GPU timing capture
- no live audio device bench
- no cross-OS run
- no full Criterion run of every case
- hardware-only bench paths build-only
