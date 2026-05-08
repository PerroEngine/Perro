# Animation

Perro animation has two resource files and two scene nodes.

Sigils:

- `$` => value var define/use.
- scenes: `@NodeKey` => scene node ref.
- `.panim`: `[Objects]` declares bare names (`Hero = Node3D`), frame blocks ref them as `@Hero`.
- `.panimtree`: graph blocks declare bare names (`[MoveBlend]`), graph inputs ref them as `@MoveBlend`.
- scene `AnimationPlayer` / `AnimationTree` bindings map object names to scene node refs with `@NodeKey`.

- `.panim`: one animation clip.
- `.panimtree`: one animation graph.
- `AnimationPlayer`: plays one `.panim`.
- `AnimationTree`: mixes many `.panim` clips through one `.panimtree`.

## `.panim`

Use `.panim` for raw clip data.

It declares animation object names in `[Objects]`.

Those names are clip-local track keys.

They are not scene node names.

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

`@Hero` is the animation object.

Scene binding maps `Hero` to a real scene node.

Full format: [`.panim` Format](resources/panim.md).

## `AnimationPlayer`

Use `AnimationPlayer` when one clip drives one set of bindings.

```ini
[PlayerRoot]
    [Node3D]
    [/Node3D]
[/PlayerRoot]

[IdlePlayer]
    [AnimationPlayer]
        animation = "res://animations/idle.panim"
        bindings = { Hero = @PlayerRoot }
        speed = 1.0
        paused = false
        playback = loop
    [/AnimationPlayer]
[/IdlePlayer]
```

`animation` loads one `.panim`.

`bindings` maps `.panim [Objects]` names to scene nodes.

Binding values must use `@NodeKey` (or a var such as `$root` that resolves to one).

Runtime API:

```rust
let clip = animation_load!(res, "res://animations/idle.panim");
let _ = anim_player_set_clip!(ctx, player, clip);
let _ = anim_player_bind!(ctx, player, "Hero", hero);
let _ = anim_player_seek_frame!(ctx, player, 0);
let _ = anim_player_play!(ctx, player);
```

Runtime docs: [Animations Module](scripting/contexts/runtime_modules/animations.md).

## `.panimtree`

Use `.panimtree` for static graph shape.

It owns:

- slot names
- graph node keys
- `Slot`, `Blend`, `Add`, `Invert`
- default weights
- masks
- required `Output`

It does not own actual scene node bindings.

It does not own runtime playback state.

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

`Blend` mixes any number of inputs.

```ini
[MoveBlend]
    [Blend]
        inputs = [@IdleSrc, @WalkSrc, @RunSrc]
        weights = [0.0, 0.35, 0.65]
    [/Blend]
[/MoveBlend]
```

`Add` applies additive deltas to a base.

```ini
[UpperBodyAdd]
    [Add]
        base = @MoveBlend
        inputs = [@AimSrc, @RecoilSrc, @BreathSrc]
        weights = [1.0, 0.45, 0.2]
    [/Add]
[/UpperBodyAdd]
```

`Invert` flips a delta for subtractive layers.

```ini
[AimSubtract]
    [Invert]
        input = @AimSrc
    [/Invert]
[/AimSubtract]
```

`Output` is required.

Full format: [`.panimtree` Format](resources/panimtree.md).

## `AnimationTree`

Use `AnimationTree` when many clips combine into one final pose.

The scene node supplies:

- `tree`: loaded `.panimtree`
- `animations`: clips bound by slot order
- per-slot clip object bindings
- `speed`
- `paused`

```ini
[Hero]
    [Node3D]
    [/Node3D]
[/Hero]

[HeroAnimTree]
    [AnimationTree]
        tree = "res://animations/player.panimtree"
        animations = [
            { animation = "res://animations/idle.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/run.panim", bindings = { Hero = @PlayerRoot }, playback = loop, speed = 1.0, paused = false },
            { animation = "res://animations/aim.panim", bindings = { Hero = @PlayerRoot }, playback = boomerang, speed = 1.0, paused = false },
        ]
        speed = 1.0
        paused = false
    [/AnimationTree]
[/HeroAnimTree]
```

Slot mapping comes from `.panimtree [Slots]`.

If `[Slots]` is `Idle, Run, Aim`, then:

- `animations[0]` feeds `Idle`
- `animations[1]` feeds `Run`
- `animations[2]` feeds `Aim`

Each slot entry can bind the same `.panim` object name to a different scene node.

Each slot entry can set `playback`, `speed`, and `paused`.

`playback` accepts `once`, `loop`, or `boomerang`.

Slot `speed` multiplies tree `speed`.

Runtime API:

```rust
let run = animation_load!(res, "res://animations/run.panim");
let _ = anim_tree_set_clip!(ctx, tree, "Run", run);
let _ = anim_tree_play_slot!(ctx, tree, "Run");
let _ = anim_tree_seek_slot_frame!(ctx, tree, "Run", 0);
let _ = anim_tree_set_slot_speed!(ctx, tree, "Run", 1.25);
let _ = anim_tree_set_slot_playback!(ctx, tree, "Run", AnimationPlaybackType::Loop);
let _ = anim_tree_set_weight!(ctx, tree, "MoveBlend", "RunSrc", 1.0);
let _ = anim_tree_pause!(ctx, tree, false);
```

Slots accept name or index.

Node and input names omit `@` in Rust API, that's just for .panim and .panimtree files

Bad refs return `false`.

Scene templates: [Scene Node Templates](scripting/scene_node_templates.md).
