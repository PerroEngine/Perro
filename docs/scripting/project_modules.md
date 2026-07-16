# Project Script Modules

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |

## Purpose

Every Rust file under `res/**/*.rs` is compiled into your game. Some files are script behaviors attached to nodes; others are plain modules of shared helpers, constants, and types. This page explains which file shape becomes an attachable script versus a shared library, and how a file path maps to a `crate::...` import so game code can be split across files without registration boilerplate.

## Use Cases

- Attach gameplay behavior to a scene node: a file with `#[State]` (plus optional `lifecycle!` / `methods!`) is referenced by `script = "res://scripts/player.rs"` in the scene.
- Share damage tables, tuning constants, or math helpers across many scripts: put free functions and structs in a bare module (no `#[State]`) and import it.
- Reuse code by importing another project file: `use crate::scripts_math;` for `res/scripts/math.rs`, or `use crate::script_modules::*;` to pull the whole generated namespace.
- Keep a large system organized across folders: `res/ai/nav/util.rs` becomes `crate::ai_nav_util` following the lowercase, non-alphanumeric-to-`_` naming rule.

## Practical Example

A shared combat module holds a damage formula, and a weapon script imports it — no registration needed.

`res/combat/damage.rs` (bare module, no `#[State]`):

```rust
pub fn falloff_damage(base: f32, distance: f32, max_range: f32) -> f32 {
    let t = (distance / max_range).clamp(0.0, 1.0);
    base * (1.0 - t * 0.75)
}
```

`res/scripts/rifle.rs` (attachable script) imports it by its generated path:

```rust
use perro_api::prelude::*;
use crate::combat_damage::falloff_damage;

#[State]
pub struct RifleState {
    #[default(35.0)]
    pub base_damage: f32,
}

methods!({
    fn hit(&self, ctx: &mut ScriptContext<'_, API>, distance: f32) -> f32 {
        let base = with_state!(ctx.run, RifleState, ctx.id, |s| s.base_damage);
        falloff_damage(base, distance, 60.0)
    }
});
```

## Reference

Perro compiles every Rust file under `res/**.rs` into the generated scripts crate.

Two valid file shapes:

- script behavior file (`#[State]` + optional `lifecycle!`/`methods!`)
- bare Rust module (free functions, constants, enums, structs, traits, impls)

Bare modules are compiled and importable, but are not added to runtime script registry.

## Importing Project Modules

Perro emit crate-root short aliases from `res` path:

```rust
use crate::folder_module;
```

`crate::folder_module` maps to `res/folder/module.rs`.

Or import generated namespace once:

```rust
use crate::script_modules::*;
```

This exports each generated module and re-exports public items from those modules.

## Module Name Mapping

Perro maps `res` relative path to module id:

- lowercases all characters
- replaces non-alphanumeric chars with `_`
- preserves path structure through `_`

Examples:

- `res/scripts/math.rs` -> `crate::scripts_math`
- `res/ai/nav/util.rs` -> `crate::ai_nav_util`
- `res/Fx-Helpers.rs` -> `crate::fx_helpers`

## Script Attachment Rule

Only files that export script constructor are attachable from scene `script = "res://...rs"`.

Bare modules are for shared Rust code used by other scripts/modules.
