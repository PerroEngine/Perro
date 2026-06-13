# Script State

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| State Struct  | [State Struct](#state-struct)   |
| Editor Expose | [Editor Expose](#editor-expose) |
| Defaults      | [Defaults](#defaults)           |
| Runtime Vars  | [Runtime Vars](#runtime-vars)   |
| Custom Types  | [Custom Types](#custom-types)   |

## Purpose

Script state stores per-node data for one script instance.

Each node with that script gets its own state value. Use state for mutable gameplay data, scene overrides, and values other scripts need to read or write.

## State Struct

Use `#[State]` on one struct in the script.

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default(100.0)]
    #[expose]
    health: f32,

    #[default(240.0)]
    #[expose]
    speed: f32,

    velocity: Vector2,
    grounded: bool,
    jump_buffer_timer: f32,
}
```

`#[State]` generates `Default` for the struct.

Fields without `#[default(...)]` use `Default::default()`.

## Editor Expose

`#[expose]` is an editor marker.

The engine state path ignores it.

The Perro editor reads the source text under `#[State]` and shows only fields with `#[expose]` in the inspector.

Use it for values you want to tune in the editor without recompiling, and for scene refs like `NodeID` that are easier to wire from the inspector.

```text
script = "res://scripts/player.rs"
script_vars = {
    health = 75.0,
    speed = 300.0
}
```

Fields without `#[expose]` stay hidden from the editor inspector.

Use this for internal values like velocity, timers, cached refs, and state flags.

## Defaults

Use `#[default(...)]` to set the initial value.

```rust
#[State]
pub struct SpinnerState {
    #[default(6.0)]
    #[expose]
    turn_speed: f32,

    #[expose]
    target: NodeID,

    #[default(false)]
    paused: bool,
}
```

Both `#[default(expr)]` and `#[default = expr]` are accepted.

`#[expose]` can appear before or after `#[default(...)]`.

Scene `script_vars` override defaults after state creation.

## Runtime Vars

Inside the same script, use typed state access.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, ctx.id, PlayerState, |state| {
            state.jump_buffer_timer -= delta_time!(ctx.run);
        });
    }
});
```

Other scripts and runtime systems can use state variables.

```rust
let health = get_var!(ctx.run, player_id, "health");
set_var!(ctx.run, player_id, "speed", variant!(320.0_f32));
```

`get_var!`, `set_var!`, and `script_vars` are runtime paths.

They do not require `#[expose]`.

## Custom Types

Custom structs/enums used through script variable APIs must support Variant conversion.

Derive `Variant` on those types.

```rust
use perro_api::prelude::*;

#[derive(Clone, Copy, Variant)]
pub struct OrbitGoal {
    pub axis: Vector3,
}

#[State]
pub struct SpinnerState {
    #[default(OrbitGoal { axis: Vector3::new(0.0, 1.0, 0.0) })]
    #[expose]
    orbit_goal: OrbitGoal,
}
```

This also applies to custom typed params/returns used in `methods!`.

See [Variant](variant.md) for accessors, `parse::<T>()`, and `into_parse::<T>()`.
