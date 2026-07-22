# Example: Generic Inspector Reads And Writes Vars

Use `get_var!` and `set_var!` when a generic tool knows a target node and member
name but does not know the target's Rust state type.

## Goal, Owners, And Flow

```text
tool data(member name) -> adapter.target NodeID
-> get_var(member) -> transform Variant -> set_var(member)
```

The target script owns its values. The adapter owns generic transformation and
knows neither the target state type nor the member until runtime. The scene
injects the fixed target, while tool data supplies the dynamic name.

```rust
#[State]
struct RuntimeInspectorState {
    #[expose]
    #[node_ref(Node2D, Node3D, UiNode)]
    target: Option<NodeID>,
}

lifecycle!({});

methods!({
    fn add_i32(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        member: String,
        amount: i32,
    ) {
        let Some(target) = with_state!(
            ctx.run,
            RuntimeInspectorState,
            ctx.id,
            |state| state.target
        ).unwrap_or_default() else {
            return;
        };

        let old = get_var!(ctx.run, target, member.as_str())
            .as_i32()
            .unwrap_or(0);

        set_var!(
            ctx.run,
            target,
            member.as_str(),
            variant!(old + amount)
        );
    }
});
```

For a known `State`, prefer `with_state_mut!`. Dynamic vars fit generic cross script calls.

`set_var!` performs strict runtime decode. A string such as
`"res://textures/icon.png"` does not load into `TextureID`; asset path coercion
only applies while scene vars load.

## Failure And Alternatives

Missing target returns without work. Missing/wrong-type members use a deliberate
fallback here; production tools should surface that mismatch to their caller.
For known `State`, typed mutation is clearer, faster, and compiler-checked.
Use a targeted method instead when the change must preserve target invariants.

Extend the adapter with an allow-list, expected schema, or undo record. Runnable
source: [adapter.rs](../../../../demos/ScriptPatterns/res/scripts/adapter.rs).

[Back To Examples](index.md)
