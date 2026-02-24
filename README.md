# Perro Game Engine

<div align="center">
  <img src="icon.png" alt="Perro Logo" width="200"/>
</div>

**Perro** is an experimental, open-source game engine written in **Rust**, built as a modern alternative to engines like Unreal, Godot, and Unity. It focuses on **simplicity** of making games without sacrificing **performance**.

## Rust as a Scripting Language

While **Rust** is typically a general-purpose systems language, Perro uses it as a scripting language through a structured authoring model. Scripts are organized into clear sections: a target node type, `#[State]` data, lifecycle entry points (`lifecycle!`), and callable behavior methods (`methods!`).

This structure makes it explicit when state is read or mutated, and how nodes are accessed at runtime.

For more details, see the full documentation: [perroengine.com/docs](https://www.perroengine.com/docs).

```rust
use perro_runtime_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_resource_context::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct ExampleState {
    #[default = 0]
    count: i32,
}

lifecycle!({
    fn on_init(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, self_id: NodeID) {
        // Read state
        let count = with_state!(ctx, ExampleState, self_id, |state| state.count)
            .unwrap_or_default();
        log_info!(count);
    }

    fn on_all_init(&self, _ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self_id: NodeID) {}

    fn on_update(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, self_id: NodeID) {
        // Mutate state
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.count += 1;
        });

        // Read node
        let _x = with_node!(ctx, SelfNodeType, self_id, |node| node.position.x)
            .unwrap_or_default();

        // Mutate node
        with_node_mut!(ctx, SelfNodeType, self_id, |node| {
            node.position.x += 1.0;
        });
    }

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self_id: NodeID) {}

    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, _self_id: NodeID) {}
});

methods!({
    fn reset_count(&self, ctx: &mut RuntimeContext<'_, RT>, _res: &ResourceContext<'_, RS>, self_id: NodeID) {
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.count = 0;
        });
    }
});
```

## Contributions

Perro is, of course, **open source**, and contributions are always appreciated: issue reports, new features, system optimizations, and other improvements. Everyone is welcome to join the project.

## Support Perro

Donations help fund full-time development, faster features, and better tooling. If you want to support the project:

- [Support Directly](https://perroengine.com/sponsor)
- [Support on Ko-fi](https://ko-fi.com/perroengine)

---

## License

Perro is licensed under the **Apache 2.0 License**. See [LICENSE](LICENSE) for details.

---
