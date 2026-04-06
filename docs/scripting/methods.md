# Script Methods

`methods!` defines callable behavior methods on your script type.

Use methods for reusable logic that you call from lifecycle hooks or through `call_method!`.

## Shape

```rust
methods!({
    fn my_method(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        value: i32,
    ) {
        // logic
    }
});
```

Required leading args:
- `&self`
- `ctx: &mut RuntimeContext<'_, RT>`
- `res: &ResourceContext<'_, RS>`
- `ipt: &InputContext<'_, IP>`
- `self_id: NodeID`

After that, add your own params.

For custom typed params/returns in `methods!`, the type must implement `CustomVariant`
(for new code, derive `Variant`).

## Calling Methods

Internal call:

```rust
self.my_method(ctx, res, ipt, self_id, 10);
```

Runtime-dispatched call:

```rust
let out = call_method!(ctx, self_id, method!("my_method"), params![10_i32]);
```

## Guidance

- Prefer direct Rust calls (`self.foo(...)`) for local behavior.
- Use `call_method!` for dynamic/cross-script calls.
- Keep state mutations explicit with `with_state_mut!`.
