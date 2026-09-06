# Runtime repeat-work audit

Track this audit's initial findings below.
See [stage 2 implementation + measurements](../project/performance_stage2_2026-09-06.md)
for shared suspension memos, narrower invalidation, cached internal schedules,
direct handler dispatch, and signal teardown indexing.
Keep boxed 3D command transport as an open measured target.

## Trace frame flow

- Dispatch script callbacks -> mutate nodes + dirty flags.
- Propagate dirty transforms -> refresh cached global transforms.
- Extract changed render state -> compare retained state -> queue commands.
- Drain commands -> renderer retained draws -> GPU prep + uploads + passes.
- Keep acquire/present waits separate from CPU/GPU work.

## Rank follow-up work

1. Measure shared sub-view suspension checks.
   - Read `runtime/world_state.rs::is_suspended_by_sub_view`.
   - Repeat enclosing-world walk per script, internal node + physics query.
   - Skip whole path when arena has no sub-views.
   - Keep visibility memo: `is_effectively_visible` already caches ancestor visibility by arena mutation revision.
   - Keep world-owner cache: rebuild only on structural revision changes.
   - Consider per-world suspension memo only after real-frame samples show material cost.
   - Invalidate on visibility/enabled/suspend flag writes, reparent, remove + slot reuse; respect callback changes within same dispatch.
   - Expect broad mutation revision to limit cache reuse when each script writes nodes.
2. Measure 3D command allocation + transport.
   - Read `runtime/render/three_d/extract.rs` + `perro_render_bridge/src/commands.rs`.
   - Allocate `Box<Command3D>` per emitted 3D command; drop box after consumption.
   - Keep unchanged-draw gate: retained-state comparison skips command emission.
   - Keep dirty-extraction gate: idle scenes skip extraction work.
   - Keep enum size guard: `RenderCommand` stride <=128 bytes; naive unboxing grows all queue slots.
   - Compare arena/batch transport against existing transport only with measured allocation + queue-copy cost; no blanket unbox change.
3. Measure internal schedule snapshots under stable membership.
   - Read `runtime/internal_updates.rs::snapshot_dispatch`.
   - Copy all scheduled NodeIDs into reused scratch per internal update/fixed-update pass.
   - Keep snapshot semantics: callbacks can mutate live schedule; existing shrink/order test covers this need.
   - Consider revision-gated snapshot reuse; require all register/unregister paths to bump revision + tests for callback add/remove.
   - Distinguish script dispatch: `runtime/scheduling.rs` walks live script slots directly; no blanket per-frame script snapshot clone.

## Run focused CPU benches

```powershell
cargo bench -p perro_runtime --features bench --bench runtime_core_hotpaths -- sub_view_suspension --warm-up-time 1 --measurement-time 2 --noplot
cargo bench -p perro_runtime --features bench --bench runtime_core_hotpaths -- render_bridge_3d --warm-up-time 1 --measurement-time 2 --noplot
```

- Use actual suspension predicate for 1k + 10k distinct nodes under 0/1/4/16 visible nested sub-views.
- Warm world-membership + visibility caches before samples.
- Exclude node/scene construction + callback cost; measure repeated predicate scans only.
- Use actual runtime queue/drain/drop path for 1k + 10k 3D removal commands.
- Exclude scene-node removal + renderer processing; include box alloc/drop + normal drain overhead.
- Reuse command buffers; warm resource scan before samples.
- Treat removal stream as allocation/transport baseline; not mesh-extraction or GPU benchmark.
- Run timing alone after compilation; repeat runs for stability.

## Record baseline: 2026-09-06

- Use Windows + Intel Core i9-9900K @3.60GHz.
- Use workspace bench profile: opt-level 3, LTO off, 64 codegen units.
- Run compiled bench executable twice; use 10 samples/case, 1s warmup + 2s measurement.
- Pause team builds/tests/other benches during timing.
- Record Criterion central time estimates below; no code changes between runs.

| Scan / command stream | Count | Run 1 | Run 2 |
| --- | ---: | ---: | ---: |
| Suspension, no sub-views | 1,000 | 2.398 us | 2.394 us |
| Suspension, depth 1 | 1,000 | 19.09 us | 19.19 us |
| Suspension, depth 4 | 1,000 | 56.71 us | 57.35 us |
| Suspension, depth 16 | 1,000 | 214.5 us | 214.6 us |
| Suspension, no sub-views | 10,000 | 23.88 us | 23.93 us |
| Suspension, depth 1 | 10,000 | 218.1 us | 226.7 us |
| Suspension, depth 4 | 10,000 | 611.1 us | 597.2 us |
| Suspension, depth 16 | 10,000 | 2.221 ms | 2.192 ms |
| 3D queue + drain + drop | 1,000 | 71.80 us | 62.50 us |
| 3D queue + drain + drop | 10,000 | 751.3 us | 816.9 us |

- Infer depth-sensitive repeat cost from suspension scans; confirm no-sub-view fast path stays cheap.
- Treat depth 16 as stress case, not typical scene claim.
- Note bridge variance: 10k 95% intervals 733-799 us + 758-935 us; need longer samples + allocator counters before transport decisions.
- Ignore Criterion run-to-run speed/regression labels here: same binary both runs.
- Pass `cargo bench -p perro_runtime --features bench --bench runtime_core_hotpaths --no-run` + both focused runs.
- Pass `rustfmt --check` for modified Rust files + `git diff --check`.

## Bound claims

- Keep runtime behavior + script APIs unchanged; add `bench` feature helper only.
- Claim no runtime speed gain from instrumentation.
- Treat rank as code-based investigation priority, not measured whole-frame bottleneck order.
- Add no suspension cache or transport redesign without representative timing + invalidation proof.
