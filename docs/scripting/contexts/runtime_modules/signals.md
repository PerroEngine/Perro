# Signals Module

Purpose:

- Global pub/sub messaging between scripts.
- One script emits a signal, any script listening for that signal name reacts.
- Signals are **globally broadcast by name** — emitters and listeners don't need to know about each other.

Macros:

- `signal_connect!(ctx, listener_node_id, signal, handler_function) -> bool`
- `signal_disconnect!(ctx, listener_node_id, signal, handler_function) -> bool`
- `signal_emit!(ctx, signal, params) -> usize`
- `signal_emit!(ctx, signal) -> usize`

Notes:

- `listener_node_id` is the node ID of the script that **has the handler function**.
- `signal` is a `SignalID` created with `signal!("name")`.
- `handler_function` is the method name on the listener script, created with `func!("name")`.
- `signal_emit!` returns the count of listeners that were triggered.
- 3-arg `signal_emit!` uses `&[Variant]` (commonly `params![...]`).
- 2-arg `signal_emit!` emits with empty params.

## Example: Simple Broadcast

**Emitter (Player):**

```rust
lifecycle!({
    fn on_update(&self, ctx, res, ipt, self_id) {
        if key_pressed!(ipt, KeyCode::Space) {
            let listener_count = signal_emit!(ctx, signal!("player_jumped"));
            log::info!("Jump emitted, {} listeners reacted", listener_count);
        }
    }
});
```

**Listener (Enemy):**

```rust
#[State]
pub struct EnemyState {
    alerted: bool,
}

lifecycle!({
    fn on_init(&self, ctx, res, ipt, self_id) {
        // This enemy listens for "player_jumped" globally and connects to it's own "on_alert" function
        signal_connect!(ctx, self_id, signal!("player_jumped"), func!("on_alert"));
    }
});

methods!({
    fn on_alert(&self, ctx, res, ipt, self_id) {
        with_state_mut!(ctx, EnemyState, self_id, |state| {
            state.alerted = true;
        });
    }
});
```

## Example: Signal With Parameters

**Emitter:**

```rust
signal_emit!(ctx, signal!("enemy_defeated"), params![enemy_id, 50i32]); // 50 = points
```

**Listener:**

```rust
methods!({
    fn on_enemy_defeated(&self, ctx, res, ipt, self_id, enemy_id: NodeID, points: i32) {
        log::info!("Defeated enemy {:?} for {} points", enemy_id, points);
    }
});

lifecycle!({
    fn on_init(&self, ctx, res, ipt, self_id) {
        signal_connect!(ctx, self_id, signal!("enemy_defeated"), func!("on_enemy_defeated"));
    }
});
```

## Physics Collision Signals

Physics bodies automatically emit collision signals:

```rust
// When RigidBody3D with node name "Player" collides, it emits:
signal!("Player_Collided")
// params[0] = source body NodeID
// params[1] = other body NodeID
```

**Listen for collisions:**

```rust
lifecycle!({
    fn on_init(&self, ctx, res, ipt, self_id) {
        signal_connect!(ctx, self_id, signal!("Player_Collided"), func!("on_hit"));
    }
});

methods!({
    fn on_hit(&self, ctx, res, ipt, self_id, other_id: NodeID) {
        log::info!("Collided with {:?}", other_id);
    }
});
```

## Area Overlap Signals

`Area2D`/`Area3D` emit lifecycle signals:

- `"{AreaNodeName}_Entered"` — body entered area
- `"{AreaNodeName}_Occupied"` — body still inside
- `"{AreaNodeName}_Exited"` — body left area

```rust
signal_connect!(ctx, self_id, signal!("TriggerZone_Entered"), func!("on_enter"));
signal_connect!(ctx, self_id, signal!("TriggerZone_Exited"), func!("on_exit"));
```

## Disconnecting

```rust
signal_disconnect!(ctx, self_id, signal!("player_jumped"), func!("on_alert"));
```
