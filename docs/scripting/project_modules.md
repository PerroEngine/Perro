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

Folder names do not decide whether a file is a script. Every `.rs` file under `res` becomes a project module, no matter which folder contains it. File content decides whether that module is also an attachable script.

Perro manages the Rust module declarations for the whole `res` tree. You normally create only folders and `.rs` files; you do not need to add `mod foo;`, create a `mod.rs` file, or maintain a separate module list. Use `mod.rs` only when the folder module itself needs constants, types, functions, or other code.

## Use Cases

- Attach gameplay behavior to a scene node: a file with `#[State]` (plus optional `lifecycle!` / `methods!`) is referenced by `script = "res://scripts/player.rs"` in the scene.
- Share damage tables, tuning constants, or math helpers across many scripts: put free functions and structs in a bare module (no `#[State]`) and import it.
- Reuse code by importing another project file: `use crate::scripts::math;` for `res/scripts/math.rs`, or use `super::math` from a sibling module.
- Keep a large system organized across folders: `res/ai/nav/util.rs` becomes `crate::ai::nav::util`.

## Decision Guide

Use a project module when several scripts share Rust types, pure calculations, or adapters. Keep lifecycle hooks, methods, and `#[State]` in the script that owns the node instance. A module call is an in-process Rust call, not cross-node messaging; use a method or signal when the target is another script instance.

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
use crate::combat::damage::falloff_damage;

#[State]
pub struct RifleState {
    #[default(35.0)]
    pub base_damage: f32,
}

methods!({
    fn hit(&self, ctx: &mut ScriptContext<'_, API>, distance: f32) -> f32 {
        let base = with_state!(ctx.run, RifleState, ctx.id, |s| s.base_damage).unwrap_or_default();
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

For example, `res/helper_modules/bob.rs` becomes `crate::helper_modules::bob`. It stays a bare project module unless its contents define Perro script behavior. The `scripts` folder name has no special registration meaning.

## Importing Project Modules

Treat `res` as the Rust crate root. Every folder below `res` becomes a module level, and every `.rs` file becomes the module at its matching path:

```text
res/scripts/gameplay/player.rs
-> crate::scripts::gameplay::player
```

Import one public item:

```rust
use crate::scripts::gameplay::player::PlayerState;
```

Import every public item from the module:

```rust
use crate::scripts::gameplay::player::*;
```

`crate` always starts at `res`, no matter which project module contains the import.

Relative imports follow the same tree. In `res/scripts/gameplay/player.rs`, `super` means its parent module, `crate::scripts::gameplay`:

```text
current module: crate::scripts::gameplay::player
super:          crate::scripts::gameplay
```

This makes sibling files available through `super`:

```rust
// res/scripts/gameplay/player.rs
use super::movement::move_player; // res/scripts/gameplay/movement.rs
use super::stats::PlayerStats;    // res/scripts/gameplay/stats.rs
```

Use more than one `super` to move through more than one parent. Descend through module names after reaching the needed parent:

```rust
// res/scripts/ui/hud.rs
use super::super::gameplay::player::PlayerState;
```

Perro discovers new folders and `.rs` files during script sync and updates the generated module tree. Adding `res/scripts/gameplay/player.rs` is enough to create `crate::scripts::gameplay::player`; no source file needs `mod scripts;`, `mod gameplay;`, or `mod player;`.

A `mod.rs` file is optional. Add one only when the folder module needs its own code. Its items live directly in that folder module:

```text
res/scripts/gameplay/mod.rs    -> crate::scripts::gameplay
res/scripts/gameplay/player.rs -> crate::scripts::gameplay::player
```

## Module Name Mapping

Perro maps each `res` relative path part to one module id:

- lowercases all characters
- replaces non-alphanumeric chars with `_`
- preserves folders through `::`
- adds `_` after a Rust keyword
- treats `mod.rs` as the parent folder module

Examples:

- `res/scripts/math.rs` -> `crate::scripts::math`
- `res/ai/nav/util.rs` -> `crate::ai::nav::util`
- `res/Fx-Helpers.rs` -> `crate::fx_helpers`
- `res/type/value.rs` -> `crate::type_::value`
- `res/ai/nav/mod.rs` -> items under `crate::ai::nav`

Do not create both `res/foo.rs` and `res/foo/mod.rs`. Both files define `crate::foo`, so Perro reports a module-path collision.

## Generated Crate Layout

Perro emits the `res` file tree as the real Rust module tree inside generated `.perro/scripts/src/lib.rs`. Each module includes its transformed `.gen.rs` wrapper:

```rust
pub mod scripts {
    pub mod gameplay {
        include!("scripts/gameplay/mod.gen.rs");

        pub mod movement {
            include!("scripts/gameplay/movement.gen.rs");
        }

        pub mod player {
            include!("scripts/gameplay/player.gen.rs");
        }
    }
}
```

The wrapper includes the real `res` source and adds Perro script glue when needed. Runtime registries reference the same nested module path:

```rust
scripts::gameplay::player::perro_create_script
```

## Script Attachment Rule

Only files that export script constructor are attachable from scene `script = "res://...rs"`.

Bare modules are for shared Rust code used by other scripts/modules.
