# Feature Story: Player, Camera, And HUD

## Goal

Player movement drives one authored camera and health changes update any
interested UI/audio system without giving the player UI references.

## Owners And Wiring

```text
scene -> player.camera = @Camera
input -> PlayerState + CharacterBody -> Camera3D
damage -> PlayerState -> health_changed -> HUD + audio
```

The player owns health and movement. The camera owns camera fields. The HUD owns
label presentation. The scene injects the fixed camera `NodeID` into player
state. HUD and audio connect to `health_changed` during `on_all_init`.

```rust
#[State]
struct PlayerState {
    #[default = 100]
    health: i32,
    #[default = NodeID::nil()]
    #[node_ref(Camera3D)]
    camera: NodeID,
}
```

## Complete Flow

`on_update` reads input, mutates the player's concrete node, copies the camera
ID from state, ends the borrow, and updates the concrete camera. `take_damage`
mutates typed state, copies the resulting health out, ends the borrow, and emits
`health_changed`. HUD receives health and mutates its own `UiLabel` through
`ctx.id`.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("health_changed"), func!("show_health"));
    }
});

methods!({
    fn take_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> i32 {
        let health = with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            state.health = (state.health - amount.max(0)).max(0);
            state.health
        }).unwrap_or(0);
        signal_emit!(ctx.run, signal!("health_changed"), params![ctx.id, health]);
        health
    }
});
```

HUD handler accepts `(source: NodeID, health: i32)`, filters `source` when more
than one player exists, and edits its own `UiLabel`. Camera update follows the
same borrow-safe shape: copy `camera` out of state, skip nil, mutate camera in a
separate `with_node_mut!` call.

## Why These APIs

- Inject `NodeID`: the camera is a fixed authored dependency.
- Use `with_node_mut!`: concrete player/camera/HUD types are known.
- Use typed state: the player state type is known inside its script.
- Emit a signal: health change is a fact with zero or many listeners.

Do not query for the camera by name each frame. Do not store HUD IDs on the
player; that couples gameplay to presentation. A method would fit a targeted
camera command, but not health fan-out.

## Failure And Extensions

A nil/removed camera skips camera work while player movement continues. No HUD
listener is valid; signal emission still succeeds. Extend with another signal
listener for hurt audio, or inject an optional aim target if it is fixed.

Verified fixed refs + fan-out: [ScriptPatterns scene](../../../../demos/ScriptPatterns/res/main.scn),
[player](../../../../demos/ScriptPatterns/res/scripts/player.rs),
[HUD](../../../../demos/ScriptPatterns/res/scripts/hud.rs), and
[audit listener](../../../../demos/ScriptPatterns/res/scripts/audit.rs).
