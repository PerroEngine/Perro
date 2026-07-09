# Perro Codebase Audit — 2026-07-09

Future-agent handoff for bug fixes, API cleanup, docs, and verification.

## Snapshot

- Scope: all workspace areas; 853 Rust/Markdown/manifest files indexed.
- Method: three parallel source audits, prior-audit reconciliation, unsafe/panic/docs scans, and workspace build.
- Baseline command: `cargo check --workspace --all-targets`.
- Baseline result: all engine/devtool crates reached `Checking` with no diagnostics on Windows x86_64; the final website target did not finish before handoff because parallel audit commands contended for the shared Cargo target lock. Re-run the command for a definitive full-workspace result.
- Confidence: high for listed findings; this is a broad static audit, not proof that no other defects exist.
- Prior work: read `docs/project/engine_audit_2026-07-06.md`, `docs/audit_spec_0707.md`, and `docs/audit_progress_0707.md` before changing old backlog items. Much of the 2026-07-07 audit already landed.

## Working-tree guard

The audit ran with existing uncommitted work, including webcam/video changes across runtime, graphics, nodes, resource API, demos, and `Cargo.lock`. These changes belong to the user. Do not reset or overwrite them. Re-run `git status --short` before every fix lane.

## Execution update — 2026-07-09

Work ran in isolated `codex/audit-*` worktrees. The combined branch is `codex/audit-integration` at `D:\Rust\Perro_codex_audit_integration`; main remains untouched because it contains user/external-agent work.

Landed on the integration branch:

- DLC names/path containment: `dee0c040`.
- DLC remount backing replacement: `92207b77`.
- Scene lexer errors/iterative skipping: `c3d73ae4`.
- `Transform3D::looking_at`: `83463cd5`.
- Bounded/stale-recoverable script write locks: `6aed3d94`.
- Scoped static-pipeline override guard: `731fde77`.
- Tileset finite/positive geometry validation: `7e55f794`.
- `MicClip` invariants: `dae2bae6`.
- Runtime texture allocation budget: `cf40b948`.
- Unsafe DLC callback boundary/length cap: `7be6345b`.
- Docs map/link fixes: `7b045504`.
- Published-tool package metadata: `c059a284`.
- Fallible parser helpers: `66375b83`.
- Collision-safe CSV string lookups: `c6590ba0`.
- Strict shared generational-ID parser: `912a219a`.
- Index-safe `NodeArena::edit` guard: `316f13c0`.
- Typed audio enqueue results with compatibility shims: `7466460a`.
- DLC static generation kept on override-owning thread: `95d27d72`.
- Dynamic-script ABI v2 descriptor/fingerprint gate: `0ae60aaa`.

Integration gates so far:

- `cargo fmt --all -- --check`: pass.
- Combined touched-crate tests: 440+ unit tests and 21 doc tests pass; expected slow tests remain ignored.
- Touched-crate clippy with `-D warnings`: pass.

Still blocked by design work:

- Script trait-object calls are now gated by a versioned `#[repr(C)]` descriptor and exact build fingerprint before any dylib callback. A future fully stable ABI still needs an opaque handle, producer-owned destroy function, C-safe values, and unwind containment.
- Generic DLC registry needs canonical path/hash/type inventory emitted by the static pipeline; source-extension scanning cannot represent synthesized GLTF mesh/skeleton keys correctly.

## Severity map

- P0: filesystem escape or destructive-action risk; fix first.
- P1: UB, wrong core results, silent data corruption, hangs, or stale state.
- P2: hostile-input robustness, invariant/API traps, or material resource risk.
- P3: ergonomics, docs, metadata, and maintainability.

## Findings

### P0 — DLC names permit path escape

Evidence:

- `perro_source/build_pipeline/perro_compiler/src/scripts.rs:7-13` rejects only the reserved name `self`.
- `sync_dlc_scripts` joins the raw name below `dlcs` and `.perro/dlc` at `scripts.rs:23-32`.
- `perro_source/build_pipeline/perro_compiler/src/dlc.rs:401-415` repeats raw joins.
- `dlc.rs:451-454` recursively removes the computed staging path.

Impact: a name such as `../target` can move reads/writes outside the DLC root and can direct recursive removal outside the intended staging directory.

Fix:

1. Add one validated `DlcName`/single-path-component type shared by compiler, CLI, IO, and project parsing.
2. Reject empty, `.`, `..`, separators, prefixes, absolute paths, NUL, and platform-specific alternate separators.
3. Resolve/canonicalize the nearest existing ancestor and prove the final path remains a descendant before write, move, or recursive remove.
4. Add traversal tests for `/`, `\`, drive/UNC prefixes, encoded-looking text, and nested `..`.

Acceptance: no public/compiler entry point can form an out-of-root DLC path; recursive removal receives only a checked descendant.

### P1 — scene lexer hides malformed input and can recurse deeply

Evidence:

- `perro_source/runtime_project/perro_scene/src/lexer.rs:96-100` skips malformed numeric lexemes by recursively requesting another token.
- `lexer.rs:113` recursively skips unknown characters.
- `lexer.rs:70-84` accepts an unterminated quote as a normal string token.
- Comment skipping at `lexer.rs:60-65` also recurses.
- The fallible parser contract begins at `perro_source/runtime_project/perro_scene/src/parser.rs:761`.

Impact: corrupt scenes and typos may parse successfully with data silently omitted. Long runs of comments or invalid bytes can overflow the call stack.

Fix: make lexing iterative; emit a typed lexical error with byte span and line/column; propagate it through parser errors. Test malformed numbers, unknown chars, EOF strings, and very long comment/invalid runs.

### P1 — `Transform3D::looking_at` uses view rotation

Evidence:

- `perro_source/core/perro_structs/src/structs/structs_3d/transform_3d.rs:101-110` extracts rotation from `Mat4::look_at_rh`, a world-to-view matrix.
- The crate already has the object-space `Quaternion::looking_at` contract at `.../quaternion.rs:232-270`.

Impact: object orientation is inverted/transposed relative to the expected local `-Z` look direction.

Fix: route `Transform3D::looking_at` through `Quaternion::looking_at(target - eye, up)` or invert the view rotation. Test that transformed local `-Z` points at `target - eye`, plus degenerate-up behavior.

### P1 — script dynamic-library ABI exposes Rust trait objects as C ABI

Evidence:

- `perro_source/script_stack/perro_scripting/src/script_trait.rs:8-9` declares an `extern "C" fn() -> *mut dyn ScriptBehavior<API>` and suppresses `improper_ctypes`.
- Generated registry exports the same trait-object function pointer in `perro_source/build_pipeline/perro_compiler/src/script_writer.rs:72-80,116-138`.

Impact: Rust fat-pointer/vtable layout is not a stable C ABI. Host/plugin compiler or layout mismatch can cause UB.

Fix: design a versioned `#[repr(C)]` ABI table with opaque handles and explicit construct/drop/call functions; include ABI/version/size handshake. Add an end-to-end test that builds and loads a generated dylib and rejects a mismatched ABI.

### P1 — safe DLC callback registration can cause UB

Evidence:

- `perro_source/io_stack/perro_io/src/asset_io.rs:170` exposes safe `register_dlc_static_binary_lookup`.
- Callback type is unsafe at `asset_io.rs:16`.
- `asset_io.rs:516-525` trusts foreign pointer and length and creates a slice.

Impact: a callback with an invalid pointer, lifetime, or length causes UB through later safe asset APIs.

Fix: mark registration unsafe and document the full lifetime/validity/threading contract, or replace the ABI with a copy-into-buffer/owned-result contract. Add a maximum length before copying. Keep all unsafe conversion inside one audited boundary.

### P1 — disk remount leaves stale archive backing

Evidence:

- Disk mount updates `DLC_MOUNTS` at `perro_source/io_stack/perro_io/src/asset_io.rs:141`.
- Archive mount inserts into its separate map at `asset_io.rs:161`.
- `read_mounted_dlc_file` consults `DLC_ARCHIVES` directly at `asset_io.rs:117`.

Impact: archive `X` -> disk `X` makes listing/path resolution report disk while reads can still return bytes from the old archive.

Fix: every mount operation must atomically replace/remove all other backing variants for the name. Prefer one map keyed by name with a `Disk | Archive | Static` enum. Add every remount-order test.

### P1 — generated script write lock can hang forever

Evidence: `perro_source/build_pipeline/perro_compiler/src/script_writer.rs:174-189` loops while `.write-lock` exists, with no timeout, owner data, or stale recovery.

Impact: crash or kill during generation leaves every later sync blocked forever.

Fix: use an atomic lock file/dir containing PID, process-start identity, timestamp, and purpose; bound waits; reclaim only proven-stale locks; return a diagnostic error. Test orphan, live contention, timeout, and cleanup after panic/error.

### P1 — generic DLC registry reports false negatives

Evidence:

- `perro_source/build_pipeline/perro_compiler/src/dlc.rs:215-224` generates `registry_len = 0` and `registry_get = false`.
- Generic lookup at `dlc.rs:195-213` covers only a subset and omits scene/material/particle/animation assets.

Impact: discovery and generic lookup APIs claim assets are absent even when typed generated assets exist.

Fix: generate complete typed registry entries for every supported asset kind, or remove/rename the generic exports and document a typed-only contract. Add pack-build -> dylib-load -> enumerate -> lookup E2E coverage.

### P2 — DLC mount APIs bypass their own name validator

Evidence:

- Validator at `perro_source/io_stack/perro_io/src/asset_io.rs:284-291` rejects empty/dot/slash forms.
- Mount functions at `asset_io.rs:124,149` reject only `self` and do not call it.

Impact: invalid/unreachable mounts and ambiguous names enter global state.

Fix: use the shared `DlcName` type from the P0 fix in disk, archive, and static registration APIs.

### P2 — tileset decoder accepts non-finite/invalid geometry

Evidence:

- `perro_source/render_stack/perro_render_bridge/src/two_d.rs:479-498` uses `<= 0` checks, which NaN bypasses.
- Shape values at `two_d.rs:554-585` lack finite/range validation.

Impact: corrupt `.ptileset` bytes inject NaN/Inf or negative dimensions into render/physics state.

Fix: require finite positive dimensions/radii, finite offsets/points, and format-specific upper bounds. Add crafted corrupt-binary tests.

### P2 — `MicClip` public fields bypass invariants and serializers truncate

Evidence:

- `perro_source/audio_stack/perro_pawdio/src/mic.rs:16-20` exposes fields publicly.
- Constructor clamps sample rate/channels at `mic.rs:23-28`, but struct literals bypass it.
- `pack` narrows frame count to `u32` at `mic.rs:41`; WAV arithmetic/casts at `mic.rs:183-199` are unchecked.

Impact: zero/invalid channel configs, partial frames, integer truncation, overflow, or malformed output.

Fix: make fields private; add validated `try_new`; require samples divisible by channels; use checked conversions/multiplication; make pack/WAV encode fallible. Document sample format, interleaving, units, and limits.

### P2 — texture commands can allocate 256 MiB CPU buffers each

Evidence: `perro_source/render_stack/perro_graphics/src/backend.rs:995-1000,1404-1416` clamps dimensions to 8192 but allocates full RGBA backing.

Impact: one 8192² request allocates about 256 MiB; repeated external/camera texture commands can stall or OOM before GPU validation.

Fix: validate against device dimensions and a byte budget with checked arithmetic; avoid CPU backing for external targets where possible; emit a typed failure event. Add boundary and repeated-request tests.

### P2 — CSV hash-only name lookup treats hashes as identity

Evidence:

- Header lookup at `perro_source/core/perro_csv/src/lib.rs:120-124` compares only a 64-bit hash.
- Primary-key lookup/maps at `perro_csv/src/lib.rs:132-140,226-233` also use hash identity.
- Predicate paths at `perro_csv/src/lib.rs:768-800` verify hash plus text, showing the safer existing pattern.

Impact: a collision returns the wrong header or row.

Fix: store collision buckets and compare original text. Make pre-hashed APIs accept a typed key containing hash plus source text, or clearly mark truly hash-only methods as trusted/unchecked. Add injected-hasher collision tests.

### P2 — `NodeArena::get_mut` makes index desync easy

Evidence: `perro_source/runtime_project/perro_runtime/src/cns/node_arena.rs:14-22,213-219` documents that direct mutation of indexed fields such as name/tag/parent can desynchronize auxiliary indexes, while returning unrestricted mutable node access.

Impact: an ordinary safe API call can leave lookup/query state incorrect.

Fix: hide indexed fields behind arena operations, or return an edit guard that snapshots keys and reindexes on drop/commit. Add mutation-through-public-API consistency tests.

### P2 — global static-pipeline overrides leak/clobber state

Evidence:

- `perro_source/build_pipeline/perro_static_pipeline/src/lib.rs:51-71` exposes thread-local ambient overrides.
- `perro_source/build_pipeline/perro_compiler/src/dlc.rs:376-398` resets them only on the normal result path.

Impact: nesting clobbers caller state; panic leaves stale configuration on a reused worker thread.

Fix: return a scoped RAII guard that restores the prior value, or pass an explicit `PipelineContext`. Test nesting, error, and unwind restoration.

### P2 — parser ergonomics encourage process aborts

Evidence: panic variants `parse_scene`, `parse_scene_doc`, and `parse_value_literal` live at `perro_source/runtime_project/perro_scene/src/parser.rs:757-789`; errors are strings rather than a typed error/span.

Impact: editor/user-content mistakes can abort the host when callers choose the shortest-looking API.

Fix: make fallible APIs canonical (`parse_* -> Result`), rename panic helpers to `*_or_panic`, deprecate ambiguous old names, and return typed errors with source spans.

### P2 — ID parsers accept hyphens anywhere

Evidence: `perro_source/core/perro_ids/src/ids.rs:218-244` removes every hyphen before parsing and duplicates logic for ID types.

Impact: malformed inputs such as `1-2` are silently accepted; format docs and validation disagree.

Fix: define exact accepted canonical/legacy formats, implement one shared parser plus `FromStr`, and reject misplaced separators.

### P3 — audio command API erases failure cause

Evidence: `perro_source/audio_stack/perro_pawdio/src/controller.rs:236-275,308-370` maps `try_send(...).is_ok()` to `bool` for many commands.

Impact: queue-full, disconnected, and success states collapse into one bit; “load succeeded” means only “enqueued.”

Fix: return `Result<Enqueued, AudioCommandError>` and expose async load completion/events. Keep deprecated bool shims for compatibility.

### P3 — public API docs have no enforced floor

Evidence:

- No source crate enables `warn(missing_docs)`.
- Examples of broad undocumented surfaces: `perro_pawdio/src/types.rs:5-258`, `controller.rs:16-553`, render bridge commands/state, scene lexer/parser tokens, and CSV query builders.
- A scan found many crate roots without crate-level `//!` docs.

Fix lane:

1. Start with facade/API crates: `perro_api`, `perro_runtime_api`, `perro_resource_api`, `perro_input_api`, `perro_nodes`, `perro_scene`, `perro_pawdio`.
2. Add crate docs and examples before enabling `#![warn(missing_docs)]` per crate.
3. Document units, ranges, finite rules, error/panic behavior, thread/queue semantics, URI grammar, and every unsafe contract.
4. Promote to deny in CI only after the baseline reaches zero.

### P3 — package metadata is incomplete

Evidence: representative published/user-facing manifests (`perro_scripting`, `perro_compiler`, `perro_cli`) contain only name/version/edition and omit common package metadata. Root has no `[workspace.package]` inheritance.

Fix: add workspace `edition`, `rust-version`, `license`, `repository`, `homepage`, authors/readme policy, and descriptions per crate. Validate intended publish sets with `cargo package` in CI.

### P3 — docs and agent map drift

Evidence:

- Agent/project docs mention `playground/`, while samples live in `demos/`.
- `README.md:12` contains `seperating`; nearby intro text has wording/encoding/case issues.
- `docs/index.md:7` uses absolute `/book`, which fails in repository-local GitHub navigation.
- Existing audit files contain mojibake (`â€”`, arrows); similar text appears in source docs such as `node_arena.rs`.

Fix: update directory maps and links; normalize text to UTF-8; add markdown link and spelling checks with a small project dictionary.

## Build order

Keep each lane small and independently reversible.

1. **Contain paths**: P0 `DlcName`, containment checks, mount validation, traversal tests.
2. **Fix wrong results/state**: lexer, transform look-at, remount replacement, DLC registry.
3. **Close UB boundaries**: script ABI design/migration, static callback contract.
4. **Harden lifecycle/input**: write-lock recovery, tileset validation, `MicClip`, texture budgets, pipeline override guard.
5. **Repair safe API invariants**: `NodeArena` edit guard, CSV collision-safe keys, fallible parser naming, strict ID parsing.
6. **Improve ergonomics/docs**: typed audio errors, package metadata, rustdoc baseline, docs link/spell cleanup.

Do not combine ABI migration, filesystem containment, and broad docs churn in one commit.

## Per-lane workflow

1. Reproduce with the smallest failing unit/integration test.
2. Add the test first where practical.
3. Fix one invariant at its owning type/boundary; remove duplicate validators.
4. Run `cargo fmt --all -- --check`.
5. Run scoped tests and clippy for touched crates.
6. Run `cargo check --workspace --all-targets`.
7. For cross-crate/ABI/path changes, run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.
8. Update this file: mark the item done with commit ID, tests, and any compatibility follow-up.

## Required focused tests

- DLC: traversal/property cases; every remount order; generated pack enumerate/load; recursive-delete containment.
- Scene: lexical error spans; invalid-number/quote/char cases; million-comment iterative stress.
- Math: look-at forward-axis property over varied eye/target/up values.
- ABI: build/load same version; reject wrong ABI/version/size; verify destructor path.
- IO FFI: registration contract tests and maximum payload size.
- Audio/media: malformed `MicClip`; checked size limits; queue full vs disconnect; texture byte budgets.
- State/indexes: mutate every indexed node property and assert all lookup paths remain coherent.
- CSV/IDs: forced hash collisions and strict textual-format properties.
- Global/lock state: nested override, unwind restore, stale lock, live lock, timeout.

## Audit follow-ups not promoted to bugs

- Add decoder fuzz targets for scene/tileset/skeleton/mesh/audio/container formats.
- Add a `cargo machete` CI gate after confirming deliberate target/feature-only dependencies.
- Keep prior performance backlog profile-driven: source interning, lock map, Variant size, prepass depth, GPU-driven rendering, and frame overlap.
- Split very large source files only while changing their behavior; avoid churn-only moves.

## Completion definition

- All P0/P1 items closed with regression tests.
- All P2 items either closed or tracked in issues with owner and milestone.
- Public safe APIs no longer rely on undocumented unsafe caller behavior.
- Workspace check/test/clippy pass on supported desktop targets; target-specific CI covers wasm and at least one non-Windows desktop.
- User-facing crates have crate docs, examples, package metadata, and an enforced missing-docs baseline.
- This document records commit IDs and verification results for every closed item.
