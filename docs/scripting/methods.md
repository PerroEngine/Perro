# Script Methods

## Page Map

| Header                   | Link                                                  |
| ------------------------ | ----------------------------------------------------- |
| Purpose                  | [Purpose](#purpose)                                   |
| Decision Model           | [Decision Model](#decision-model)                     |
| Use Cases                | [Use Cases](#use-cases)                               |
| Why `methods!` Exists    | [Why `methods!` Exists](#why-methods-exists)          |
| Method Shape             | [Method Shape](#method-shape)                         |
| Visibility               | [Visibility](#visibility)                             |
| Direct Calls             | [Direct Calls](#direct-calls)                         |
| Runtime Dispatch         | [Runtime Dispatch](#runtime-dispatch)                 |
| Typed Params And Returns | [Typed Params And Returns](#typed-params-and-returns) |
| Variant Decode           | [Variant Decode](#variant-decode)                     |

## Purpose

`methods!` gives a script named behavior methods you can call directly from its own lifecycle hooks or dynamically from other scripts. It is how a node gets an API — `apply_damage`, `toggle`, `interact` — so gameplay reads as method calls instead of scattered flag-poking. Direct calls stay ordinary typed Rust; cross-script calls go through `call_method!` and `Variant`.

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Same script calls its own helper | direct Rust method, no `pub` | Compiler checks params and return type; no dispatch glue generated | Only available where concrete script code is known |
| Switch targets one scene-wired door | `pub fn` + `call_method!` | One receiver, params, and reply match command semantics | Runtime name/type mismatch returns a dynamic failure value |
| Producer announces an event to unknown listeners | signal, not method | Producer does not own listener set | No direct return value |
| Generic tool edits a member | `get_var!` / `set_var!`, not method | Operation is data access selected at runtime | Skips domain behavior unless a setter method enforces it |
| Call carries `HitInfo` and returns `HitResult` | derive `Variant` on both types | Dynamic boundary keeps one explicit schema | Decode remains fallible at receiver and caller |

## Decision Model

A method is a targeted request: the caller chooses one `NodeID`, one method, ordered arguments, and optionally consumes one reply. Prefer a domain method such as `take_damage` over setting `health` dynamically because the receiver can enforce armor, death, and signal rules in one place.

## Why `methods!` Exists

`methods!` adds callable behavior methods to the generated script type. The macro rewrites methods that take `ctx: &mut ScriptContext<'_, API>` into generic Rust methods with the correct `where API: ScriptAPI + ?Sized` bound. Because the macro owns that rewrite, methods do not declare `<API: ScriptAPI>` themselves.

Use `methods!` for logic you want to call directly from lifecycle hooks or dynamically through `call_method!`.

Source path:

- `perro_source/script_stack/perro_scripting/src/macros.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_methods.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs`

## Method Shape

```rust
methods!({
    pub fn apply_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> bool {
        amount > 0
    }
});
```

| Part          | Requirement                                                |
| ------------- | ---------------------------------------------------------- |
| visibility    | `pub` when the method is reachable through `call_method!` or signals; omit for internal helpers |
| receiver      | `&self`                                                    |
| context       | `ctx: &mut ScriptContext<'_, API>`                         |
| custom params | any supported typed params after `ctx`                     |
| return        | `()` or any type that converts with `Variant::from(value)` |

## Visibility

The compiler only generates `call_method` dispatch glue for methods declared `pub` (any form — `pub`, `pub(crate)`, ...). A non-pub method stays a plain Rust method: you can still call it directly as `self.helper(ctx)`, but `call_method!` from any script — including dynamic self dispatch on `ctx.id` — resolves to `Variant::Null`, and signals cannot invoke it.

| Declaration | Direct call `self.x(ctx)` | `call_method!` | Signal handler |
| --- | --- | --- | --- |
| `pub fn` | yes | yes | yes |
| `fn` | yes | no | no |

Two rules follow:

- Signal handlers dispatch through the same generated glue, so every method wired with `signal_connect!`, `signal_connect_many!`, or `signal_connect_pairs!` must be `pub`.
- Keep internal helpers non-pub. Each `pub` method costs a dispatch match arm plus `Variant` param decode code in the compiled binary; private helpers compile to nothing extra.

`perro doctor` flags `call_method!` and signal connections that target a method with no `pub fn` definition and points at the file that defines it. It also flags the reverse: a `pub fn` that no `call_method!`, signal connection, or animation event references can drop `pub` to shed its dispatch glue.

## Direct Calls

Direct calls are normal Rust calls. Use them inside the same script when you know the method at compile time. A method used only this way does not need `pub`.

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
    // pub because Runtime Dispatch below also calls it thru call_method!
    pub fn apply_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> bool {
        amount > 0
    }
});
```

## Runtime Dispatch

Use `call_method!` for cross-script calls or dynamic calls by `ScriptMemberID`. This path passes `Variant` params and always returns a `Variant`.

If the called method returns `bool`, `i32`, `String`, etc. or engine types like `NodeID`, `MeshID` or any custom `#[derive(Variant)]` type, the generated script bridge wraps that value with `Variant::from(value)`. If the called method returns `()`, the bridge returns `Variant::Null`.

Primitive method returns still use typed Rust in the method body.

Because dispatch is dynamic, caller code must know the expected return type and decode it.

Use `call_method!(ctx.run, ctx.id, ...)` for dynamic self dispatch.

Use `call_method!(ctx.run, other_id, ...)` for cross-script dispatch.

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
    pub fn apply_hit(&self, ctx: &mut ScriptContext<'_, API>, hit: HitInfo) -> bool {
        hit.amount > 0
    }

    pub fn last_hit(&self, ctx: &mut ScriptContext<'_, API>) -> HitInfo {
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
