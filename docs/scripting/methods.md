# Script Methods

## Page Map

| Header | Link |
| --- | --- |
| Why `methods!` Exists | [Why `methods!` Exists](#why-methods-exists) |
| Method Shape | [Method Shape](#method-shape) |
| Direct Calls | [Direct Calls](#direct-calls) |
| Runtime Dispatch | [Runtime Dispatch](#runtime-dispatch) |
| Typed Params And Returns | [Typed Params And Returns](#typed-params-and-returns) |

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

| Part | Requirement |
| --- | --- |
| receiver | `&self` |
| context | `ctx: &mut ScriptContext<'_, API>` |
| custom params | any supported typed params after `ctx` |
| return | `()` or supported typed return |

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

Use `call_method!` for cross-script calls or dynamic calls by `ScriptMemberID`. This path passes `Variant` params and returns a `Variant`.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let out = call_method!(ctx.run, ctx.id, method!("apply_damage"), params![10_i32]);
        let _ = out;
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
});
```
