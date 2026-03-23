# Animations Module

Access:

- `ctx.Animations()`

Macros:

- `animation_set_clip!(ctx, animation_player_id, animation_id) -> bool`
- `animation_play!(ctx, animation_player_id) -> bool`
- `animation_pause!(ctx, animation_player_id, paused) -> bool`
- `animation_seek_time!(ctx, animation_player_id, seconds) -> bool`
- `animation_seek_frame!(ctx, animation_player_id, frame) -> bool`
- `animation_set_speed!(ctx, animation_player_id, speed) -> bool`
- `animation_bind!(ctx, animation_player_id, track, node_id) -> bool`
- `animation_bind!(ctx, animation_player_id, ["Track": node_id, ...]) -> bool`
- `animation_bind!(ctx, animation_player_id, {"Track" => node_id, ...}) -> bool`
- `animation_clear_bindings!(ctx, animation_player_id) -> bool`

Methods:

- `ctx.Animations().set_clip(animation_player_id, animation_id) -> bool`
- `ctx.Animations().play(animation_player_id) -> bool`
- `ctx.Animations().pause(animation_player_id, paused) -> bool`
- `ctx.Animations().seek_time(animation_player_id, seconds) -> bool`
- `ctx.Animations().seek_frame(animation_player_id, frame) -> bool`
- `ctx.Animations().set_speed(animation_player_id, speed) -> bool`
- `ctx.Animations().bind(animation_player_id, track, node_id) -> bool`
- `ctx.Animations().clear_bindings(animation_player_id) -> bool`

## What `animation_player_id` Is

`animation_player_id` must be a `NodeID` for an `AnimationPlayer` node.

All macros return `false` when:

- `animation_player_id` is invalid, or
- `animation_player_id` is not an `AnimationPlayer`

## Clip Assignment

Typical flow:

1. load clip via `ResourceContext` (`animation_load!`)
2. set clip on `AnimationPlayer` (`animation_set_clip!`)
3. bind clip objects to scene nodes (`animation_bind!`)
4. play (`animation_play!`)

## Track Binding

`animation_bind!` maps clip object names to runtime nodes.

- `track` is the object name from `[Objects]` in `.panim` (without `@`)
- bindings are per-player
- rebinding same track overwrites previous node
- mapping forms apply multiple entries and return `true` only if all binds succeed
- this is a link from `AnimationObject` -> runtime `NodeID`
- bind the object to a node of the expected type for that object's animated fields
- if types do not match, those track writes will not apply as intended at runtime

Example:

```rust
let _ = animation_bind!(ctx, animation_player_id, "Hero", hero_node_id);
let _ = animation_bind!(ctx, animation_player_id, "MainCam", camera_node_id);

let _ = animation_bind!(ctx, animation_player_id, [
    "Hero": hero_node_id,
    "MainCam": camera_node_id,
]);
```

## Playback Controls

- `play`: unpauses player.
- `pause(true)`: pauses player.
- `seek_time`: sets playback time in seconds.
- `seek_frame`: sets current frame directly.
- `set_speed`: multiplies playback speed.

Scene `playback` mode (`once`, `loop`, `boomerang`) is a property on `AnimationPlayer` node data, not part of this module API.

## Full Example

```rust
let clip = animation_load!(res, "res://animations/hero_run.panim");
let _ = animation_set_clip!(ctx, animation_player_id, clip);

let _ = animation_bind!(ctx, animation_player_id, [
    "Hero": hero_id,
    "Weapon": weapon_id,
]);

let _ = animation_set_speed!(ctx, animation_player_id, 1.25);
let _ = animation_seek_frame!(ctx, animation_player_id, 0);
let _ = animation_play!(ctx, animation_player_id);
```

## Scene Authoring Relation

`AnimationPlayer` fields in `.scn`:

- `animation = "res://animations/clip.panim"`
- `bindings = [{ Hero = HeroNode }, { Weapon = WeaponNode }]`
- `bindings = [{ "Hero": HeroNode }, { "Weapon": WeaponNode }]`
- bindings are map entries: `AnimationObject -> SceneKey`
- scene key (`HeroNode`) is resolved to runtime `NodeID` during scene merge
- `speed = 1.0`
- `paused = true|false`
- `playback = "once" | "loop" | "boomerang"`
- default `paused` is `false`

Example scene-key binding:

```scn
[bob]
    [Node3D]
    [/Node3D]
[/bob]

[anim_player]
    [AnimationPlayer]
        animation = "res://animations/hero_run.panim"
        bindings = [{ Hero = bob }]
    [/AnimationPlayer]
[/anim_player]
```

Scripts can override or update these values at runtime.
