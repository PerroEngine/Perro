# Scripting Overview

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Scripting Group | [Scripting Group](#scripting-group) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use this area for all script-authored game logic.

This includes state, lifecycle hooks, custom methods, runtime node access, cross-script calls, input, resources, and `Variant` conversion.

The book explains why Perro uses these shapes.

The docs give exact macro/API paths and edge behavior.

## Scripting Group

| Task | Page |
| --- | --- |
| Write first script | [Project Script Modules](project_modules.md) |
| Store per-node data | [Script State](state.md) |
| Run engine callbacks | [Script Lifecycle](lifecycle.md) |
| Add callable methods | [Script Methods](methods.md) |
| Read/mutate nodes at runtime | [Runtime Nodes Module](contexts/runtime_modules/nodes.md) |
| Query nodes | [Query System](query_system.md) |
| Call self/cross-script methods | [Scripts Module](contexts/runtime_modules/scripts.md) |
| Convert dynamic values | [Variant](variant.md) |
| Read input | [Input API](contexts/input_api.md) |
| Load/use resources | [Resource API](contexts/resource_api.md) |
| Run CPU work in parallel | [Parallel Jobs](jobs.md) |

## Use Cases

These docs cover the game logic you write in Perro script files under `res/**/*.rs`:

- Player and enemy controllers: read input each frame in `on_update` (`ctx.ipt`), move with the physics macros, and keep velocity and timers in `#[State]`.
- Per-node gameplay data (health, ammo, cooldowns, coins): store it in `#[State]`, read/write it with `with_state!` / `with_state_mut!`, and expose it to other systems with `get_var!` / `set_var!`.
- Cross-object coordination (a switch opening a door, an enemy alerting its squad): call behavior with `call_method!` or broadcast with signals.
- Dynamic groups instead of fixed refs (every awake enemy, pickups in a room): `query!` and its helpers.
- Heavy CPU work off the frame path (pathfinding, procedural generation, bulk scoring): `jobs::spawn` / `jobs::join` / `jobs::par_map`.

Choose a context by role: `ctx.run` for runtime state, nodes, scenes, scripts, signals, time, and window calls; `ctx.res` for resources and data; `ctx.ipt` for input.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if action_pressed!(ctx.ipt, "interact") {
            for door in query!(ctx.run, all(tag["door"])) {
                let ret = call_method!(ctx.run, door, method!("toggle"), params![]);
                let opened = ret.parse::<bool>().unwrap_or(false);
                log_info!("door {:?} open {}", door, opened);
            }
        }
    }
});
```

## Reference

# Scripting Overview

Perro scripts are authored in Rust and compiled into script modules.
Perro manages most glue code for you, so scripting stays close to normal Rust instead of turning into registration boilerplate.

Core pieces:

- `#[State]` data struct
- `lifecycle!` for engine entry points
- `methods!` for callable behavior methods
- bare Rust modules for shared code (`res/**.rs` with no script behavior)
- script contexts (`RuntimeWindow`, `ResourceWindow`, `InputWindow`)

Borrow rule:

- `ctx.run` uses mutable runtime access.
- Runtime macros borrow `ctx.run` for duration of macro call.
- Do not use `ctx.run` again inside `with_state_mut!`, `with_node_mut!`, or similar closure.
- Pull copy data out first (`f32`, `NodeID`, ids, bools, enums, small math types).
- If data owns heap content (`String`, `Vec`, `Cow`, custom clone types), clone out b4 closure if later code still needs it.
- Clone cost stays local; tmp clone drops aft closure/use site.

Script dependencies:

- Add extra crates to `deps.toml` in your project root under `[dependencies]`.
- On `perro check`, `perro dev`, and `perro build`, Perro merges those entries into `.perro/scripts/Cargo.toml`.
- Keep `perro` managed by Perro; do not override it in `deps.toml`.

See:

- [Project Script Modules](project_modules.md)
- [Parallel Jobs](jobs.md)
- [Script Contexts](contexts/README.md)
- [Script Utility Modules](modules.md)
- [Struct Types](structs/index.md)
- [Node Types](nodes.md)
- [Physics Nodes](physics_nodes.md)
- [Audio Nodes](audio_nodes.md)
- [Water Bodies](water.md)
- [Node Collections](node_collections.md)
  - In-code scene trees, flat batches, child collections, and `create_nodes!`.
- [Script State](state.md)
- [Script Lifecycle](lifecycle.md)
- [Script Methods](methods.md)
- [Variant](variant.md)
