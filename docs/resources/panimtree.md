# `.panimtree` Format

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`.panimtree` is an animation graph that mixes several `.panim` clips into one final pose. It solves the problem of blending walk/run/idle by speed, or layering an aim pose on top of locomotion, without hand-writing transition and cross-fade code every frame. The graph owns the fixed shape (slot names, `Blend`/`Add`/`Invert` nodes, default weights, masks, required `Output`); the scene `AnimationTree` node owns the runtime clips and bindings and lets scripts push live weights.

## Use Cases

- Speed-based locomotion blend: feed `Idle`, `Walk`, and `Run` slots into one `[Blend]` and drive it with `anim_tree_set_weight!` from the character's current speed.
- Upper-body aim layer: stack an `[Add]` node with `base = @MoveBlend` and `inputs = [@Aim]`, masked to `bones = [Spine, Chest]`, so aiming only bends the torso.
- Directional strafe blends: mix `StrafeLeft`/`StrafeRight` clips by input axis without snapping between them.
- Recoil and breathing overlays: additive layers with per-input `weights` scale a recoil kick or idle breath onto the moving base pose.
- Subtractive cleanup: run a pose through `[Invert]` then `[Add]` to remove an already-baked additive contribution.
- Per-slot playback control at runtime: `anim_tree_play_slot!`, `anim_tree_set_slot_speed!`, and `anim_tree_set_slot_playback!` retime a single slot without touching the rest.

## Example

Author `res://animations/player.panimtree`:

```ini
[AnimationTree]
name = "PlayerLocomotion"
[/AnimationTree]

[AnimationSlots]
Idle
Run
Aim
[/AnimationSlots]

[MoveBlend]
    [Blend]
        inputs = [@Idle, @Run]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]

[AimAdd]
    [Add]
        base = @MoveBlend
        inputs = [@Aim]
        weights = [0.75]
        mask = { objects = [Hero], bones = [Spine, Chest] }
    [/Add]
[/AimAdd]

[Output]
    input = @AimAdd
[/Output]
```

Wire it into a scene `AnimationTree` (slot order matches `[AnimationSlots]`, and each
clip's `Hero` object binds to the `@PlayerRoot` scene node):

```ini
[PlayerRoot]
    [Node3D/]
[/PlayerRoot]

[HeroAnimTree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang },
        ]
    [/AnimationTree]
[/HeroAnimTree]
```

Blend run in as the character accelerates:

```rust
let run_weight = (speed / max_speed).clamp(0.0, 1.0);
let _ = anim_tree_set_weight!(ctx, tree, "MoveBlend", "Idle", 1.0 - run_weight);
let _ = anim_tree_set_weight!(ctx, tree, "MoveBlend", "Run", run_weight);
```

## Reference

# `.panimtree` Format

`*.panimtree` is a Perro animation graph resource.

It mixes `.panim` clips through named slots and graph nodes.
The `.panimtree` owns slot names and graph connections.
The scene `AnimationTree` node owns runtime clip IDs and per-slot bindings.
Slot bindings connect `.panim` object names to scene node keys.

Sigils:
- graph blocks declare bare names: `[MoveBlend]`, `[AimAdd]`
- graph refs use `@Name`: `input = @AimAdd`, `inputs = [@Idle, @Run]`
- scene-side binding values use `@SceneNodeKey` (or a var like `$root` that resolves to one)

## Example

```ini
[AnimationTree]
name = "PlayerLocomotion"
[/AnimationTree]

[AnimationSlots]
Idle
Run
Aim
[/AnimationSlots]

[MoveBlend]
    [Blend]
        inputs = [@Idle, @Run]
        weights = [1.0, 0.0]
        mask = { objects=[Hero], fields=[position, rotation, scale] }
    [/Blend]
[/MoveBlend]

[AimAdd]
    [Add]
        base = @MoveBlend
        inputs = [@Aim]
        weights = [0.75]
        mask = { objects=[Hero], bones=[Spine, Chest] }
    [/Add]
[/AimAdd]

[Output]
    input = @AimAdd
[/Output]
```

## Blocks

- `[AnimationTree]`: tree metadata.
- `[AnimationSlots]`: slot names. Scene `animations` bind by slot order.
- `@SlotName`: reads one runtime slot.
- `[NodeKey] [Blend]`: blends any number of inputs. Weights normalize.
- `[NodeKey] [Add]`: adds any number of inputs onto `base`.
- `[NodeKey] [Invert]`: flips additive delta sign.
- `[Output]`: final graph plug.

Refs use `@NodeKey` or `@SlotName`.

Masks can include `objects`, `fields`, and `bones`.

## Mix Nodes

`Blend` mixes any number of inputs together.

```ini
[MoveBlend]
    [Blend]
        inputs = [@Idle, @Run]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]
```

Weights normalize.

`[1.0, 0.0]` means full `Idle`.

`[0.0, 1.0]` means full `Run`.

`[0.5, 0.5]` means half idle + half run.

Three or more inputs use same shape.

```ini
[MoveBlend]
    [Blend]
        inputs = [@Idle, @Walk, @Run]
        weights = [0.0, 0.35, 0.65]
    [/Blend]
[/MoveBlend]
```

Weights line up by index.

`@Idle` uses `0.0`.

`@Walk` uses `0.35`.

`@Run` uses `0.65`.

`Add` layers any number of deltas on top of a base pose.

```ini
[AimAdd]
    [Add]
        base = @MoveBlend
        inputs = [@Aim]
        weights = [1.0]
    [/Add]
[/AimAdd]
```

`Aim` frame 0 is rest pose.

Current `Aim` pose minus frame 0 becomes delta.

Delta adds onto `MoveBlend`.

Weight scales delta.

Three or more additive layers use same shape.

```ini
[UpperBodyAdd]
    [Add]
        base = @MoveBlend
        inputs = [@Aim, @Recoil, @Breath]
        weights = [1.0, 0.45, 0.2]
    [/Add]
[/UpperBodyAdd]
```

`@Aim` delta uses `1.0`.

`@Recoil` delta uses `0.45`.

`@Breath` delta uses `0.2`.

`Invert` flips additive delta sign.

Use it for subtractive layers.

```ini
[AimSubtract]
    [Invert]
        input = @Aim
    [/Invert]
[/AimSubtract]

[AimRemove]
    [Add]
        base = @MoveBlend
        inputs = [@AimSubtract]
        weights = [1.0]
    [/Add]
[/AimRemove]
```

`Output` is required.

It chooses final pose plug.

```ini
[Output]
    input = @AimAdd
[/Output]
```

No `[Output]` means tree parse fails.

## Scene Binding

Scene node template:

```ini
[AnimTree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        speed = 1.0
        paused = false
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang, speed = 1.0, paused = false },
        ]
    [/AnimationTree]
[/AnimTree]
```

`tree` points at `.panimtree`.

`animations` order maps to `[AnimationSlots]` order.

Each animation entry owns its own object bindings.

Each animation entry owns slot playback behavior.

`playback` accepts `once`, `loop`, or `boomerang`.

Entry `speed` multiplies tree `speed`.

Entry `paused` pauses only that slot.

`speed` and `paused` are runtime playback defaults.

## `.panim` Object Names

If `res://anim/idle.panim` declares this:

```ini
[Objects]
Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (0, 0, 0)
}
[/Frame0]
```

Then `Hero` is not the scene node name.

`Hero` is the animation object name inside that clip.

Bind it to a real scene node in `animations`:

```ini
[PlayerRoot]
    [Node3D/]
[/PlayerRoot]

[anim_tree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang },
        ]
    [/AnimationTree]
[/anim_tree]
```

`animations[0]` feeds slot 0 from `[AnimationSlots]`, so this is `Idle`.

`animations[1]` feeds slot 1, so this is `Run`.

`animations[2]` feeds slot 2, so this is `Aim`.

Each clip can use same object name (`@Hero`) or different object names.

Bindings are per clip slot, not global.

String entries still load clips, with no bindings:

```ini
animations = ["res://animations/idle.panim", "res://animations/run.panim"]
```

String entries only work for clips that do not need object bindings.

## Full File Set

`res://animations/player.panimtree`:

```ini
[AnimationTree]
name = "PlayerLocomotion"
[/AnimationTree]

[AnimationSlots]
Idle
Run
Aim
[/AnimationSlots]

[MoveBlend]
    [Blend]
        inputs = [@Idle, @Run]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]

[UpperBodyAdd]
    [Add]
        base = @MoveBlend
        inputs = [@Aim]
        weights = [0.75]
        mask = { objects=[Hero], bones=[Spine, Chest] }
    [/Add]
[/UpperBodyAdd]

[Output]
    input = @UpperBodyAdd
[/Output]
```

`res://animations/idle.panim`:

```ini
[Animation]
name = "Idle"
fps = 30
[/Animation]

[Objects]
Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (0, 0, 0)
}
[/Frame0]
```

Scene:

```ini
[PlayerRoot]
    [Node3D/]
[/PlayerRoot]

[AnimTree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0 },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0 },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang, speed = 0.8 },
        ]
    [/AnimationTree]
[/AnimTree]
```
