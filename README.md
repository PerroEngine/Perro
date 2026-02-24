# Perro Game Engine

<div align="center">
  <img src="icon.png" alt="Perro Logo" width="200"/>
</div>

**Perro** is an experimental, open-source game engine written in **Rust**, built as a modern alternative to engines like Unreal, Godot, and Unity. It focuses on **simplicity** of making games without sacrificing **performance**.

## Rust as a Scripting Language

While **Rust** is typically a general-purpose systems language, Perro uses it as a scripting language through a structured authoring model. Scripts are organized into clear sections: a target node type, `#[State]` data, lifecycle entry points (`lifecycle!`), and callable behavior methods (`methods!`).

This structure makes it explicit when state is read or mutated, and how nodes are accessed at runtime.

For more details, see the full documentation: [perroengine.com/docs](https://www.perroengine.com/docs).

<pre><code class="language-rust">
use perro_runtime_context::prelude::*;
use perro_nodes::prelude::*;
use perro_structs::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_resource_context::prelude::*;
use perro_scripting::prelude::*;

<span style="color:#C586C0;">type</span> <span style="color:#4EC9B0;">SelfNodeType</span> = <span style="color:#4EC9B0;">Node2D</span>;

<span style="color:#C586C0;">#[State]</span>
<span style="color:#C586C0;">pub struct</span> <span style="color:#4EC9B0;">ExampleState</span> {
    <span style="color:#C586C0;">#[default = 0]</span>
    count: i32,
}

<span style="color:#DCDCAA;">lifecycle!</span>({
    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">on_init</span>(&<span style="color:#569CD6;">self</span>, ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {
        // Read state
        <span style="color:#C586C0;">let</span> count = <span style="color:#DCDCAA;">with_state!</span>(ctx, <span style="color:#4EC9B0;">ExampleState</span>, <span style="color:#9CDCFE;">self_id</span>, |state| state.count)
            .<span style="color:#DCDCAA;">unwrap_or_default</span>();
        <span style="color:#DCDCAA;">log_info!</span>(count);
    }

    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">on_all_init</span>(&<span style="color:#569CD6;">self</span>, _ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">_self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {}

    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">on_update</span>(&<span style="color:#569CD6;">self</span>, ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {
        // Mutate state
        <span style="color:#DCDCAA;">with_state_mut!</span>(ctx, <span style="color:#4EC9B0;">ExampleState</span>, <span style="color:#9CDCFE;">self_id</span>, |state| {
            state.count += 1;
        });

        // Read node
        <span style="color:#C586C0;">let</span> _x = <span style="color:#DCDCAA;">with_node!</span>(ctx, <span style="color:#4EC9B0;">SelfNodeType</span>, <span style="color:#9CDCFE;">self_id</span>, |<span style="color:#9CDCFE;">node</span>| <span style="color:#9CDCFE;">node</span>.position.x)
            .<span style="color:#DCDCAA;">unwrap_or_default</span>();

        // Mutate node
        <span style="color:#DCDCAA;">with_node_mut!</span>(ctx, <span style="color:#4EC9B0;">SelfNodeType</span>, <span style="color:#9CDCFE;">self_id</span>, |<span style="color:#9CDCFE;">node</span>| {
            <span style="color:#9CDCFE;">node</span>.position.x += 1.0;
        });
    }

    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">on_fixed_update</span>(&<span style="color:#569CD6;">self</span>, _ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">_self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {}

    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">on_removal</span>(&<span style="color:#569CD6;">self</span>, _ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">_self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {}
});

<span style="color:#DCDCAA;">methods!</span>({
    <span style="color:#C586C0;">fn</span> <span style="color:#DCDCAA;">reset_count</span>(&<span style="color:#569CD6;">self</span>, ctx: &mut <span style="color:#4EC9B0;">RuntimeContext</span>&lt;'_, RT&gt;, _res: &<span style="color:#4EC9B0;">ResourceContext</span>&lt;'_, RS&gt;, <span style="color:#9CDCFE;">self_id</span>: <span style="color:#4EC9B0;">NodeID</span>) {
        <span style="color:#DCDCAA;">with_state_mut!</span>(ctx, <span style="color:#4EC9B0;">ExampleState</span>, <span style="color:#9CDCFE;">self_id</span>, |state| {
            state.count = 0;
        });
    }
});
</code></pre>

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
