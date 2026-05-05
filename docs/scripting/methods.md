# Script Methods

`methods!` defines callable behavior methods on your script type.

Use methods for reusable logic that you call from lifecycle hooks or through `call_method!`.

## Shape

```rust
methods!({
    fn my_method(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        value: i32,
    ) {
        // logic
    }
});
```

Required leading args:
- `&self`
- `ctx: &mut ScriptContext<'_, API>`

After that, add your own params.

For custom typed params/returns in `methods!`, derive `Variant` on the custom type.

Manual decode path also exists via `Variant::parse::<T>()`.
This equals `<T as DeriveVariant>::from_variant(&value)`.

```rust
let control_mode = params
    .first()
    .and_then(|v| v.parse::<ArcheryControlMode>().ok())
    .unwrap_or_default();
```

## Calling Methods

Internal call:

```rust
self.my_method(ctx, 10);
```

Runtime-dispatched call:

```rust
let out = call_method!(ctx.run, ctx.id, method!("my_method"), params![10_i32]);
```

## Guidance

- Prefer direct Rust calls (`self.foo(...)`) for local behavior.
- Use `call_method!` for dynamic/cross-script calls.
- Keep state mutations explicit with `with_state_mut!`.




