# Script Methods

## Page Map

| Header                   | Link                                                  |
| ------------------------ | ----------------------------------------------------- |
| Why `methods!` Exists    | [Why `methods!` Exists](#why-methods-exists)          |
| Method Shape             | [Method Shape](#method-shape)                         |
| Direct Calls             | [Direct Calls](#direct-calls)                         |
| Runtime Dispatch         | [Runtime Dispatch](#runtime-dispatch)                 |
| Typed Params And Returns | [Typed Params And Returns](#typed-params-and-returns) |
| Variant Decode           | [Variant Decode](#variant-decode)                     |

## Why `methods!` Exists

`methods!` adds callable behavior methods to the generated script type. The macro rewrites methods that take `ctx: &mut ScriptContext<'_, API>` into generic Rust methods with the correct `where API: ScriptAPI + ?Sized` bound. Because the macro owns that rewrite, methods do not declare `<API: ScriptAPI>` themselves.

Use `methods!` for logic you want to call directly from lifecycle hooks or dynamically through `call_method!`.

## Method Shape

```rust
methods!({
    fn apply_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> bool {
        amount > 0
    }
});
```

| Part          | Requirement                                                |
| ------------- | ---------------------------------------------------------- |
| receiver      | `&self`                                                    |
| context       | `ctx: &mut ScriptContext<'_, API>`                         |
| custom params | any supported typed params after `ctx`                     |
| return        | `()` or any type that converts with `Variant::from(value)` |

## Direct Calls

Direct calls are normal Rust calls. Use them inside the same script when you know the method at compile time.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if key_pressed!(ctx.ipt, KeyCode::Space) {
            let accepted = self.apply_damage(ctx, 1);
            let _ = accepted;
        }
    }
});

methods!({
    fn apply_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> bool {
        amount > 0
    }
});
```

## Runtime Dispatch

Use `call_method!` for cross-script calls or dynamic calls by `ScriptMemberID`. This path passes `Variant` params and always returns a `Variant`.

If the called method returns `bool`, `i32`, `String`, etc. or engine types like `NodeID`, `MeshID` or any custom `#[derive(Variant)]` type, the generated script bridge wraps that value with `Variant::from(value)`. If the called method returns `()`, the bridge returns `Variant::Null`.

Because dispatch is dynamic, caller code must know the expected return type and decode it.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let out = call_method!(ctx.run, ctx.id, method!("apply_damage"), params![10_i32]);
        let accepted = out.as_bool().unwrap_or(false);
    }
});
```

## Typed Params And Returns

Built-in scalar types work through `Variant`. Custom structs/enums used as method params or returns should derive `Variant`.

```rust
#[derive(Clone, Debug, Default, Variant)]
struct HitInfo {
    amount: i32,
}

methods!({
    fn apply_hit(&self, ctx: &mut ScriptContext<'_, API>, hit: HitInfo) -> bool {
        hit.amount > 0
    }

    fn last_hit(&self, ctx: &mut ScriptContext<'_, API>) -> HitInfo {
        HitInfo { amount: 10 }
    }
});
```

## Variant Decode

Decode `call_method!` results from `Variant` at the call site.

This is the same rule as `get_var!`: dynamic API returns `Variant`, caller decodes expected type.

```rust
let ok = call_method!(ctx.run, target, method!("apply_hit"), params![HitInfo { amount: 10 }])
    .as_bool()
    .unwrap_or(false);

let hit = call_method!(ctx.run, target, method!("last_hit"), params![])
    .into_parse::<HitInfo>()
    .unwrap_or_default();
```

Use `as_bool()` and other `as_*` accessors for cheap primitive reads. Use `parse::<T>()` when keeping the `Variant`, or `into_parse::<T>()` when consuming it.

See [Variant](variant.md) for accessors and custom type conversion.
