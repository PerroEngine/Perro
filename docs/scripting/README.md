# Scripting Overview

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Scripting Overview` when this feature, type group, file format, or workflow appears in game code or assets.

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

# Scripting Overview

Perro scripts are authored in Rust and compiled into script modules.
Perro manages most glue code for you, so scripting stays close to normal Rust instead of turning into registration boilerplate.

Core pieces:

- `#[State]` data struct
- `lifecycle!` for engine entry points
- `methods!` for callable behavior methods
- bare Rust modules for shared code (`res/**.rs` with no script behavior)
- script contexts (`RuntimeWindow`, `ResourceWindow`, `InputWindow`)

Borrow rule:

- `ctx.run` uses mutable runtime access.
- Runtime macros borrow `ctx.run` for duration of macro call.
- Do not use `ctx.run` again inside `with_state_mut!`, `with_node_mut!`, or similar closure.
- Pull copy data out first (`f32`, `NodeID`, ids, bools, enums, small math types).
- If data owns heap content (`String`, `Vec`, `Cow`, custom clone types), clone out b4 closure if later code still needs it.
- Clone cost stays local; tmp clone drops aft closure/use site.

Script dependencies:

- Add extra crates to `deps.toml` in your project root under `[dependencies]`.
- On `perro check`, `perro dev`, and `perro build`, Perro merges those entries into `.perro/scripts/Cargo.toml`.
- Keep `perro` managed by Perro; do not override it in `deps.toml`.

See:

- [Project Script Modules](project_modules.md)
- [Script Contexts](contexts/README.md)
- [Script Utility Modules](modules.md)
- [Struct Types](structs/index.md)
- [Node Types](nodes.md)
- [Water Bodies](water.md)
- [Scene Node Templates](scene_node_templates/index.md)
  - Includes `root_of` scene composition (scenes inside scenes), merge rules, and examples.
- [Script State](state.md)
- [Script Lifecycle](lifecycle.md)
- [Script Methods](methods.md)
