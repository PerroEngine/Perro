# Static Release Build Profile

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
