# Script State

## Page Map

| Header    | Link                    |
| --------- | ----------------------- |
| Purpose   | [Purpose](#purpose)     |
| Use Cases | [Use Cases](#use-cases) |
| Example   | [Example](#example)     |
| Reference | [Reference](#reference) |

## Purpose

Use `Script State` when this feature, type group, file format, or workflow appears in game code or assets.

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

# Script State

`#[State]` is for variables registered per script instance.

Each node with that script gets its own state instance, and those fields are what runtime/script APIs can read/write (`with_state!`, `with_state_mut!`, `get_var!`, `set_var!`).

## What Goes In `#[State]`

- Per-instance mutable gameplay data
- Values you want exposed/accessible through script runtime APIs
- Data that must differ per node/script instance

## What Can Stay Outside State

You can keep normal Rust items outside state:

- `const` values
- `structs`
- `enums`

## Example

```rust
const SPEED: f32 = 6.0;

#[State]
pub struct PlayerState {
    #[default = 100]
    health: i32,
}
```

If you need cross-script/runtime member access, put that value in `#[State]`.

## Custom Types And Variant Conversion

Custom structs/enums used by script APIs must support Variant conversion.
Derive `Variant` on those types.

```rust
use perro_api::prelude::*;

#[derive(Clone, Copy, Variant)]
pub struct OrbitGoal {
    pub axis: Vector3,
}

#[State]
pub struct SpinnerState {
    #[default = OrbitGoal { axis: Vector3::new(0.0, 1.0, 0.0) }]
    pub orbit_goal: OrbitGoal,
}
```

This applies to both:

- custom types stored in `#[State]`
- custom typed params/returns used in `methods!` (runtime/cross-script dispatch path)

If a custom type used there does not derive `Variant`, script compilation fails.

See [Variant](variant.md) for accessors, `parse::<T>()`, and `into_parse::<T>()`.

Scene side:

```text
script_vars = { orbit_goal: { axis: (0.0, 0.0, 1.0) } }
```
