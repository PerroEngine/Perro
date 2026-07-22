# Feature Story: Manager And Spawned Enemies

## Goal

An encounter manager spawns enemies, counts living members, and advances after
the last death without fixed refs to instances that do not exist at scene load.

## Owners And Data Flow

```text
manager timer -> instantiate enemy scene -> tag/group membership
enemy owns HP + movement
enemy death -> enemy_died signal -> manager count/phase
manager query -> current dynamic enemy set
```

The manager owns wave order and spawn cadence. Each enemy owns its node and
state. The enemy scene defines internal fixed refs. Runtime instances belong to
a tag/query set rather than manager state full of scene-injected IDs.

## State And Scene Wiring

```rust
#[State]
struct EncounterState {
    #[default = PreloadedSceneID::nil()]
    enemy_scene: PreloadedSceneID,
    #[default = 0]
    wave: i32,
}
```

The authored manager node owns this state. `on_init` preloads
`res://scenes/enemy.scn` and stores its runtime scene handle. Each enemy scene
root carries tag `enemy`; its own scene injects any child refs it needs.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let scene = scene_preload!(ctx.run, "res://scenes/enemy.scn")
            .unwrap_or(PreloadedSceneID::nil());
        with_state_mut!(ctx.run, EncounterState, ctx.id, |state| state.enemy_scene = scene);
    }

    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, signal!("enemy_died"), func!("on_enemy_died"));
        timer_start!(ctx.run, Duration::from_millis(500), "encounter_spawn_wave");
    }
});

methods!({
    fn on_enemy_died(&self, ctx: &mut ScriptContext<'_, API>, _enemy: NodeID) {
        let alive = query!(ctx.run, all(tags["enemy"]));
        if alive.is_empty() {
            timer_start!(ctx.run, Duration::from_secs(1), "encounter_spawn_wave");
        }
    }
});
```

The timer handler loads `enemy_scene`, handles `Err` by leaving the wave
unchanged, and moves successful roots under the encounter subtree. Enemy death
emits `params![ctx.id]` before removal so listeners know the source.

## Complete Flow

During `on_all_init`, the manager connects `enemy_died` and starts a named
`spawn_wave` timer. Its handler instantiates the enemy scene at spawn points.
Each enemy's damage method updates typed HP; after the state closure ends, zero
HP emits `enemy_died` and removes the node. The manager handler queries current
enemy membership and starts the next wave when empty.

## Why These APIs

- Use a named timer: wave start is delayed work without visible progress.
- Instantiate a scene: an enemy has reusable child structure and scripts.
- Use query/tag membership: instances are created and removed dynamically.
- Use signal: enemies report a fact without knowing the encounter manager.

Do not inject enemy `NodeID`s: they do not exist at scene construction. Do not
make every enemy call one hard-coded manager unless a reply is required.

## Failure And Extensions

Failed spawns do not enter a registry. Empty queries are normal. Duplicate death
events must not decrement below zero; deriving liveness from current nodes avoids
that class of drift. Extend with pooling, a wave definition resource, or a
separate reward listener on the same signal.

Verified scene preload/load flow: [Demo2D manager](../../../../demos/Demo2D/res/scripts/demo2d_manager.rs).
Verified runtime spawn + tag flow: [Demo2D dynamic zones](../../../../demos/Demo2D/res/scripts/demo2d_manager.rs).
Exact query forms: [Query System](../../query_system.md).
