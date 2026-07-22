# Example: Player Health Signal Updates HUD

Use a signal because health change is an event and more listeners may appear
later. The player does not need a HUD reference.

## Owners And Data Flow

```text
damage method -> PlayerState.health -> player_health_changed(health)
-> HUD UiLabel
-> optional audio/achievement/analytics listeners
```

## Player Script

```rust
#[State]
struct PlayerState {
    #[default = 100]
    health: i32,
}

lifecycle!({});

methods!({
    fn take_damage(&self, ctx: &mut ScriptContext<'_, API>, amount: i32) -> bool {
        let health = with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            state.health = (state.health - amount.max(0)).max(0);
            state.health
        }).unwrap_or(0);

        signal_emit!(
            ctx.run,
            signal!("player_health_changed"),
            params![health]
        );
        health > 0
    }
});
```

## HUD Script

```rust
type SelfNodeType = UiLabel;

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("player_health_changed"),
            func!("on_health_changed")
        );
    }
});

methods!({
    fn on_health_changed(&self, ctx: &mut ScriptContext<'_, API>, health: i32) {
        with_node_mut!(ctx.run, SelfNodeType, ctx.id, |label| {
            label.text = format!("Health: {health}").into();
        });
    }
});
```

Audio, achievements, or analytics scripts may connect to the same signal
without changing the player script.

No listeners is valid. A receiver removed later simply stops reacting. Use a
method instead if the player must target one receiver and consume its reply.
Do not inject every possible listener into player state.

[Back To Examples](index.md)
