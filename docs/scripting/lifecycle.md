# Script Lifecycle

Lifecycle hooks are declared in `lifecycle!({ ... })`.

Available hooks:
- `on_init`: runs when this script instance is created
- `on_all_init`: runs after all scripts have initialized
- `on_update`: runs every frame
- `on_fixed_update`: runs on fixed timestep
- `on_removal`: runs when script/node is removed

## Example

```rust
lifecycle!({
    fn on_init(&self, _ctx: &mut ScriptContext<'_, RT, RS, IP>) {}

    fn on_update(&self, ctx: &mut ScriptContext<'_, RT, RS, IP>) {
        let dt = delta_time!(ctx.run);
        with_node_mut!(ctx.run, Node2D, ctx.id, |node| {
            node.position.x += dt * 5.0;
        });
    }
});
```

## Methods vs Lifecycle

Use `methods!` for reusable callable logic (including `call_method!` targets).
Use lifecycle methods for engine-driven entry points.

Related:
- [Script Contexts](contexts/README.md) for callback signatures and macro convention.
- [Script Methods](methods.md) for reusable/callable method bodies.


