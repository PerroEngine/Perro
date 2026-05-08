# `.panimtree` Format

`*.panimtree` is a Perro animation graph resource.

It mixes `.panim` clips through named slots and graph nodes.
The `.panimtree` owns slot names and graph connections.
The scene `AnimationTree` node owns runtime clip IDs and per-slot bindings.
Slot bindings connect `.panim` object names to scene node keys.

## Example

```ini
[AnimationTree]
name = "PlayerLocomotion"
[/AnimationTree]

[Slots]
Idle
Run
Aim
[/Slots]

[IdleSrc]
    [Slot]
        slot = Idle
    [/Slot]
[/IdleSrc]

[RunSrc]
    [Slot]
        slot = Run
    [/Slot]
[/RunSrc]

[AimSrc]
    [Slot]
        slot = Aim
    [/Slot]
[/AimSrc]

[MoveBlend]
    [Blend]
        inputs = [@IdleSrc, @RunSrc]
        weights = [1.0, 0.0]
        mask = { objects=[Hero], fields=[position, rotation, scale] }
    [/Blend]
[/MoveBlend]

[AimAdd]
    [Add]
        base = @MoveBlend
        inputs = [@AimSrc]
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
- `[Slots]`: slot names. Scene `animations` bind by slot order.
- `[NodeKey] [Slot]`: reads one runtime slot.
- `[NodeKey] [Blend]`: blends any number of inputs. Weights normalize.
- `[NodeKey] [Add]`: adds any number of inputs onto `base`.
- `[NodeKey] [Invert]`: flips additive delta sign.
- `[Output]`: final graph plug.

Refs use `@NodeKey`.

Masks can include `objects`, `fields`, and `bones`.

## Mix Nodes

`Blend` mixes any number of inputs together.

```ini
[MoveBlend]
    [Blend]
        inputs = [@IdleSrc, @RunSrc]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]
```

Weights normalize.

`[1.0, 0.0]` means full `IdleSrc`.

`[0.0, 1.0]` means full `RunSrc`.

`[0.5, 0.5]` means half idle + half run.

Three or more inputs use same shape.

```ini
[MoveBlend]
    [Blend]
        inputs = [@IdleSrc, @WalkSrc, @RunSrc]
        weights = [0.0, 0.35, 0.65]
    [/Blend]
[/MoveBlend]
```

Weights line up by index.

`@IdleSrc` uses `0.0`.

`@WalkSrc` uses `0.35`.

`@RunSrc` uses `0.65`.

`Add` layers any number of deltas on top of a base pose.

```ini
[AimAdd]
    [Add]
        base = @MoveBlend
        inputs = [@AimSrc]
        weights = [1.0]
    [/Add]
[/AimAdd]
```

`AimSrc` frame 0 is rest pose.

Current `AimSrc` pose minus frame 0 becomes delta.

Delta adds onto `MoveBlend`.

Weight scales delta.

Three or more additive layers use same shape.

```ini
[UpperBodyAdd]
    [Add]
        base = @MoveBlend
        inputs = [@AimSrc, @RecoilSrc, @BreathSrc]
        weights = [1.0, 0.45, 0.2]
    [/Add]
[/UpperBodyAdd]
```

`@AimSrc` delta uses `1.0`.

`@RecoilSrc` delta uses `0.45`.

`@BreathSrc` delta uses `0.2`.

`Invert` flips additive delta sign.

Use it for subtractive layers.

```ini
[AimSubtract]
    [Invert]
        input = @AimSrc
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
            { animation = "res://animations/idle.panim", bindings = { Hero = PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/run.panim", bindings = { Hero = PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/aim.panim", bindings = { Hero = PlayerRoot }, playback = boomerang, speed = 1.0, paused = false },
        ]
    [/AnimationTree]
[/AnimTree]
```

`tree` points at `.panimtree`.

`animations` order maps to `[Slots]` order.

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
@Hero = Node3D
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
    [Node3D]
    [/Node3D]
[/PlayerRoot]

[anim_tree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = PlayerRoot }, playback = loop },
            { animation = "res://animations/run.panim", bindings = { Hero = PlayerRoot }, playback = loop },
            { animation = "res://animations/aim.panim", bindings = { Hero = PlayerRoot }, playback = boomerang },
        ]
    [/AnimationTree]
[/anim_tree]
```

`animations[0]` feeds slot 0 from `[Slots]`, so this is `Idle`.

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

[Slots]
Idle
Run
Aim
[/Slots]

[IdleSrc]
    [Slot]
        slot = Idle
    [/Slot]
[/IdleSrc]

[RunSrc]
    [Slot]
        slot = Run
    [/Slot]
[/RunSrc]

[AimSrc]
    [Slot]
        slot = Aim
    [/Slot]
[/AimSrc]

[MoveBlend]
    [Blend]
        inputs = [@IdleSrc, @RunSrc]
        weights = [1.0, 0.0]
    [/Blend]
[/MoveBlend]

[UpperBodyAdd]
    [Add]
        base = @MoveBlend
        inputs = [@AimSrc]
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
@Hero = Node3D
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
    [Node3D]
    [/Node3D]
[/PlayerRoot]

[AnimTree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = PlayerRoot }, playback = loop, speed = 1.0 },
            { animation = "res://animations/run.panim", bindings = { Hero = PlayerRoot }, playback = loop, speed = 1.0 },
            { animation = "res://animations/aim.panim", bindings = { Hero = PlayerRoot }, playback = boomerang, speed = 0.8 },
        ]
    [/AnimationTree]
[/AnimTree]
```
