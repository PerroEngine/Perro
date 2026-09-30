# Renderer audit 2026-09-30

## scope

- rd `perro_graphics/src/postprocess`
- rd `perro_graphics/src/two_d`
- rd `perro_graphics/src/ui`
- rd `perro_graphics/src/gpu`
- rd 3D prep + material, light, shadow, + IBL shaders
- rd particle + water paths
- inspect `gpu_frame` + `cpu_prepare` benches

## method

- pair ref + candidate runs
- kp scene, viewport, adapter, API, warmup, + sample cfg same
- rd CPU prepare, encode, GPU main, GPU water, GPU 2D, + draw count
- use pixel readback for HDR, alpha, order, + SSAO
- reject output drift, quality cut, + noisy delta

## accepted

### post identity prune

- drop exact no-op FX b4 chain pass build
- kp `has_effects`, chain select, + HDR target rules
- use one copy pass for identity-only selected chain
- skip identity descriptors inside active merge runs
- source: `perro_graphics/src/postprocess/mod.rs`
- tests: `perro_graphics/src/postprocess/tests/mod.rs`
- readback: `perro_graphics/tests/unit/composite_gpu_tests.rs`

Final GPU pairs use RX 7800 XT, Vulkan immediate, 1280x720, 60 warmup, + 240 samples. Metric = `gpu_main_median_us`.

| case | ref pair | candidate pair | result |
| --- | ---: | ---: | --- |
| zero identity | 164.640 / 169.400 | 135.640 / 135.880 | ~19% lower |
| ordered mixed | 305.240 / 303.000 | 195.280 / 197.280 | ~35% lower |
| ordered active ref | 191.520 / 193.480 | 188.960 / 188.960 | noise |
| alpha active ref | 135.160 / 135.960 | 143.080 / 133.440 | noise |
| alpha mixed | 141.560 / 146.360 | 141.880 / 135.920 | small; no claim |

HDR, alpha, + ordered readback pass exact. Old same-SHA `post-*.csv` ABBA rows stay out of evidence.

### sparse CPU dirty hint

- kp membership + revision guards
- kp camera/resource fallback
- kp sparse animation, dense, + shadow sync paths
- invalid hint -> full rebuild
- source review: `perro_graphics/src/three_d/renderer.rs`, `perro_graphics/src/three_d/gpu/prepare.rs`, `perro_graphics/src/gpu/frame.rs`

Final ABBA CPU prep probe (`gpu_3d_us` = CPU time preparing GPU work): 10k baseline `739 / 691 us` -> candidate `127 / 128 us` (~82% lower); 1k baseline `62 / 72 us` -> candidate `16 / 14 us` (~78% lower).

GPU main median: 10k baseline `220.760 / 219.200 us` -> candidate `180.720 / 182.320 us` (~17.5% lower); 1k baseline `121.280 / 127.080 us` -> candidate `122.520 / 121.680 us` (noise). Every leg keeps `draw_calls_3d = 1`; instances = `1,000` or `10,000`. `gpu_3d_us` tracks CPU prep; `gpu_main_median_us` tracks GPU execution. No global FPS claim.

### SSAO blur center reuse

- reuse center sample in blur path
- keep edge, sky, + sample output within `2e-6` pixel tolerance
- source: `perro_graphics/src/three_d/shaders/ssao_bilateral_blur.wgsl`
- final ABBA: ref `121480 / 121360 ns` -> candidate `110160 / 110320 ns` median (~9.2% lower)
- p95: ref `122560 / 123000 ns` -> candidate `111440 / 111760 ns`
- rig: Vulkan, AMD RX 7800 XT, AMD proprietary driver 24.7.1 (LLPC)

## rejected

### depth gate

- rm trial; restore depth source to HEAD
- no-mod GPU timestamp: ref `989320 / 990520 ns` -> candidate `988840 / 989720 ns` (~0.06%; no useful gain)
- active modifier control stay stable
- no quality or default cfg change accepted

## CPU ingestion probe

Rig: i9-9900K + RX 7800 XT, driver `32.0.11029.1008`.
Metric: `criterion_time_estimate` point estimate; idle + one unit us, all unit ms.

| case | baseline pair | candidate pair | read |
| --- | ---: | ---: | --- |
| idle | 1.2895 / 1.2796 us | 1.2799 / 1.2726 us | same |
| one | 1.7040 / 1.7319 us | 1.7408 / 1.7523 us | small overhead |
| all | 4.4617 / 5.2214 ms | 4.4559 / 4.5197 ms | noisy |

## material + shadow controls

- material: 6 shader cases; plain baseline `126.080 / 141.800 us` -> final `144.360 / 141.440 us`; clock/load drift -> no speed claim
- shadow moving lights: baseline `160.240 / 186.600 us` -> final `183.760 / 185.800 us`
- shadow batches 256: baseline `198.480 / 230.120 us` -> final `234.320 / 234.960 us`
- shadow moving caster: baseline `256.560 / 257.400 us` -> final `255.720 / 250.600 us`
- shadow counts stay: 8 layers + 2 draws; 1 layer + 256 draws; 7 layers + 2 draws
- raw 6 + 6 control cases: `docs/project/renderer_audit_2026-09-30.csv` (`gpu_main_median_us`, CPU-prep `gpu_3d_us`, `gpu_shadow_us`, + shadow counts)
- broad clock/load drift -> controls inconclusive
- no shipped-game FPS claim

## preexisting correctness work

- transform-only move skips alpha batch resort (`three_d/gpu/prepare.rs:712-720`, `776+`)
- transform-only move skips baked LOD band recheck (`three_d/gpu/prepare.rs:712-720`)
- stream target recreate clears 3D bind while `DIRTY_STREAMS` may skip main 3D prepare (`gpu/textures.rs:449-450`, `gpu/frame.rs:139-149`, `416-440`)
- add paired alpha, LOD, + stream resize checks b4 fx

## defers

- UI partial replay tile -> mesh bins (`ui/gpu.rs:1179`)
- UI full-change tessellation path
- sprite vertex texture-dimension lookup
- point particle CPU eval + trig loop
- water coast storage reads + idle trig gate
- water voxel fixed ray-step count

No measured proof -> no change.

## validation gates

- `cargo test --workspace --exclude perro_graphics -- --test-threads=1` -> `2263 pass / 58 ignored`
- `cargo test -p perro_graphics -- --test-threads=1` -> `619 pass / 2 ignored`
- total: `2882 pass`
- `cargo clippy --workspace --all-targets -- -D warnings` -> pass
- `cargo fmt --all -- --check` -> pass
- initial graphics fixture fail -> init fix -> focused + full suite pass

## artifacts + hashes

- source HEAD: `e2387e9dd00ef9f168ae61093c40aea087471c70`
- original `gpu_frame` baseline: `8FC96B00F0430A6682A989712565D27FC5D3ED6F94859DC5E7EC6CD47C6B2787`
- isolated post baseline: `E271698A855D0584601F454996E84FB8B812E8483E6B07320201FABB37638196`
- prior fresh post candidate: `6456AC7D30B28F9D25C5CAA7B1B7E460EAD91DE978135678F32AB3F63D91FEB8`
- final `gpu_frame`: `61196519ABE9817D1672ABC29B4A78EAAF45DE6ECD6C24412584B6488A080618`
- final `cpu_prepare`: `C23D995CCBC50F0BE544E492844A14C863070D29F6AA74F853F4ED2078509163`
- old `cpu_prepare` baseline: `6E63A2E09A62F03E23E3B6B5579F238FE053866CE79418ECEBD459E52D306BE6`
- final post rows: `target/renderer-audit-2026-09-30/post-final-*.csv`
- final sparse rows: `target/renderer-audit-2026-09-30/sparse-final-*.csv`
- final SSAO rows: `target/renderer-audit-2026-09-30/ssao-final-timestamps.log`

## ranked shader probes

1. per-frag light GGX + 4/9-tap shadow PCF
   - source: `perro_graphics/src/three_d/shaders/prelude_3d.wgsl:580-715`, `898-951`
   - probe local-light cull + caller light-direction reuse
   - pair lit, shadow, edge, + light-count output
   - risk: shadow edge drift + light response drift
2. packed IBL loads
   - source: `perro_graphics/src/three_d/shaders/shared_3d.wgsl:403-462`, `prelude_3d.wgsl:980-1030`
   - try ~4 irradiance + 8 spec + 4 BRDF loads
   - test filtered textures w/ roughness + edge parity
   - risk: roughness edge + reflection drift
3. derivative tangent normal-map path
   - source: `perro_graphics/src/three_d/shaders/prelude_3d.wgsl:312-342`, `872`
   - probe vertex tangent path
   - risk: mesh bandwidth + per-pixel quality

Current specialization already rm unused maps, shadows, + modifiers. No variant add w/o paired proof.

## validity + replay

- use same adapter, viewport, API, warmup, + sample cfg for each GPU pair
- use fresh candidate + ref hashes for post pairs
- use exact readback for HDR, alpha, + order
- use SSAO pixel tolerance `2e-6`
- sparse hint reject stale rev, bad membership, bad node index, camera/LOD/resource dirt, structural edit, + dense/animation mismatch
- keep quality + default cfg same
- report GPU timestamp + prepare deltas only; no global FPS claim

Replay cmds:

```text
$env:WGPU_BACKEND = "vulkan"
$env:PERRO_PRESENT_MODE = "immediate"
$env:PERRO_GPU_WARMUP_FRAMES = "60"
$env:PERRO_GPU_SAMPLE_FRAMES = "240"
$env:PERRO_GPU_BENCH = "post_identity"
$env:PERRO_GPU_BENCH_CSV = "target/renderer-audit-2026-09-30/replay-post.csv"
cargo bench -p perro_graphics --bench gpu_frame

$env:PERRO_GPU_BENCH = "audit_sparse_transform"
$env:PERRO_GPU_BENCH_CSV = "target/renderer-audit-2026-09-30/replay-sparse.csv"
cargo bench -p perro_graphics --bench gpu_frame

cargo test -p perro_graphics ssao_blur_timestamp_probe_abba --lib -- --ignored --test-threads=1 --nocapture
cargo test -p perro_graphics shader_equivalence_tests --lib -- --nocapture
cargo test -p perro_graphics sparse_hint --lib -- --nocapture
cargo test -p perro_graphics --lib
cargo bench -p perro_graphics --bench cpu_prepare
```

Evidence files: `target/renderer-audit-2026-09-30/`; [raw audit CSV](renderer_audit_2026-09-30.csv).
