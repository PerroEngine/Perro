# Scripting Overview

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Mental Model | [Mental Model](#mental-model) |
| Scripting Group | [Scripting Group](#scripting-group) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use this area for all script-authored game logic.

This includes state, lifecycle hooks, custom methods, runtime node access, cross-script calls, input, resources, and `Variant` conversion.

The book explains why Perro uses these shapes.

The docs give exact macro/API paths and edge behavior.

## Mental Model

One script instance has one owner: the node in `ctx.id`.

The node stores scene data. `#[State]` stores script-owned per-instance data. Lifecycle hooks choose when work runs. Methods target one receiver. Signals announce an event without choosing every receiver. Queries discover a set that changes at runtime. `Variant` crosses boundaries where the concrete Rust type is not known to the caller.

Keep those roles separate. A fixed camera belongs in a scene-injected `NodeID`; it is not a query. Typed health belongs behind `with_state!`; it is not a string lookup. A coin-collected event belongs in a signal when several independent systems may react; it is not a chain of hard-coded calls.

## Scripting Group

| Task | Page |
| --- | --- |
| Follow script authoring standards | [Script Authoring Guide](authoring/index.md) |
| See scripts work together | [Script Teamwork Examples](authoring/examples/index.md) |
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

| Situation | Choose | Why | Tradeoff |
| --- | --- | --- | --- |
| Controller owns health, velocity, or cooldown data | `#[State]` + typed state access | Value lives with one script instance and keeps its Rust type | Other script types need a shared type or dynamic boundary |
| Switch knows one door | scene-injected `NodeID` + `call_method!` | Dependency and receiver stay explicit | Caller depends on method name and return schema |
| Coin must notify HUD, audio, and achievements | signal | Producer stays independent from current listeners | Connection lifetime and payload schema need ownership |
| Manager needs every currently spawned enemy | query | Result reflects runtime membership | Query costs more and gives weaker guarantees than a fixed ref |
| Tool knows a member name but not its Rust state type | `get_var!` / `set_var!` | `Variant` supports runtime-selected members | Decode may fail; typed access is safer and cheaper |
| Pathfinding or bulk scoring costs too much in one frame | job | CPU work leaves the frame callback | Inputs/results must cross a task boundary and cannot borrow runtime state |

Choose a context by role: `ctx.run` for runtime state, nodes, scenes, scripts, signals, time, and window calls; `ctx.res` for resources and data; `ctx.ipt` for input.

## Example

For fixed dependencies, store a `NodeID` in state and inject it from the scene.
Use a query only when the set is dynamic:

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

- [Script Authoring Guide](authoring/index.md)
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
