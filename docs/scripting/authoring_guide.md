# Script Authoring Guide

## Purpose

Use this page as the default standard for Perro gameplay scripts.

Scripts own behavior for one attached node. `#[State]` holds data for each
instance of that script. Scene files may override state fields before
`on_init` runs.

## Choose Script State

Put a value in `#[State]` when it belongs to one script instance and must
survive a callback:

- mutable gameplay values such as health, velocity, and mode
- cached runtime values used by later callbacks
- fixed node dependencies as `NodeID` or `Option<NodeID>`
- per-instance assets as typed IDs such as `TextureID` or `MeshID`

Keep constants as Rust constants. Keep values used by one callback as local
variables. Do not use state as a bag for stateless temporary results.

```rust
#[derive(Clone, Default, Variant)]
struct CharacterLook {
    portrait: TextureID,
    materials: Vec<MaterialID>,
}

#[State]
struct PlayerState {
    #[default = 100]
    #[expose]
    health: i32,

    #[expose]
    #[node_ref(Camera3D)]
    camera: Option<NodeID>,

    #[expose]
    look: CharacterLook,

    velocity: Vector3,
}
```

`#[expose]` organizes what is visible from the editor inspector. Any state field
may be set through scene `script_vars`, including fields inside derived custom
types.

Scene asset strings coerce to their typed resource IDs before `on_init`:

```text
script_vars = {
    health = 125,
    camera = @MainCamera,
    look = {
        portrait = "res://textures/player.png",
        materials = ["res://materials/body.rue", "res://materials/trim.rue"]
    }
}
```

This path supports `TextureID`, `MaterialID`, `MeshID`, `AnimationID`,
`AnimationTreeID`, `NavMeshID`, and `SoundFontID`. Coercion recurses through
options, lists, maps, tuples, and custom `#[derive(Variant)]` values. An absent
field or decode failure keeps the field default. Asset load failure keeps each
resource module's normal nil/failure behavior. Runtime `set_var!` stays strict:
it accepts the field's actual `Variant` type, not a resource path string.

## Choose A Node

`ctx.id` is the node that owns the current script instance.

Use the narrowest stable way to find another node:

1. Store a fixed scene dependency as a state `NodeID`.
2. Derive a structural dependency from the parent or children.
3. Query when membership is dynamic or many nodes match.

Do not search by name every frame for a dependency the scene already knows.
`#[node_ref(...)]` gives the editor and doctor a type hint; the runtime value
remains a `NodeID`.

Use `with_node!` for known typed reads and `with_node_mut!` for known typed
writes. `SelfNodeType` is an optional Rust alias, not an engine rule.

```rust
type SelfNodeType = CharacterBody3D;

let speed = with_node!(ctx.run, SelfNodeType, ctx.id, |node| {
    node.velocity.length()
});

let camera = with_state!(ctx.run, PlayerState, ctx.id, |state| state.camera);

if let Some(camera) = camera {
    with_node_mut!(ctx.run, Camera3D, camera, |node| {
        node.fov = 70.0;
    });
}
```

Use node-base helpers for shared identity, hierarchy, and transform behavior.
Use a concrete node type when editing type-specific fields.

```rust
let parent_id = get_node_parent_id!(ctx.run, ctx.id);

with_base_node_mut!(ctx.run, Node2D, parent_id, |base| {
    base.position.x += 1.0;
});
```

Treat refs as optional at runtime. A target may be absent, removed, or have the
wrong type. Skip work on a missing optional ref; gameplay code need not panic or
log every optional miss.

## Choose Typed Or Dynamic Script Access

Use `with_state!` and `with_state_mut!` when the state type is known. Use a
normal Rust helper or direct method call for behavior inside the same script.

Use `get_var!`, `set_var!`, and `call_method!` when the target script or member
is selected at runtime. Dynamic calls return `Variant`; decode the expected
type at the call site.

Use methods for a targeted command with known receiver, arguments, and an
optional return value. Use signals first for events, fan-out, and loose or
cross-scene coordination.

## Choose A Timer

Use a named timer for one-shot delays and cooldown completion. Connect its
finished signal to a method. The runtime stores one active timer per name;
starting that name again resets its deadline. Use distinct names for work that
must run concurrently.

Keep a state clock only when code needs continuous progress each frame, such as
an animation blend or HUD countdown.

## Keep Runtime Borrows Short

Runtime helpers borrow `ctx.run` for the duration of their closure. Never call
another `ctx.run` API inside a `with_state!`, `with_state_mut!`, `with_node!`, or
`with_node_mut!` closure.

Copy or clone the required value out, end the closure, and make the next call:

```rust
let emit_phase_two = with_state_mut!(ctx.run, BossState, ctx.id, |state| {
    let emit = !state.phase_two && state.health <= 50.0;
    state.phase_two |= emit;
    emit
}).unwrap_or(false);

if emit_phase_two {
    signal_emit!(ctx.run, signal!("boss_phase_two"), params![]);
}
```

## Script Boundaries

Split scripts around behavior ownership, not a fixed size rule. Keep one
cohesive behavior with the node that owns it. Move scene-wide orchestration to
a controller script. Put shared constants, math, and pure transforms in normal
Rust modules without `#[State]`.

## See Also

- [Script State](state.md)
- [Script Methods](methods.md)
- [Runtime Nodes](contexts/runtime_modules/nodes.md)
- [Query System](query_system.md)
- [Signals](contexts/runtime_modules/signals.md)
- [Time And Named Timers](contexts/runtime_modules/time.md)
- [Variant](variant.md)
