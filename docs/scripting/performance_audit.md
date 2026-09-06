# Script + Variant perf audit

Audit date: 2026-09-06.

Baseline commit: `0d384c0b`; host: Windows x64; Rust: `1.97.1`.

| Path | Cost + result | Action |
| --- | --- | --- |
| `with_state!` / `with_state_mut!` | Dense slot + full ID check + cached `TypeId`; no codec or heap alloc | Kp closure borrow; fx active-slot miss aft peer swap-remove |
| `with_node!` | Arena lookup + exact type dispatch; no node clone | Kp typed read path |
| `with_node_mut!` | Transform/base snapshots + UI payload hash + dirty hooks | Kp hooks; group related writes in one closure |
| `params!` | Borrow stack arr of **owned** `Variant` vals | Kp macro; arg-list heap alloc = 0; text/container vals may still alloc |
| `variant!` | `Variant::from(value)`; move owned vals | Move unique `Arc<T>` / `Rc<T>` payload; clone only w/ shared strong owners |
| `Variant` | Inline scalar/ID; shared text/bytes; owned arr/object; box large engine structs | Kp layout + <=32-byte stride guard; avoid deep container clone on hot paths |
| `#[derive(Variant)]` / `DeriveVariant` | Default array struct + u16 enum tag; owned codec + static object keys | Kp wire forms; rm scratch heap alloc for small fixed-arr dcod |
| `get_var!` | Const-ID match for known fields/nested paths; owned result | Skip leaf fields in dynamic fallback; add safe erased-state type guard |
| `set_var!` | Const-ID match + typed dcod; object patch fallback | Use owned dcod for non-object vals; kp object patch behavior |
| `call_method!` | Instance lookup + `Rc` handle + callback frame + trait call + const-ID match | Kp dispatch; reuse callback context; avoid per-call text-name hash via `method!` / `func!` |
| Gen member tables | Rust `match` on const hashes; no runtime map build | Kp; no evidence for map replacement |
| Gen script registry | Static constructor slice + binary search; dynamic constructor map | Kp; attach/load path, separate from member dispatch |
| Nested dynamic fallback | Encode root + walk fields + hash full paths | Reuse one path buffer; skip scalar/empty roots; kp full-path hash format |
| JSON / scene codec | Tree conversion + resource resolver work | Kp at data boundaries; avoid in per-frame typed state access |

Fix active-state access after peer removal: old dense slot may point elsewhere or fall out of range. Retry full generational ID only on slot mismatch. Kp exact state-type check + stale-ID rejection. Kp scheduler snapshot validation separate.

Fix gen `get_var` / `set_var` / scene-state casts: public safe trait accepts `dyn Any`; old unchecked cast permits wrong-type refs. Add checked downcast -> panic on mismatch. Cost: one `Any` type check at erased-state boundary; no extra check in cached `with_state*` path.

Kp strict numeric arg types, missing-arg defaults, unknown-member result, object patch fallback, scene resource resolution, visibility rules + node dirty hooks.

Measure via `variant_hotpaths` Criterion bench: 1 s warmup, 2 s total measurement, 30 samples; same host + bench profile (`opt-level = 3`). Use 3 s measurement for final large-arr comparison. Isolate encode setup via `iter_batched`; no whole-frame/FPS claim. Report time estimates below; round relative gains.

| Case | Before | After | Time cut |
| --- | ---: | ---: | ---: |
| Fixed `[i32; 3]` dcod | 59.0 ns | 18.35 ns | ~69% |
| Fixed `[i32; 16]` dcod | 76.9 ns | 61.76 ns | ~20% |
| Fixed `[i32; 128]` dcod | 330 ns | 335.5 ns | No gain claim; see reference below |
| Unique `Arc<Variant>` encode, 128 vals | 1.112 us | 70.8 ns | ~94% |
| Unique `Rc<Variant>` encode, 128 vals | 1.134 us | 57.5 ns | ~95% |
| Shared `Arc<Variant>` encode, 128 vals | 1.023 us | 1.027 us | No significant change |
| Nested set, last of 64 fields | 12.71 us | 2.021 us | ~84% |
| `params![i32, f32, bool]` | 7.90 ns | 7.86 ns | No significant change |

Reject full stack-array dcod: 128-item probe regresses ~22%. Cap stack path at 16 vals + 4 KiB of `Option<T>` scratch; kp heap path above cap. Treat cap as measured policy for this host, not universal optimum.

Restore original large-arr body after shared-helper regression. Recheck original body beside final code: 337.0 ns reference vs 335.5 ns final; overlapping confidence intervals. Kp reference case in bench for future host-drift checks.

Kp claims scoped to measured cases. No end-to-end `call_method`, frame-time or node-speed claim. Gen var type guard adds one check; net get/set dispatch cost remains unmeasured.

Run baseline before code edits:

```text
cargo bench -p perro_scripting --bench variant_hotpaths -- --warm-up-time 1 --measurement-time 2 --sample-size 30 --save-baseline audit_before
```

Run comparison after code edits:

```text
cargo bench -p perro_scripting --bench variant_hotpaths -- 'variant_hotpaths/(array_decode_[0-9]+$|unique_|shared_|nested_|params_)' --warm-up-time 1 --measurement-time 2 --sample-size 30 --baseline audit_before
```

Run final large-arr + old-body reference together:

```text
cargo bench -p perro_scripting --bench variant_hotpaths -- array_decode_128 --warm-up-time 1 --measurement-time 3 --sample-size 30
```

Prioritize next gains only w/ workload data:

1. Add typed nested-field access metadata for imported/dynamic schemas -> skip full-root encode + walk. Kp unknown-key + patch semantics.
2. Add owned scene dcod contract -> `into_parse_scene` still calls borrowed `from_scene_variant`, despite owned input.
3. Add borrowed typed method args if container-heavy dispatch dominates -> current `&[Variant]` requires clone for owned `Variant` / `String` params. Use raw `&[Variant]` or `&str` methods where suitable today.
4. Split type validation from `is_type::<T>()` dcod -> current probe may alloc full result + drop it. Use `kind` / exact accessors for cheap kind checks.
5. Bench heapify for `BinaryHeap` dcod + static keys in hand-written engine codecs -> current push-per-item heap build + repeated key alloc.

Kp API/wire layout in this patch. Defer shared-array/object or borrowed-Variant redesign: public enum ownership + mutation + serialization contracts need separate design + workload proof.

Verify: 946 unit tests + 24 integration tests pass; include borrow compile-pass/fail fixtures + derive roundtrips. Pass generated all-type compile + 4 generated-glue runtime tests. Pass Clippy for Variant, scripting + compiler. Pass diff whitespace check.

Trace sources:

- [State store](../../perro_source/runtime_project/perro_runtime/src/cns/script_collection.rs) + [runtime dispatch](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/scripts.rs)
- [Node access](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/node_api.rs) + [dirty snapshots](../../perro_source/runtime_project/perro_runtime/src/rt_ctx/nodes/helpers.rs)
- [Variant macros](../../perro_source/core/perro_variant/src/macros.rs) + [codec suite](../../perro_source/core/perro_variant/src/variant/derive.rs)
- [Derive macros](../../perro_source/script_stack/perro_scripting_macros/src/lib.rs)
- [Gen var glue](../../perro_source/build_pipeline/perro_compiler/src/script_methods.rs) + [method dispatch](../../perro_source/build_pipeline/perro_compiler/src/script_fields.rs)
- [Nested path walk](../../perro_source/script_stack/perro_scripting/src/nested_vars.rs) + [bench](../../perro_source/script_stack/perro_scripting/benches/variant_hotpaths.rs)
