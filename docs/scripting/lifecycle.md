# Script Lifecycle

## Page Map

| Header | Link |
| --- | --- |
| Why `lifecycle!` Exists | [Why `lifecycle!` Exists](#why-lifecycle-exists) |
| Hook Signatures | [Hook Signatures](#hook-signatures) |
| Hooks | [Hooks](#hooks) |
| Examples | [Examples](#examples) |

## Why `lifecycle!` Exists

`lifecycle!` declares engine-driven script entry points. It expands to an `impl<API> ScriptLifecycle<API>` block for the generated script type. Because the macro owns the generic `impl<API>`, hook functions use `ScriptContext<'_, API>` but do not declare `<API: ScriptAPI>` themselves.

Use lifecycle hooks for work the engine calls automatically: setup, per-frame logic, fixed-step logic, and cleanup.

## Hook Signatures

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {}
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {}
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {}
    fn on_fixed_update(&self, ctx: &mut ScriptContext<'_, API>) {}
    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {}
});
```

| Hook | Runs | Use when |
| --- | --- | --- |
| `on_init` | script instance creation | cache node IDs, load resources, initialize state |
| `on_all_init` | after all script instances initialize | resolve links to other scripts/nodes that must already exist |
| `on_update` | once per rendered frame | input, animation control, visual/gameplay updates |
| `on_fixed_update` | fixed timestep | physics-style deterministic updates |
| `on_removal` | before script/node removal completes | disconnect signals, stop sounds, release references |

## Hooks

### `on_init`

Signature: `fn on_init(&self, ctx: &mut ScriptContext<'_, API>) -> ()`

Use it when this script needs startup work for its own node.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let texture = texture_load!(ctx.res, "res://textures/player.png");
        with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            state.texture = texture;
        });
    }
});
```

### `on_all_init`

Signature: `fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) -> ()`

Use it when setup depends on other scripts or child nodes that must already exist.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let children = get_node_children_ids!(ctx.run, ctx.id);
        let _ = children;
    }
});
```

### `on_update`

Signature: `fn on_update(&self, ctx: &mut ScriptContext<'_, API>) -> ()`

Use it for frame logic, input edges, and visual state.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        if key_down!(ctx.ipt, KeyCode::KeyD) {
            let step = Vector2::new(160.0 * dt, 0.0);
            if let Some(pos) = get_local_pos_2d!(ctx.run, ctx.id) {
                set_local_pos_2d!(ctx.run, ctx.id, pos + step);
            }
        }
    }
});
```

### `on_fixed_update`

Signature: `fn on_fixed_update(&self, ctx: &mut ScriptContext<'_, API>) -> ()`

Use it for fixed-step simulation work.

```rust
lifecycle!({
    fn on_fixed_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = fixed_delta_time!(ctx.run);
        let _ = dt;
    }
});
```

### `on_removal`

Signature: `fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) -> ()`

Use it for cleanup that belongs to the script instance.

```rust
lifecycle!({
    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {
        audio_stop_all!(ctx.res);
    }
});
```

## Examples

Use free helper functions only outside `lifecycle!`; those helpers must declare their generic.

```rust
fn read_dt<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>) -> f32 {
    delta_time!(ctx.run)
}
```
