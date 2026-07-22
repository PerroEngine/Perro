# Static Release Build Profile

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Scope | [Scope](#scope) |
| Current profile | [Current profile](#current-profile) |
| Separate dynamic paths | [Separate dynamic paths](#separate-dynamic-paths) |
| Measure change | [Measure change](#measure-change) |
| Guard | [Guard](#guard) |

## Purpose

When you run `perro build`, Perro generates an isolated `.perro/project` crate and links your static script code into the final native, web, or Android artifact. This page documents the `[profile.release]` that crate uses — `opt-level = 3`, fat LTO, one codegen unit, `panic = abort`, stripped root symbols — and why each key is set that way. It also explains how the release profile differs from the separate dev and DLC dylib build paths, and how to measure a profile change before adopting it. Read it before touching release build settings or weighing build-time cost against runtime and binary-size wins.

## Use Cases

- **See what optimizations ship in a release build.** The [Current profile](#current-profile) table lists every `[profile.release]` key (`opt-level`, `lto`, `codegen-units`, `panic`, `debug`, `strip`, `incremental`) with the reason for each.
- **Avoid breaking the release profile.** The `scaffold_project_release_strip_only_targets_project_package` guard test checks the emitted keys; update it alongside any policy change.
- **Keep dev and DLC builds fast without copying their overrides.** Dev script dylibs use incremental + 64 codegen units + no LTO; DLC dylibs use O3 + fat LTO + one codegen unit; neither belongs in the static project build.
- **A/B test a profile tweak safely.** Set separate `CARGO_TARGET_DIR` values and `CARGO_PROFILE_RELEASE_*` env vars, then compare build time, artifact size, boot time, and frame time.
- **Decide whether a change is worth it.** Keep a profile change only when it shows a measured runtime or size win worth the added build-time cost.

## Decision Rule

Treat profile keys as measured project policy, not generic "maximum optimization"
switches. Compare build time, artifact size, startup, and representative frame
time. Keep dev/DLC overrides separate because they optimize different loops.

## Scope

`perro build` creates an isolated `.perro/project` crate.

`scripts` stays a normal path dependency of that crate.

Cargo links static script code into final native, web, or Android artifact.

`[profile.release]` in generated project manifest applies to root crate and dependencies.

## Current profile

| Key | Value | Why |
| --- | --- | --- |
| `opt-level` | `3` | Max runtime optimization. |
| `lto` | `fat` | Cross-crate optimization, including static script crate. |
| `codegen-units` | `1` | More whole-program optimization. |
| `panic` | `abort` | Remove unwind runtime. |
| `debug` | `false` | Avoid debug info. |
| root `strip` | `symbols` | Cut shipped root artifact symbol table. |
| dependency `strip` | `none` | Keep linker inputs usable through final link. |
| `incremental` | `true` | Keep repeated project builds practical; validate code-size/runtime gain before changing. |

This profile already covers static `SCRIPT_REGISTRY` constructors.

No C ABI, dylib export, or `no_mangle` path exists in static release registry code.

## Separate dynamic paths

Dev script dylibs use fast release overrides: incremental, 64 codegen units, no LTO.

DLC package dylibs use O3, fat LTO, one codegen unit, and no incremental build.

Do not copy dylib overrides into static project build.

Static project links scripts directly; dylib path optimizes rebuild/reload cost.

## Measure change

Use a real project with stable scene/script workload.

Run clean baseline and candidate builds in distinct target dirs.

```powershell
$env:CARGO_TARGET_DIR = "$PWD\target-release-baseline"
perro build --path <project>

$env:CARGO_TARGET_DIR = "$PWD\target-release-candidate"
$env:CARGO_PROFILE_RELEASE_LTO = "off"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "64"
perro build --path <project>
```

Record:

- wall build time
- final executable or wasm size
- boot time
- scene attach time with 1k+ static scripts
- frame time in script-heavy scene

Clear `CARGO_PROFILE_RELEASE_*` env vars after comparison.

Keep profile change only with measured runtime or size win worth build-time cost.

## Guard

`scaffold_project_release_strip_only_targets_project_package` checks emitted profile keys.

Update test and this page together with any release-profile policy change.
