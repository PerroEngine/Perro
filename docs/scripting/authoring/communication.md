# Script Communication

Perro offers four communication paths. Pick from what the caller knows.

## Mental Model

```text
typed state = shared compile-time knowledge
method = command/question to one target
signal = fact announced to zero/many listeners
dynamic var = runtime-selected member bridge
```

| Caller knows                      | Use                               | Result                          |
| --------------------------------- | --------------------------------- | ------------------------------- |
| concrete Rust state type          | `with_state!` / `with_state_mut!` | typed read or write             |
| target node + behavior name       | `call_method!`                    | targeted call + `Variant` reply |
| event name                        | `signal_emit!`                    | zero or many listeners          |
| target node + runtime member name | `get_var!` / `set_var!`           | dynamic `Variant` access        |

## Call A Method

Use methods for commands sent to one known target. Methods accept params and
may return a value.

```rust
let result = call_method!(
    ctx.run,
    door_id,
    method!("set_open"),
    params![true]
);
let opened = result.as_bool().unwrap_or(false);
```

The receiver declares the dynamic API in `methods!`:

```rust
methods!({
    fn set_open(&self, ctx: &mut ScriptContext<'_, API>, open: bool) -> bool {
        with_state_mut!(ctx.run, DoorState, ctx.id, |state| {
            state.open = open;
        }).is_some()
    }
});
```

Inside the same script, call `self.set_open(ctx, true)` directly.

## Emit A Signal

Use signals for facts that happened. The emitter does not choose or know the
listeners.

```rust
signal_emit!(
    ctx.run,
    signal!("player_health_changed"),
    params![health]
);
```

Each listener connects one of its own methods once:

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("player_health_changed"),
            func!("on_health_changed")
        );
    }
});
```

Prefer a signal when many systems may react, the receiver may live in another
scene, or the emitter must stay independent from UI/audio/analytics code.

## Get Or Set A Dynamic Var

Use dynamic vars for generic tools and behavior where the member name or state
type is selected at runtime.

```rust
let old = get_var!(ctx.run, target_id, var!("health"))
    .as_i32()
    .unwrap_or(0);

set_var!(ctx.run, target_id, var!("health"), variant!(old + 10));
```

Do not use `get_var!` / `set_var!` as the default way to reach a known state
type. Dynamic access loses compile-time field checking and returns no typed
borrow.

`get_var!` and `call_method!` return `Variant`. Decode at the call site with an
`as_*` accessor, `parse::<T>()`, or `into_parse::<T>()`.

## Failure And Tradeoffs

Dynamic calls can miss a target/member or fail to decode params/replies. Signal
emission does not guarantee a listener or reply. Typed access gives the
strongest compile-time checks. Use it freely for a script's own state.
Cross-script typed access requires the target state to be public and
intentionally importable from its generated script module; prefer a method when
that module/type coupling is not part of the design. Keep behavior invariants
behind target methods; do not use dynamic vars to bypass target validation.

Signals are runtime-global channels keyed by `SignalID`, not by emitter. When
more than one source may emit the same event, include `ctx.id` in params or use
distinct channel names so listeners can identify the source.

## Examples

- [Targeted method call](examples/call_method.md)
- [Loose signal event](examples/signals.md)
- [Dynamic get/set vars](examples/dynamic_vars.md)
- [Combined pickup flow](examples/pickup_flow.md)
- [Runnable ScriptPatterns flow](../../../demos/ScriptPatterns/README.md)

[Back To Guide](index.md)
