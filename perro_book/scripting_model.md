# Scripting Model

Perro scripts split stable behavior from per-node state.

## Goal

Know why scripts look like small Rust modules instead of classes.

## Decision Model

Ask three questions before writing a runtime call:

1. Who owns the data: this script state, a node, or another script?
2. Is the target fixed, structural, or selected at runtime?
3. Does the flow need a return value, fan-out, or dynamic member selection?

Known state and node types use typed access. A fixed target command uses a
method. A loose event uses a signal. `get_var!` and `set_var!` stay for adapters,
tools, and systems whose member name is data. This makes dynamic dispatch an
explicit boundary instead of the default scripting style.

## Behavior And State

Behavior is shared.

State is per attached node.

The behavior object holds generated dispatch code:

- lifecycle hooks
- method table
- `get_var`
- `set_var`
- `call_method`

Each node with the script gets its own state object from `create_state`.

This lets Perro keep behavior cheap to call and state local to one script instance.

Use state for per-instance mutable values, cached runtime values, fixed node
refs, and per-instance typed asset IDs. Keep constants and callback-local
temporary values outside state.

`#[expose]` only controls editor inspector organization. Any state field that
supports `Variant` conversion may receive a scene override.

Source path:

- `perro_source/script_stack/perro_scripting/src/script_trait.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs`

## `ctx.id`

`ctx.id` is this script's node id.

Use it for self state and self node access.

```rust
let speed = with_state!(ctx.run, PlayerState, ctx.id, |state| state.speed);
```

`ctx.id` stays valid for the callback because the runtime is calling that script on that node.

IDs from parents, children, queries, scene loads, or state refs are handles.

They work while the target node exists.

The main runtime failure is stale id, wrong node type, or removed target.

## State Closures

`with_state!` borrows typed state for the closure body and returns a value.

`with_state_mut!` borrows typed state mutably for the closure body.

```rust
let hp = with_state!(ctx.run, PlayerState, ctx.id, |state| state.health);

with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
    state.health -= 10.0;
});
```

The closure shape prevents long state borrows from escaping.

It also keeps the runtime free to call more APIs after the closure ends.

## Methods

Use `methods!` for behavior other scripts, signals, animation events, or tools call.

You write typed Rust params and typed Rust returns.

Generated glue converts params from `Variant` and converts the return into `Variant`.

```rust
methods!({
    fn damage(&self, ctx: &mut ScriptContext<'_, API>, amount: f32) -> bool {
        with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            state.health -= amount;
            state.health <= 0.0
        }).unwrap_or(false)
    }
});
```

Cross-script calls return `Variant`.

Parse when the caller needs the typed value.

```rust
let result = call_method!(ctx.run, enemy, method!("damage"), params![12.0_f32]);
let dead = result.parse::<bool>().unwrap_or(false);
```

For same-script work, call a normal Rust helper directly when possible.

Use `call_method!(ctx.run, ctx.id, ...)` only when dynamic self dispatch is useful.

## Node Refs

State can store `NodeID`.

Use `#[node_ref(...)]` to tell editor and doctor what type the id should point at.

```rust
#[State]
pub struct CameraRigState {
    #[expose]
    #[node_ref(Camera3D)]
    camera: NodeID,
}
```

Runtime type stays `NodeID`.

The hint helps scene authoring and validation.

Prefer a state ref for a fixed dependency, a parent/child relation for a
structural dependency, and a query for a dynamic set. Treat optional refs as
optional at runtime; skip work when the target is absent or no longer matches
the expected type.

## Asset Refs

Scene `script_vars` may assign a resource path string to a typed `TextureID`,
`MaterialID`, `MeshID`, `AnimationID`, `AnimationTreeID`, `NavMeshID`, or
`SoundFontID` field. Resolution happens before `on_init` and recurses through
options, collections, tuples, and custom `#[derive(Variant)]` values.

Runtime `set_var!` remains strict and expects the typed value.

## Calls And Events

Use typed `with_state!` access when the state type is known. Use dynamic
`get_var!`, `set_var!`, and `call_method!` only when the member or target script
is selected at runtime.

Use a method for a targeted command and optional return value. Use a signal for
an event, fan-out, or loose cross-scene coordination.

Use named timers for one-shot delays and cooldown completion. One timer exists
per name; restarting it resets the deadline. Keep a state clock when gameplay
needs continuous progress each frame.

## Borrow Rule

Never call another `ctx.run` API inside a state or node access closure. Copy or
clone data out, let the closure end, and make the next runtime call.

## Reference

- [Verified ScriptPatterns Flow](../demos/ScriptPatterns/README.md)
- [Script State](/docs/scripting/state)
- [Script Authoring Guide](/docs/scripting/authoring/index)
- [Script Methods](/docs/scripting/methods)
- [Variant](/docs/scripting/variant)
- [Scripts Module](/docs/scripting/contexts/runtime_modules/scripts)
