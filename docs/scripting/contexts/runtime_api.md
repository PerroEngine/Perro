# Runtime API

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Runtime Modules | [Runtime Modules](#runtime-modules) |
| Example | [Example](#example) |

## Purpose

`ctx.run` is the window a script uses to act on the live game world. It is where
gameplay code reads the frame clock, moves and spawns nodes, loads the next
level, plays animations and sound, runs physics queries, and fires the signals
that let systems talk to each other. If a script needs to change something that
is happening *right now* in the running game, it happens through `ctx.run`.

Resources you load ahead of time (textures, meshes, audio clips) live under
`ctx.res`; live player input lives under `ctx.ipt`. `ctx.run` is the runtime
side: the state that changes every frame.

## Use Cases

- Frame-rate-independent movement and cooldowns: read the frame delta with `delta_time!(ctx.run)` and drive one-shot delays with `timer_start!` (see [Time](runtime_modules/time.md)).
- Move and pose the world: reposition a character with `set_global_pos_3d!(ctx.run, id, pos)`, aim a turret with `look_at_3d!`, or spawn a pickup with `spawn!` (see [Nodes](runtime_modules/nodes.md)).
- Level flow: swap the active level with `scene_load!(ctx.run, "res://levels/boss.pscene")` or warm the next area with `scene_preload!` (see [Scenes](runtime_modules/scenes.md)).
- Cross-system messaging: announce `signal_emit!(ctx.run, signal!("boss_defeated"), params![])` and let unlocks, music, and UI react (see [Signals](runtime_modules/signals.md)).
- Character control and hit detection: slide a player with `physics_move_and_slide_3d!` and shoot a line-of-sight ray with `ctx.run.Physics().raycast_3d(...)` (see [Physics](runtime_modules/physics.md)).
- Playback and feedback: trigger a jump clip with `anim_player_play!`, or attach a footstep sound to the player with `audio_play_attached!` (see [Animations](runtime_modules/animations.md), [Audio](runtime_modules/audio.md)).

## Runtime Modules

| Module | Page | Ctx |
| --- | --- | --- |
| Animations | [animations](runtime_modules/animations.md) | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Audio | [audio](runtime_modules/audio.md) | `ctx.run.Audio()` |
| Helpers | [helpers](runtime_modules/helpers.md) | `helper macros` |
| Mesh Query | [mesh_query](runtime_modules/mesh_query.md) | `ctx.run.MeshQuery()` |
| Navmesh | [navmesh](runtime_modules/navmesh.md) | `ctx.run.NavMesh()` |
| Node Query | [node_query](runtime_modules/node_query.md) | `ctx.run.NodeQuery()` |
| Nodes | [nodes](runtime_modules/nodes.md) | `ctx.run.Nodes()` |
| Physics | [physics](runtime_modules/physics.md) | `ctx.run.Physics()` |
| Scenes | [scenes](runtime_modules/scenes.md) | `ctx.run.Scene()` |
| Scripts | [scripts](runtime_modules/scripts.md) | `ctx.run.Scripts()` |
| Signals | [signals](runtime_modules/signals.md) | `ctx.run.Signals()` |
| Time | [time](runtime_modules/time.md) | `ctx.run.Time()` |
| Window | [window](runtime_modules/window.md) | `ctx.run.Window()` |

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        if dt > 0.0 {
            window_set_title!(ctx.run, "Perro");
        }
    }
});
```
