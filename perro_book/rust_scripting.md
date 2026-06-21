# Rust Scripting

Perro scripts are Rust modules with generated engine hooks.

## Goal

Write script state, lifecycle hooks, and callable methods.

## Script Parts

Core script pieces:

- `type SelfNodeType = ...`
- `#[State]` for per-node state
- `lifecycle!` for engine hooks
- `methods!` for calls from other scripts
- `ctx.run`, `ctx.res`, `ctx.ipt` for runtime/resource/input APIs
- `NodeID` for self, refs, query results, and cross-script calls
- `Variant` for dynamic params and returns

## State

Use state for data that belongs to one script instance:

```rust
#[State]
pub struct DoorState {
    #[default(false)]
    open: bool,
}
```

Expose values when editor or scene authoring needs them:

```rust
#[default(4.0)]
#[expose]
speed: f32,
```

## Lifecycle

Use lifecycle hooks for frame work:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        log_info!("init {:?}", ctx.id);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Methods

Use methods for behavior other scripts call.

Write typed Rust params and typed Rust returns.

Generated glue converts params from `Variant` and converts return values back into `Variant`.

```rust
methods!({
    fn set_open(&self, ctx: &mut ScriptContext<'_, API>, open: bool) -> bool {
        with_state_mut!(ctx.run, DoorState, ctx.id, |state| {
            state.open = open;
            state.open
        }).unwrap_or(false)
    }
});
```

Call methods by ID:

```rust
let ret = call_method!(ctx.run, door_id, method!("set_open"), params![true]);
let is_open = ret.parse::<bool>().unwrap_or(false);
```

Use a normal Rust helper for same-script logic when the caller is already inside the script.

Use `call_method!(ctx.run, ctx.id, ...)` for dynamic self dispatch, such as animation events or generic tool code.

Use `call_method!(ctx.run, other_id, ...)` for cross-script calls.

## Borrow Rule

Runtime macros borrow `ctx.run` during the macro call.

Do not use `ctx.run` again inside a `with_*_mut!` closure.

Pull copy data out first.

Clone owned data before closure when later code still needs it.

This shape is intentional.

State and node mutation stay inside short closures/calls, so long borrows do not cross later runtime API work.

## Modules

Use bare Rust modules for shared game code.

Keep engine-facing logic in scripts.

Keep math/helpers/data transforms in normal Rust modules.

## Reference

- [Scripting Overview](/docs/scripting/README.md)
- [Script State](/docs/scripting/state.md)
- [Script Lifecycle](/docs/scripting/lifecycle.md)
- [Script Methods](/docs/scripting/methods.md)
- [Project Script Modules](/docs/scripting/project_modules.md)
- [Variant](/docs/scripting/variant.md)
- [Scripts Module](/docs/scripting/contexts/runtime_modules/scripts.md)
