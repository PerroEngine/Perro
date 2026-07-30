# Script State

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Mental Model  | [Mental Model](#mental-model)   |
| Use Cases     | [Use Cases](#use-cases)         |
| State Struct  | [State Struct](#state-struct)   |
| Visibility    | [Visibility](#visibility)       |
| Editor Expose | [Editor Expose](#editor-expose) |
| Node Ref Hints | [Node Ref Hints](#node-ref-hints) |
| Defaults      | [Defaults](#defaults)           |
| Runtime Vars  | [Runtime Vars](#runtime-vars)   |
| Custom Types  | [Custom Types](#custom-types)   |
| Practical Example | [Practical Example](#practical-example) |

## Purpose

Script state stores per-node data for one script instance.

Each node with that script gets its own state value. Use state for mutable gameplay data, cached runtime values, fixed node refs, typed asset IDs, scene overrides, and values other scripts need to read or write.

Keep constants outside state. Keep temporary values local when they do not need to survive the callback.

Behavior is separate from state.

The generated behavior object owns lifecycle/method dispatch, while each attached node owns a separate state value.

Source path:

- `perro_source/script_stack/perro_scripting/src/script_trait.rs`
- `perro_source/script_stack/perro_scripting_macros/src/lib.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs`

## Mental Model

State answers: "what must this script instance remember after this callback returns?"

Put mutable per-instance values, cached results, fixed `NodeID` dependencies, and typed asset IDs in `#[State]`. Keep constants at module scope. Keep one-callback calculations in locals. Keep node transform/render fields on the node type instead of copying them into script state.

Scene `script_vars` inject only `pub` state fields before `on_init`; `#[expose]` only decides whether the editor inspector lists a field. Scene asset paths may decode into supported typed asset IDs during this scene-only injection path. Runtime `set_var!` remains strict and expects the correct `Variant` kind.

Runtime cross-script access (`get_var!` / `set_var!`) exists only for fields declared `pub` — see [Visibility](#visibility).

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Health or ammo must survive frames per actor | state field | Lifetime matches the script instance | Access must respect state borrow scope |
| Designer tunes speed in the inspector | `pub` + `#[expose]` state field | Editor lists the value while scene injection supplies it | Attribute does not validate or gate runtime writes |
| Actor always uses one camera or spawn point | `Option<NodeID>` + `#[node_ref(...)]` | Scene owns wiring and missing refs stay safe | Ref can become stale if target is removed |
| Scene chooses one texture per instance | `pub` typed asset ID field + path injection | Runtime starts with a stable cached ID | Path coercion exists only during scene injection |
| Generic UI reads a field by name | `pub` field + `get_var!` / `set_var!` | Caller need not know the state type | Name/type mismatch returns the API failure value |
| Field is internal bookkeeping only | non-pub field + `with_state!` | No glue generated at all, smaller binary | Not reachable from scenes or other scripts |
| Waypoints never cross a dynamic boundary | ordinary Rust collection in state | No `Variant` conversion needed | Dynamic callers cannot inspect it unless its type supports `Variant` |

## State Struct

Use `#[State]` on one struct in the script.

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default(100.0)]
    #[expose]
    pub health: f32,

    #[default(240.0)]
    #[expose]
    pub speed: f32,

    velocity: Vector2,
    grounded: bool,
    jump_buffer_timer: f32,
}
```

`#[State]` generates `Default` for the struct.

Fields without `#[default(...)]` use `Default::default()`.

In this example scenes tune `health` and `speed`, so both are `pub`. Only the script itself touches `velocity`, `grounded`, and `jump_buffer_timer`, so they stay non-pub and the compiler generates no glue for them.

## Visibility

The compiler generates dynamic glue only for fields declared `pub` (any form — `pub`, `pub(crate)`, ...). A non-pub field is purely internal:

- still works with `with_state!` / `with_state_mut!` inside its own script,
- receives no scene `script_vars` injection and no `.panim` `set_var` events — an authored value naming it does not apply,
- returns `Variant::Null` from `get_var!` and ignores `set_var!` / `broadcast_var!` — from every script, including dynamic self access on `ctx.id`.

| Declaration | `with_state!` (own script) | Scene `script_vars` | `get_var!` / `set_var!` |
| --- | --- | --- | --- |
| `pub speed: f32` | yes | yes | yes |
| `speed: f32` | yes | no | no |

Mark a field `pub` when a scene or `.panim` sets it, or when another script (or a generic system addressing fields by name) needs it. Every `pub` field costs get/set match arms plus `Variant` conversion code in the compiled binary; private fields compile to no glue at all.

The compiler also statically scans every `.scn` and `.panim` at build time and only emits scene-injection arms for `pub` fields a scene actually sets; every build re-scans, so scene edits need no manual step. Runtime spawns that attach scripts with vars (`node_collection!` script specs, `script_attach_with_vars`) still reach any `pub` field — unmatched vars route through the strict `set_var` path.

Nested members ride their root field: when `pub config: Tuning` is exposed, `get_var!(..., var!("config.speed"))` resolves through the root's variant tree regardless of `Tuning`'s inner field visibility.

`perro doctor` flags `get_var!` / `set_var!` / `broadcast_var!` calls that reference a field with no `pub` definition anywhere and points at the file that defines it. It warns `scene var private` when a scene `script_vars` entry targets a non-pub field — that value will not apply. It also flags the reverse: a `pub` field that nothing references dynamically — no access macro, scene `script_vars` entry, or animation event — can drop `pub` to shed its generated glue.

## Editor Expose

`#[expose]` is an editor marker.

The engine state path ignores it. An exposed field must also be `pub` for the editor-authored scene value to apply.

The Perro editor reads the source text under `#[State]` and shows only fields with `#[expose]` in the inspector.

Use it for values you want to tune in the editor without recompiling, and for scene refs like `NodeID` that are easier to wire from the inspector.

```text
script = "res://scripts/player.rs"
script_vars = {
    health = 75.0,
    speed = 300.0
}
```

Fields without `#[expose]` stay hidden from the editor inspector.

Use this for internal values like velocity, timers, cached refs, and state flags.

## Node Ref Hints

Use `#[node_ref(...)]` on `NodeID` fields to tell editor and doctor which node types are expected.

Runtime type stays `NodeID`.

The hint only affects inspector pick lists and doctor/clippy warnings.

Use hints when a state field points to a scene node with a required type.

The runtime still resolves the id at use site, so removed or wrong-type nodes can still fail API calls.

```rust
#[derive(Clone, Copy, Variant)]
pub struct RigRefs {
    #[node_ref(Skeleton3D)]
    pub skeleton: NodeID,
}

#[State]
pub struct PlayerState {
    #[expose]
    #[node_ref(Camera2D, Camera3D)]
    pub camera: NodeID,

    #[expose]
    #[node_ref(Node3D)]
    pub aim_target: NodeID,

    #[expose]
    pub rig: RigRefs,
}
```

Scene overrides still use normal node refs.

```text
script_vars = {
    camera = @MainCamera,
    aim_target = @AimMarker,
    rig = { skeleton = @HeroSkeleton }
}
```

Inspector filters node picker by hint.

Doctor warns when scene ref target does not match.

Built-in scene node fields use same hint model.

Examples:

- `CameraStream*.camera` accepts `Camera2D` or `Camera3D`.
- `UiCameraStream.camera` accepts `Camera2D` or `Camera3D`.
- `MeshInstance3D.skeleton` accepts `Skeleton3D`.
- 2D skeleton helper fields accept `Skeleton2D`.
- 3D skeleton helper fields accept `Skeleton3D`.

## Defaults

Use `#[default(...)]` to set the initial value.

```rust
#[State]
pub struct SpinnerState {
    #[default(6.0)]
    #[expose]
    pub turn_speed: f32,

    #[expose]
    pub target: NodeID,

    #[default(false)]
    paused: bool,
}
```

Both `#[default(expr)]` and `#[default = expr]` are accepted.

`#[expose]` can appear before or after `#[default(...)]`.

Scene `script_vars` override defaults after state creation.

Only `pub` fields receive scene overrides. `#[expose]` is not a gate; `pub` is.

Scene strings for path-backed resource fields resolve to typed IDs before
`on_init`. This applies to `TextureID`, `MaterialID`, `MeshID`, `AnimationID`,
`AnimationTreeID`, `NavMeshID`, and `SoundFontID`, including values nested in
options, lists, maps, tuples, and custom `#[derive(Variant)]` types.

```rust
#[derive(Clone, Default, Variant)]
struct Look {
    portrait: TextureID,
    materials: Vec<MaterialID>,
}

#[State]
struct ActorState {
    #[expose]
    pub look: Look,
}
```

```text
script_vars = {
    look = {
        portrait = "res://textures/portrait.png",
        materials = ["res://materials/body.pmat"]
    }
}
```

Invalid scene values keep the field default. Normal runtime `set_var!` remains
strict and does not coerce resource path strings.

## Runtime Vars

Inside the same script, use typed state access.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            state.jump_buffer_timer -= dt;
        });
    }
});
```

Other scripts and runtime systems can use state variables declared `pub`.

```rust
#[State]
pub struct PlayerState {
    #[default(100.0)]
    #[expose]
    pub health: f32,

    #[default(240.0)]
    #[expose]
    pub speed: f32,
}
```

```rust
let health = get_var!(ctx.run, player_id, "health");
set_var!(ctx.run, player_id, "speed", variant!(320.0_f32));
```

`get_var!`, `set_var!`, and `script_vars` are runtime paths.

They do not require `#[expose]`, but `get_var!` / `set_var!` require the field to be `pub` — see [Visibility](#visibility).

## Custom Types

Custom structs/enums used through script variable APIs must support Variant conversion.

Derive `Variant` on those types.

```rust
use perro_api::prelude::*;

#[derive(Clone, Copy, Variant)]
pub struct OrbitGoal {
    pub axis: Vector3,
}

#[State]
pub struct SpinnerState {
    #[default(OrbitGoal { axis: Vector3::new(0.0, 1.0, 0.0) })]
    #[expose]
    pub orbit_goal: OrbitGoal,
}
```

This also applies to custom typed params/returns used in `methods!`.

See [Variant](variant.md) for accessors, `parse::<T>()`, and `into_parse::<T>()`.

## Practical Example

A coin wallet whose count survives every frame and can be read by a separate HUD script.

Wallet script (`res/scripts/wallet.rs`):

```rust
use perro_api::prelude::*;

#[State]
pub struct WalletState {
    #[default(0)]
    #[expose]
    pub coins: i32,
}

methods!({
    pub fn add_coins(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) {
        with_state_mut!(ctx.run, WalletState, ctx.id, |s| s.coins += amount);
    }
});
```

HUD script reads the value dynamically by node id — no `#[expose]` needed for `get_var!`, but `coins` must be `pub`:

```rust
let coins = get_var!(ctx.run, wallet_id, var!("coins")).as_i32().unwrap_or(0);
```

The starting balance can be set per placement in the scene because `coins` is `pub`; `#[expose]` only makes the field visible in the inspector:

```text
script = "res://scripts/wallet.rs"
script_vars = { coins = 50 }
```
