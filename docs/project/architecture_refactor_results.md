# Architecture refactor results

Record final architecture changes, validation and same-host CPU measurements.

## Change

- Centralize update hooks and service membership in node metadata; derive legacy flags.
- Share phase order between normal and profiled execution.
- Group 86 private fields into scene, extraction and physics-sync owners; keep callback state together.
- Relax runtime/resource facade bounds to each accessed domain; keep game API syntax.
- Add architecture guide, dependency-direction CI check and reusable A/B probe runner.

## Method

Use the initial worktree at `3a4549f9beb718e3c94e2a3653ad72f5a84bd749`, including prior uncommitted work, as baseline. Preserve it under `target/architecture-refactor/baseline-src`.
Build baseline and candidate into separate targets. Add the same probe fixture to both. Keep compiler jobs stopped during timing.
Run on Windows/MSVC, Intel i9-9900K, Rust 1.97.1, using the workspace bench profile.
For each fixture, alternate baseline/candidate order across seven process pairs. Each process warms 60 frames, then collects 101 batches of ten frames. Report median paired change; negative means less CPU time.
Inspect [raw samples and executable hashes](architecture_refactor_results.json).

## CPU samples

| Fixture | Nodes | Paired CPU change | Slower pairs |
| --- | ---: | ---: | ---: |
| Mixed built-in hooks | 1,000 | -12.15% | 0/7 |
| 2D fixed bone-chain hooks | 1,000 | -19.23% | 0/7 |
| Hook attach/remove churn | 1,000 | -11.93% | 0/7 |
| Full frame, idle scripts | 1,000 | -0.15% | 3/7 |
| Full frame, 1% moving | 1,000 | +0.22% | 6/7 |
| Full frame, all moving | 1,000 | -0.37% | 2/7 |
| Mixed built-in hooks | 10,000 | -12.38% | 0/7 |
| 2D fixed bone-chain hooks | 10,000 | -19.09% | 0/7 |
| Hook attach/remove churn | 10,000 | -11.66% | 0/7 |
| Full frame, idle scripts | 10,000 | +0.09% | 4/7 |
| Full frame, 1% moving | 10,000 | -0.23% | 2/7 |
| Full frame, all moving | 10,000 | -0.11% | 2/7 |

An earlier layout showed a repeatable ~1% idle-frame slowdown. Reject that candidate and group hot callback state together. The final full-frame pairs overlap baseline noise; no repeatable slowdown remains in this matrix.
The small positive sparse-frame result at 1k nodes gets an additional 11-pair check with both executables on CPU 2: −0.23% median, 4/11 slower pairs.
Run existing 512-body force/impulse benchmarks for 2D, 3D and mixed dimensions. Short 2D samples flag one regression; extend that case to seven alternating pairs with 60 samples, one-second warmup and two-second measurement windows.
The extended 2D check gives -0.96% median paired change, with both positive and negative pairs. The short 3D/mixed comparisons show no statistically significant regression. These existing fixtures include runtime teardown in their measured closure.

## Allocation and size checks

- Keep `size_of::<Runtime>()` at 20,112 bytes.
- Keep zero allocations and zero retained-byte growth across 1,010 warmed frames for update hooks, fixed hooks and idle/sparse/all-moving full-frame fixtures.
- Keep churn at 15 allocations and 47,616 retained bytes over the same interval, equal to baseline.

## Validation

- Pass workspace tests excluding graphics: 2,065 tests; 56 ignored.
- Pass serial graphics suite: 540 tests.
- Pass final runtime suite: 794 tests, including one extra phase-order test after the workspace run.
- Cover script-before-builtin order, normal/timed world and draw parity, live type replacement, generational reuse, schedule mutation, physics freshness and resource invalidation.
- Pass workspace format check and dependency-direction check across 44 crates.
- Pass final runtime-only Clippy with `-D warnings`.
- Pass final docs suite: 16 tests; 1 ignored, including generated internal links.
- Pass runtime library check for `wasm32-unknown-unknown` without default features.
- Pass workspace Clippy with all targets. The existing `-F clippy::all` command reports future-compatibility warnings for existing allow/expect attributes.

## Reproduce

Build both benchmark binaries from separate source snapshots/targets. Run the comparison without concurrent builds:

```text
python tools/bench_architecture.py <baseline-exe> <candidate-exe> --output <results.json>
```

Use `--nodes`, `--cases`, `--repeats` and optional Windows `--cpu` for focused rechecks.
For Criterion physics probes, invoke the executable with `--bench physics/runtime_fixed_step_forces`; direct invocation without `--bench` runs fixture checks without timing.

Limit claims to these workloads and this host. No GPU-time, cross-OS speed or controlled incremental-compile claim.
Keep crate dependencies unchanged; enforce direction in CI. Preserve the existing static/dev asset paths and resource-event ordering.
