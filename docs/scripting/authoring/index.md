# Script Authoring Guide

Use this guide as the default design standard for Perro gameplay scripts. It
explains ownership and communication choices before listing macros. The goal is
code whose data source, target, lifetime, and failure behavior are visible.

## Mental Model

One script instance belongs to one node. `ctx.id` identifies that owner.
`#[State]` holds values that survive callbacks. A scene wires known dependencies
and per-instance assets through `script_vars` before `on_init`. Runtime calls
cross an ownership boundary only when the target owns the behavior or data.

```text
scene construction -> script_vars -> on_init -> on_all_init -> update callbacks
owned node <-> owned typed state -> fixed refs / relations / queries -> other owners
```

## Guide Map

| Need | Use |
| --- | --- |
| choose state fields, node refs, or asset IDs | [State And References](state_and_refs.md) |
| edit self, another node, or another script | [Node And State Access](node_and_state_access.md) |
| choose typed state, methods, signals, or dynamic vars | [Script Communication](communication.md) |
| use timers and avoid nested runtime borrows | [Timers And Borrows](timers_and_borrows.md) |
| choose a callback and understand init order | [Lifecycle](lifecycle.md) |
| wire scenes and assign ownership | [Ownership And Scene Wiring](ownership_and_scene_wiring.md) |
| choose fixed refs, relations, or queries | [References And Queries](references_and_queries.md) |
| inject typed assets and understand lifetime | [Typed Assets](typed_assets.md) |
| spawn nodes or attach scripts at runtime | [Spawn And Runtime Attach](spawn_and_runtime_attach.md) |
| split scripts, debug, test, and check perf | [Boundaries And Quality](boundaries_and_quality.md) |
| see several scripts form one feature | [Examples](examples/index.md) |
| write or review docs examples | [Documentation Standard](documentation_standard.md) |

## Core Rules

- use `ctx.id` for the node that owns the current script
- store fixed dependencies as scene-injected `NodeID` fields
- use parent/child relations for structural dependencies
- use queries for dynamic sets, not fixed refs
- use `with_state!` / `with_state_mut!` when the Rust state type is known
- use `call_method!` for a targeted dynamic command with params or a return value
- use signals for events, fan-out, and loose or cross-scene flow
- use `get_var!` / `set_var!` only when the member name or type is dynamic
- use named timers for delays and cooldown completion
- copy values out of runtime closures before the next `ctx.run` call
- split scripts by behavior ownership, not a fixed size rule

## Communication Choice

```text
known Rust state type? -> with_state! / with_state_mut!
targeted behavior?     -> call_method!
event or many listeners? -> signal_emit!
runtime member name?   -> get_var! / set_var!
```

## Full Examples

- [Player, Camera, And HUD](examples/player_camera_hud.md)
- [Switch Calls Door](examples/call_method.md)
- [Manager And Spawned Enemies](examples/spawned_enemies.md)
- [Pickup, Inventory, And UI](examples/pickup_flow.md)
- [Scene-Injected Asset Variants](examples/asset_variants.md)
- [Timer-Driven Cooldown](examples/cooldown.md)
- [Dynamic Inspector Adapter](examples/dynamic_vars.md)

The runnable [ScriptPatterns demo](../../../demos/ScriptPatterns/README.md)
combines fixed refs, typed asset injection, methods, signal fan-out, dynamic
vars, a named timer, typed node access, and borrow-safe flow.

## API References

- [State](../state.md)
- [Methods](../methods.md)
- [Signals](../contexts/runtime_modules/signals.md)
- [Scripts Runtime Module](../contexts/runtime_modules/scripts.md)
- [Nodes Runtime Module](../contexts/runtime_modules/nodes.md)
- [Named Timers](../contexts/runtime_modules/time.md)
- [Variant](../variant.md)
