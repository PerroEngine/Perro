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
- `structs` that aren't used in runtime state/methods
- `enums` that aren't used in runtime state/methods

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
For new code, derive `Variant` on those types.

```rust
use perro::prelude::*;

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

Scene side:

```text
script_vars = { orbit_goal: { axis: (0.0, 0.0, 1.0) } }
```

