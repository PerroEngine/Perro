# Perf Audit 2026-07-08

## Base

- commit user work: `f06dadd3 Save current workspace changes`
- chk tree clean b4 audit
- run `cargo test`
- res: pass, 188s
- run broad bench try
- res: too slow + disk full
- run `cargo clean`
- res: rm 13.5GiB, free 26.15GB

## Bench Done

short Criterion run: `--sample-size 10 --warm-up-time 0.5 --measurement-time 1`

- `perro_structs/bitmask_query_ops`: ~51.9us
- `perro_structs/bitmask_build_from_layers`: ~110.0us
- `perro_structs/bitmask_push_pop_layers`: ~94.8us
- `perro_structs/bitmask_without_layers`: ~110.3us
- `perro_structs/vector2_bulk_ops`: ~104.0us
- `perro_structs/vector3_bulk_ops`: ~193.6us
- `perro_structs/quaternion_bulk_ops`: ~985.4us
- `perro_structs/quaternion_lerp/slerp`: ~798.5us
- `perro_structs/quaternion_lerp/nlerp`: ~238.8us
- `perro_structs/transform2d_mat_roundtrip`: ~584.5us
- `perro_structs/transform3d_mat_roundtrip`: ~623.3us
- `perro_structs/matrix4_bulk_ops`: ~413.0us
- `perro_structs/matrix20x20_mul`: ~467.3us
- `perro_structs/matrix25x15_mul_15x25`: ~571.0us
- `perro_structs/matrix15x25_mul_25x15`: ~698.6us
- `perro_structs/neighbors_8_api`: ~1.607ms
- `perro_structs/neighbors_8_unchecked`: ~2.305ms
- `perro_structs/count_neighbors_4_api`: ~447.2us
- `perro_structs/count_neighbors_4_unchecked`: ~391.7us
- `perro_structs/count_neighbors_8_api`: ~975.3us
- `perro_structs/count_neighbors_8_unchecked`: ~1.908ms
- `huge_csv_primary_find`: ~5.74us
- `huge_csv_primary_hash_find`: ~3.44us
- `huge_csv_header_get`: ~2.86us
- `huge_csv_query_filter_sort_limit`: ~1.67ms

## Bench Gaps

- full workspace bench fails: lib harness rejects Criterion args
- runtime benches need `--features bench`
- release bench link fills disk on Windows PDB
- graphics bench hits `LNK1318` / `LNK1108`
- fix bench flow b4 next full run:
  - use per-target bench only
  - use `CARGO_PROFILE_BENCH_DEBUG=0`
  - use `CARGO_PROFILE_BENCH_INCREMENTAL=false`
  - use `cargo bench -j 1`
  - run target groups, clean between groups if disk < 20GB

## Top Opt List

### 1. Bench Harness

- add `xtask bench-all` or `cargo run -p perro_cli -- bench-workspace`
- list bench targets frm metadata
- add feature map (`perro_runtime` => `bench`)
- pass Criterion args only to bench bins
- set low-debug env on Windows
- write one csv/json summary
- res: full bench repeat w/o manual loop

### 2. Matrix Neighbor Paths

- file: `perro_source/core/perro_structs/src/structs/matrix/mod.rs`
- issue: unchecked 8-neighbor slower than safe api
- data: api ~975us, unchecked ~1.908ms
- chg: inspect branch shape + closure call count
- add bench case 4 direct index math
- target: keep 4-neighbor unchecked win, fix 8-neighbor regress

### 3. CSV Query Sort

- file: `perro_source/core/perro_csv/src/lib.rs`
- issue: `huge_csv_query_filter_sort_limit` ~1.67ms
- chg: partial select for `limit` instead of full sort
- chg: reuse query scratch Vec
- chg: cache header col index path
- target: cut sort-heavy query

### 4. Runtime Resource Locks

- file: `perro_source/runtime_project/perro_runtime/src/rs_ctx/core.rs`
- issue: many `Mutex` fields in script-facing API
- chg: batch frame snapshots for listener/options/viewport/csv/skeleton/webcam
- chg: avoid lock per script call where main thread owns data
- target: lower script tick contention

### 5. Asset IO Global Locks

- file: `perro_source/io_stack/perro_io/src/asset_io.rs`
- issue: `RwLock<HashMap<String,...>>` + path string hash on load/read
- chg: intern DLC names + res paths to `u64`
- chg: snapshot mounted archives at frame/load boundary
- target: faster asset resolve + less lock scope

### 6. Render GPU Writes

- files: `perro_source/render_stack/perro_graphics/src/three_d/gpu/*`
- issue: many `queue.write_buffer` call sites
- chg: add write counter bench/assert
- chg: dirty-range pack for particles/material/camera paths
- chg: coalesce small writes into staging slabs
- target: lower CPU submit cost

### 7. Particle Buffers

- files: `perro_source/render_stack/perro_graphics/src/three_d/particles/gpu/*`
- issue: many buffer grow/write sites
- chg: cap growth policy + no shrink in hot run
- chg: track dirty slots in bitset
- target: less buffer churn + less write traffic

### 8. Meshlet LOD Build

- file: `perro_source/render_stack/perro_meshlets/src/lib.rs`
- issue: clone current LOD, full sort, alloc per ratio
- chg: reuse buffers
- chg: avoid `current.clone()` for each LOD
- chg: profile `sort_unstable_by_key`
- target: faster static build pipeline

### 9. Static Pipeline Compile Diet

- files: `perro_source/build_pipeline/perro_static_pipeline/*`
- issue: many rayon modules + heavy deps in workspace
- chg: feature-gate heavy asset kinds
- chg: split website build deps frm default build where possible
- target: cut bench/test compile time

### 10. Dup Deps

- cmd: `cargo tree -d --workspace`
- issue: dup families still high
- left: `base64`, `bitflags`, `hashbrown`, `toml`, `windows`, `thiserror`, `itertools`, `getrandom`, `convert_case`
- chg: bump upstream deps where direct
- chg: gate website/leptos deps outside default bench/test when possible
- target: less compile time + bin size

### 11. Audio/Mic Locks

- files: `perro_source/audio_stack/perro_pawdio/src/{player,controller,mic}.rs`
- issue: `Arc<Mutex<Vec<i16>>>` sample buffer + controller locks
- chg: ring buffer for mic samples
- chg: lock-free or single-lock batch drain
- target: lower callback risk + less mem move

### 12. Web Demo Artifacts

- files: `perro_website/public/demos/*`
- issue: cargo test/build touches wasm/js artifacts
- chg: stop tests/build scripts from rewriting checked-in demo bundles unless explicit
- target: keep tree clean after test/bench

## Next Order

1. fix bench harness
2. rerun runtime bench group w/ feature map
3. fix matrix 8-neighbor unchecked path
4. optimize CSV limit sort
5. audit render write counts
6. lock batch in `rs_ctx`

