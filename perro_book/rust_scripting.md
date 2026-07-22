# Rust Scripting

Perro scripts are Rust modules with generated engine hooks.

## Goal

Write script state, lifecycle hooks, and callable methods.

## Script Boundary

Split scripts by state owner and lifecycle, not by arbitrary "one role per
file" rules. Keep helpers as normal Rust functions or shared modules when they
do not need independent state or attachment. Add another script when a node
needs its own lifecycle, scene wiring, or callable boundary.

This keeps direct Rust calls available inside one behavior while preserving
methods and signals for real runtime boundaries.

## Script Parts

Core script pieces:

- `type SelfNodeType = ...`
- `#[State]` for per-node state
- `lifecycle!` for engine hooks
- `methods!` for calls from other scripts
- `ctx.run`, `ctx.res`, `ctx.ipt` for runtime/resource/input APIs
- `NodeID` for self, refs, query results, and cross-script calls
- `Variant` for dynamic params and returns

Use the [Script Authoring Guide](/docs/scripting/authoring/index.md) as the
canonical choice guide for state, node refs, queries, signals, methods, timers,
and dynamic access.

## State

Use state for data that belongs to one script instance:

```rust
#[State]
pub struct DoorState {
    #[default(false)]
    open: bool,
}
```

Expose values when the editor inspector should show them:

```rust
#[default(4.0)]
#[expose]
speed: f32,
```

`#[expose]` is editor-only organization. Scene `script_vars` can inject any
state field that supports `Variant` conversion.

Store fixed dependencies as `NodeID` fields and per-instance resources as typed
asset IDs. Scene paths resolve before `on_init`, including nested derived types:

```rust
#[derive(Clone, Default, Variant)]
struct DoorLook {
    icon: TextureID,
}

#[State]
struct DoorState {
    #[node_ref(Node3D)]
    target: Option<NodeID>,
    look: DoorLook,
}
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

Prefer signals for events, fan-out, and loose cross-scene links. Prefer methods
for a targeted command with arguments or a return value.

Use named timers for one-shot delays and cooldown completion. Starting the same
timer name resets its deadline; use separate names for concurrent timers.

## Borrow Rule

Runtime macros borrow `ctx.run` during the macro call.

Do not use `ctx.run` again inside any `with_state!`, `with_state_mut!`,
`with_node!`, or `with_node_mut!` closure.

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
