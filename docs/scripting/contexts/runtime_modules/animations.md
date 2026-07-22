# Animations Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `set_clip` | [`set_clip`](#set_clip) |
| `play` | [`play`](#play) |
| `pause` | [`pause`](#pause) |
| `seek_frame` | [`seek_frame`](#seek_frame) |
| `set_speed` | [`set_speed`](#set_speed) |
| `bind` | [`bind`](#bind) |
| `clear_bindings` | [`clear_bindings`](#clear_bindings) |
| `anim_player_set_clip` | [`anim_player_set_clip`](#anim_player_set_clip) |
| `anim_player_play` | [`anim_player_play`](#anim_player_play) |
| `anim_player_pause` | [`anim_player_pause`](#anim_player_pause) |
| `anim_player_seek_frame` | [`anim_player_seek_frame`](#anim_player_seek_frame) |
| `anim_player_set_speed` | [`anim_player_set_speed`](#anim_player_set_speed) |
| `anim_player_bind` | [`anim_player_bind`](#anim_player_bind) |
| `anim_player_clear_bindings` | [`anim_player_clear_bindings`](#anim_player_clear_bindings) |
| `set_clip` | [`set_clip`](#set_clip) |
| `play_slot` | [`play_slot`](#play_slot) |
| `pause_slot` | [`pause_slot`](#pause_slot) |
| `seek_slot_frame` | [`seek_slot_frame`](#seek_slot_frame) |
| `set_slot_speed` | [`set_slot_speed`](#set_slot_speed) |
| `set_slot_playback` | [`set_slot_playback`](#set_slot_playback) |
| `seek_node_time` | [`seek_node_time`](#seek_node_time) |
| `set_weight` | [`set_weight`](#set_weight) |
| `pause` | [`pause`](#pause) |
| `anim_tree_set_clip` | [`anim_tree_set_clip`](#anim_tree_set_clip) |
| `anim_tree_play_slot` | [`anim_tree_play_slot`](#anim_tree_play_slot) |
| `anim_tree_pause_slot` | [`anim_tree_pause_slot`](#anim_tree_pause_slot) |
| `anim_tree_seek_slot_frame` | [`anim_tree_seek_slot_frame`](#anim_tree_seek_slot_frame) |
| `anim_tree_set_slot_speed` | [`anim_tree_set_slot_speed`](#anim_tree_set_slot_speed) |
| `anim_tree_set_slot_playback` | [`anim_tree_set_slot_playback`](#anim_tree_set_slot_playback) |
| `anim_tree_seek_node_time` | [`anim_tree_seek_node_time`](#anim_tree_seek_node_time) |
| `anim_tree_set_weight` | [`anim_tree_set_weight`](#anim_tree_set_weight) |
| `anim_tree_pause` | [`anim_tree_pause`](#anim_tree_pause) |

## Purpose

This module drives animation playback from gameplay code. It covers two node
types. An `AnimationPlayer` plays a single clip and is the tool for one-shot
actions and simple looped states: trigger a jump, an attack swing, a door
opening. An `AnimationTree` blends and sequences multiple clips through named
slots and blend nodes, which is how you crossfade walk into run by speed or run
a locomotion state machine. Both let scripts start, pause, retime, and scrub
animation in response to input and game events.

Clips are `AnimationID` resources loaded through `ctx.res.Animations()`; this
module is the runtime side that plays them on a node.

## Use Cases

- One-shot action (jump, attack, hit reaction): point the player at a clip with `anim_player_set_clip!(ctx.run, player, jump_clip)` then `anim_player_play!(ctx.run, player)`.
- Enrage or slow-motion: rescale playback with `anim_player_set_speed!(ctx.run, player, 1.5)` or an `AnimationTree` slot speed.
- Speed-based locomotion blend: drive a blend node's inputs with `anim_tree_set_weight!(ctx.run, tree, "locomotion", "run", weight)` so the character eases from walk to run.
- Locomotion state machine: switch the active clip in a slot with `anim_tree_set_clip!` / `anim_tree_play_slot!` when the movement state changes.
- Hitstop / pause menu: freeze a character mid-motion with `anim_player_pause!(ctx.run, player, true)` or `anim_tree_pause!`, then resume.
- Cutscene poses and scrubbing: jump to an exact frame with `anim_player_seek_frame!(ctx.run, player, frame)`.
- Retarget a track to another node (shared animation, swapped prop): `anim_player_bind!(ctx.run, player, ["Weapon": weapon_node])`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.AnimPlayer() / ctx.run.AnimTree()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Load a jump clip once, cache the `AnimationPlayer` child, then trigger the clip
when a jump-input handler fires.

```rust
#[State]
struct HeroAnim {
    #[expose]
    pub jump: AnimationID,

    #[expose]
    #[node_ref(AnimationPlayer)]
    pub player: Option<NodeID>,
}

methods!({
    // Wired to the jump input action.
    fn on_jump(&self, ctx: &mut ScriptContext<'_, API>) {
        let (player, jump) = with_state!(ctx.run, HeroAnim, ctx.id, |s| (s.player, s.jump));
        if let Some(player) = player {
            anim_player_set_clip!(ctx.run, player, jump);
            anim_player_play!(ctx.run, player);
        }
    }
});
```

Wire both fixed dependencies in scene `script_vars`. The animation path
resolves to `AnimationID` before `on_init`:

```text
script_vars = {
    jump = "res://anim/hero_jump.panim",
    player = @HeroAnimationPlayer
}
```

## API Reference

### `set_clip`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn set_clip(&mut self, player: NodeID, animation: AnimationID) -> bool` |
| Params | `&mut self, player: NodeID, animation: AnimationID` |
| Returns | `bool` |
| Use when | Use `set_clip` to set clip on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_clip` cannot apply to the supplied target or inputs; `true` confirms success. |

### `play`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn play(&mut self, player: NodeID) -> bool` |
| Params | `&mut self, player: NodeID` |
| Returns | `bool` |
| Use when | Use `play` to play on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `play` cannot apply to the supplied target or inputs; `true` confirms success. |

### `pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn pause(&mut self, player: NodeID, paused: bool) -> bool` |
| Params | `&mut self, player: NodeID, paused: bool` |
| Returns | `bool` |
| Use when | Use `pause` to pause on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `pause` cannot apply to the supplied target or inputs; `true` confirms success. |

### `seek_frame`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn seek_frame(&mut self, player: NodeID, frame: u32) -> bool` |
| Params | `&mut self, player: NodeID, frame: u32` |
| Returns | `bool` |
| Use when | Use `seek_frame` to seek frame on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `seek_frame` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn set_speed(&mut self, player: NodeID, speed: f32) -> bool` |
| Params | `&mut self, player: NodeID, speed: f32` |
| Returns | `bool` |
| Use when | Use `set_speed` to set speed on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_speed` cannot apply to the supplied target or inputs; `true` confirms success. |

### `bind`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn bind<S: AsRef<str>>(&mut self, player: NodeID, track: S, node: NodeID) -> bool` |
| Params | `&mut self, player: NodeID, track: S, node: NodeID` |
| Returns | `bool` |
| Use when | Use `bind` to bind on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `bind` cannot apply to the supplied target or inputs; `true` confirms success. |

### `clear_bindings`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer()` |
| Signature | `pub fn clear_bindings(&mut self, player: NodeID) -> bool` |
| Params | `&mut self, player: NodeID` |
| Returns | `bool` |
| Use when | Use `clear_bindings` to clear bindings on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `clear_bindings` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_player_set_clip`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_set_clip!(ctx.run, player, animation)` |
| Params | `ctx, player, animation` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_player_set_clip` to anim player set clip on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_player_set_clip` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_player_play`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_play!(ctx.run, player)` |
| Params | `ctx, player` |
| Returns | `same as backing method` |
| Use when | Use `anim_player_play` to anim player play on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Uses the backing `anim_player_play` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `anim_player_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_pause!(ctx.run, player, paused)` |
| Params | `ctx, player, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_player_pause` to anim player pause on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_player_pause` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_player_seek_frame`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_seek_frame!(ctx.run, player, frame)` |
| Params | `ctx, player, frame` |
| Returns | `same as backing method` |
| Use when | Use `anim_player_seek_frame` to anim player seek frame on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Uses the backing `anim_player_seek_frame` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `anim_player_set_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_set_speed!(ctx.run, player, speed)` |
| Params | `ctx, player, speed` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_player_set_speed` to anim player set speed on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_player_set_speed` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_player_bind`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_bind!(ctx.run, player, [ $(track : node),* $(,)? ])` |
| Params | `ctx, player, [ $(track : node),* $(,)? ]` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_player_bind` to anim player bind on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_player_bind` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_player_clear_bindings`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_player_clear_bindings!(ctx.run, player)` |
| Params | `ctx, player` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_player_clear_bindings` to anim player clear bindings on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_player_clear_bindings` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_clip`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn set_clip<'a, S: IntoAnimTreeSlotArg<'a>>( &mut self, tree: NodeID, slot: S, animation: AnimationID, ) -> bool` |
| Params | `&mut self, tree: NodeID, slot: S, animation: AnimationID,` |
| Returns | `bool` |
| Use when | Use `set_clip` to set clip on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_clip` cannot apply to the supplied target or inputs; `true` confirms success. |

### `play_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn play_slot(&mut self, tree: NodeID, slot: &str) -> bool` |
| Params | `&mut self, tree: NodeID, slot: &str` |
| Returns | `bool` |
| Use when | Use `play_slot` to play slot on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `play_slot` cannot apply to the supplied target or inputs; `true` confirms success. |

### `pause_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn pause_slot(&mut self, tree: NodeID, slot: &str, paused: bool) -> bool` |
| Params | `&mut self, tree: NodeID, slot: &str, paused: bool` |
| Returns | `bool` |
| Use when | Use `pause_slot` to pause slot on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `pause_slot` cannot apply to the supplied target or inputs; `true` confirms success. |

### `seek_slot_frame`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn seek_slot_frame(&mut self, tree: NodeID, slot: &str, frame: u32) -> bool` |
| Params | `&mut self, tree: NodeID, slot: &str, frame: u32` |
| Returns | `bool` |
| Use when | Use `seek_slot_frame` to seek slot frame on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `seek_slot_frame` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_slot_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn set_slot_speed(&mut self, tree: NodeID, slot: &str, speed: f32) -> bool` |
| Params | `&mut self, tree: NodeID, slot: &str, speed: f32` |
| Returns | `bool` |
| Use when | Use `set_slot_speed` to set slot speed on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_slot_speed` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_slot_playback`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn set_slot_playback( &mut self, tree: NodeID, slot: &str, playback_type: AnimationPlaybackType, ) -> bool` |
| Params | `&mut self, tree: NodeID, slot: &str, playback_type: AnimationPlaybackType,` |
| Returns | `bool` |
| Use when | Use `set_slot_playback` to set slot playback on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_slot_playback` cannot apply to the supplied target or inputs; `true` confirms success. |

### `seek_node_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn seek_node_time(&mut self, tree: NodeID, node: &str, seconds: f32) -> bool` |
| Params | `&mut self, tree: NodeID, node: &str, seconds: f32` |
| Returns | `bool` |
| Use when | Use `seek_node_time` to seek node time on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `seek_node_time` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_weight`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn set_weight(&mut self, tree: NodeID, node: &str, input: &str, weight: f32) -> bool` |
| Params | `&mut self, tree: NodeID, node: &str, input: &str, weight: f32` |
| Returns | `bool` |
| Use when | Use `set_weight` to set weight on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `set_weight` cannot apply to the supplied target or inputs; `true` confirms success. |

### `pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimTree()` |
| Signature | `pub fn pause(&mut self, tree: NodeID, paused: bool) -> bool` |
| Params | `&mut self, tree: NodeID, paused: bool` |
| Returns | `bool` |
| Use when | Use `pause` to pause on a runtime animation player/tree; caller owns compatible IDs and playback state. |
| Fails when / edge behavior | Returns `false` when `pause` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_set_clip`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_set_clip!(ctx.run, tree, slot, animation)` |
| Params | `ctx, tree, slot, animation` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_set_clip` to anim tree set clip on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_set_clip` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_play_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_play_slot!(ctx.run, tree, slot)` |
| Params | `ctx, tree, slot` |
| Returns | `same as backing method` |
| Use when | Use `anim_tree_play_slot` to anim tree play slot on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Uses the backing `anim_tree_play_slot` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `anim_tree_pause_slot`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_pause_slot!(ctx.run, tree, slot, paused)` |
| Params | `ctx, tree, slot, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_pause_slot` to anim tree pause slot on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_pause_slot` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_seek_slot_frame`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_seek_slot_frame!(ctx.run, tree, slot, frame)` |
| Params | `ctx, tree, slot, frame` |
| Returns | `same as backing method` |
| Use when | Use `anim_tree_seek_slot_frame` to anim tree seek slot frame on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Uses the backing `anim_tree_seek_slot_frame` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `anim_tree_set_slot_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_set_slot_speed!(ctx.run, tree, slot, speed)` |
| Params | `ctx, tree, slot, speed` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_set_slot_speed` to anim tree set slot speed on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_set_slot_speed` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_set_slot_playback`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_set_slot_playback!(ctx.run, tree, slot, playback)` |
| Params | `ctx, tree, slot, playback` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_set_slot_playback` to anim tree set slot playback on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_set_slot_playback` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_seek_node_time`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_seek_node_time!(ctx.run, tree, node, seconds)` |
| Params | `ctx, tree, node, seconds` |
| Returns | `same as backing method` |
| Use when | Use `anim_tree_seek_node_time` to anim tree seek node time on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Uses the backing `anim_tree_seek_node_time` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `anim_tree_set_weight`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_set_weight!(ctx.run, tree, node, input, weight)` |
| Params | `ctx, tree, node, input, weight` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_set_weight` to anim tree set weight on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_set_weight` cannot apply to the supplied target or inputs; `true` confirms success. |

### `anim_tree_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.AnimPlayer() / ctx.run.AnimTree()` |
| Signature | `anim_tree_pause!(ctx.run, tree, paused)` |
| Params | `ctx, tree, paused` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `anim_tree_pause` to anim tree pause on a runtime animation player/tree; the caller owns valid player, slot, clip, and node IDs. |
| Fails when / edge behavior | Returns `false` when `anim_tree_pause` cannot apply to the supplied target or inputs; `true` confirms success. |

