# Example: Pickup Flow

This feature uses each path for a different reason:

```text
pickup -> call player method -> targeted command + bool reply
player -> emit inventory_changed -> loose event
debug adapter -> get/set var -> runtime-selected member
```

## Goal, Owners, And Wiring

The pickup owns collectability and a fixed player ref injected by the scene. The
player owns inventory and capacity. HUD owns presentation. A debug adapter owns
generic runtime editing. The player does not know the pickup, HUD, or adapter.

```text
[HealthPotion]
script = "res://scripts/pickup.rs"
script_vars = { player = @Player, item_id = "health_potion" }
```

## Pickup Calls Player

```rust
#[State]
struct PickupState {
    #[expose]
    #[node_ref(Node2D, Node3D)]
    player: Option<NodeID>,

    #[default = String::new()]
    item_id: String,
}

lifecycle!({});

methods!({
    fn collect(&self, ctx: &mut ScriptContext<'_, API>) -> bool {
        let (player, item_id) = with_state!(ctx.run, PickupState, ctx.id, |state| {
            (state.player, state.item_id.clone())
        }).unwrap_or_default();
        let Some(player) = player else {
            return false;
        };

        let accepted = call_method!(
            ctx.run,
            player,
            method!("add_item"),
            params![item_id]
        )
        .as_bool()
        .unwrap_or(false);

        if accepted {
            remove_node!(ctx.run, ctx.id);
        }
        accepted
    }
});
```

## Player Owns Inventory Behavior

```rust
#[State]
struct PlayerState {
    #[default = Vec::new()]
    items: Vec<String>,

    #[default = 10]
    capacity: i32,
}

methods!({
    fn add_item(&self, ctx: &mut ScriptContext<'_, API>, item_id: String) -> bool {
        let count = with_state_mut!(ctx.run, PlayerState, ctx.id, |state| {
            if state.items.len() as i32 >= state.capacity {
                return None;
            }
            state.items.push(item_id);
            Some(state.items.len() as i32)
        }).flatten();

        let Some(count) = count else {
            return false;
        };

        signal_emit!(
            ctx.run,
            signal!("inventory_changed"),
            params![count]
        );
        true
    }
});
```

## HUD Listens

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("inventory_changed"),
            func!("show_item_count")
        );
    }
});

methods!({
    fn show_item_count(&self, ctx: &mut ScriptContext<'_, API>, count: i32) {
        with_node_mut!(ctx.run, UiLabel, ctx.id, |label| {
            label.text = format!("Items: {count}").into();
        });
    }
});
```

## Debug Adapter Uses Dynamic Vars

```rust
let capacity = get_var!(ctx.run, player_id, var!("capacity"))
    .as_i32()
    .unwrap_or(0);

set_var!(
    ctx.run,
    player_id,
    var!("capacity"),
    variant!(capacity + 1)
);
```

Production gameplay with a known `PlayerState` should use typed state access.
The adapter stays dynamic because its target/member selection comes from tool
data.

## Failure, Bad Alternatives, And Extensions

Missing player or rejected inventory call leaves the pickup in place. Missing
signal listeners are valid. A method is required for collect because the pickup
needs acceptance before removal; a signal alone gives no reply. Directly
mutating player state from the pickup would bypass capacity behavior. Calling
the HUD directly would couple inventory to UI.

Extend with an `inventory_full` reply/result, sound and achievement listeners,
or a spawned pickup query. Keep each new reaction on the signal side unless the
player requires a targeted result.

## Verified Equivalents

- targeted command + reply: [controller](../../../../demos/ScriptPatterns/res/scripts/controller.rs)
  -> [player](../../../../demos/ScriptPatterns/res/scripts/player.rs)
- signal fan-out: [player](../../../../demos/ScriptPatterns/res/scripts/player.rs)
  -> [HUD](../../../../demos/ScriptPatterns/res/scripts/hud.rs) +
  [audit](../../../../demos/ScriptPatterns/res/scripts/audit.rs)
- dynamic adapter: [adapter](../../../../demos/ScriptPatterns/res/scripts/adapter.rs)

[Back To Examples](index.md)
