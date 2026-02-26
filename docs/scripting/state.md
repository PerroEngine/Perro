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
- variables not needing to be accessed by the engine or other scripts

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
