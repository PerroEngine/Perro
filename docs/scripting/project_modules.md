# Project Script Modules

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Project Script Modules` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# Project Script Modules

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
