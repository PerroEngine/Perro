# BitMask

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |

## Purpose

`BitMask` is the shared 32-bit layer mask that decides what interacts with what. It answers gameplay questions like "which cameras can see this prop", "which bodies this bullet is allowed to hit", and "does this wall block that sound". Instead of ad-hoc booleans, you tag objects with layers and filter them with masks, so systems stay data-driven and cheap to test.

## Use Cases

- Separate player, enemy, world, and projectile collision so hits only register between the right groups: set `collision_layers` / `collision_mask` on bodies (scene `only(...)` / `without(...)`, or `BitMask::with` / `bitmask!` in code).
- Hide a whole layer from one camera (editor gizmos, a minimap-only prop set, first-person arms): add the layer to the camera `render_mask` while renderables keep their `render_layers`.
- Restrict a hitscan weapon to solid geometry so shots ignore triggers and allies: build a `PhysicsQueryFilter` with `layers` / `mask` and pass it to `physics_raycast_3d!` / `physics_shape_cast_3d!`.
- Let a wall occlude only some spatial sources: tag emitters with `audio_layer` and give the mask geometry an `audio_mask`.
- Fade a decal or snow shell against just the meshes it should cover: set `blend_layers` / `blend_mask` on the inserted mesh (see the Mesh Blend section below).

## Decision Guide

Use a `BitMask` when one value represents a set of independent numbered layers. Use an enum when exactly one mode is valid, and a struct of booleans when fields need semantic names and scene clarity. For interactions, layers state what an object is; masks state which layers it ignores. Test both participants' policy instead of treating one side as global truth.

## Practical Example

A shooter where enemy fire passes through friendly units. Player bodies live on layer `1`, enemies on layer `2`, and an enemy raycast weapon only tests the world and the player.

Scene bodies:

```text
[Player]
    [CharacterBody3D]
        collision_layers = only(1)
        [Node3D/]
    [/CharacterBody3D]
[/Player]

[Grunt]
    [CharacterBody3D]
        collision_layers = only(2)
        [Node3D/]
    [/CharacterBody3D]
[/Grunt]
```

Enemy weapon script — the raycast filter accepts the world (layer `32`) and the player (layer `1`) but never other enemies:

```rust
use perro_api::prelude::*;

const HITTABLE: BitMask = BitMask::with([1, 32]);

fn fire<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, from: Vector3, dir: Vector3) {
    let filter = PhysicsQueryFilter { layers: HITTABLE, ..Default::default() };
    if let Some(hit) = physics_raycast_3d!(ctx.run, from, dir, 100.0, filter) {
        let _ = hit;
    }
}
```

## Reference

# BitMask

`BitMask` is the shared 32-bit layer mask type used by render, physics, and audio.

Layer numbers are `1..=32`.
Layer `1` maps to bit `0`.
Layer `32` maps to bit `31`.

Use layer helpers when authoring Rust code:

```rust
const PLAYER: BitMask = BitMask::with([1]);
const WORLD_AND_PROPS: BitMask = BitMask::with([2, 3]);
const ALL_BUT_DEBUG: BitMask = BitMask::ALL.without_layers([32]);

let all_but_player = BitMask::without(1);
let all_but_ui = BitMask::without([4, 5]);

let mut layers = BitMask::ALL;
layers.pop(5);
layers.push(8);

let new_layers = layers.popped(5).pushed([1, 2]);
```

Or use the macro:

```rust
const NONE: BitMask = bitmask!([]);
const PLAYER: BitMask = bitmask!([1]);
const WORLD_AND_PROPS: BitMask = bitmask!([2, 3]);
```

Invalid layer numbers in `BitMask::layer`, `BitMask::with`, or `BitMask::without_layers` fail at const eval.
Invalid layer numbers in `BitMask::without`, `BitMask::push`, `BitMask::pushed`, `BitMask::pop`, or `BitMask::popped` panic.
Use `BitMask::try_layer(layer)` for runtime-checked input.

Raw bit values are still available:

```rust
let raw = BitMask::from_bits(0b1010);
let bits: u32 = raw.bits();
```

Slices and vectors use runtime helpers:

```rust
let layers = vec![1usize, 3, 4];
let mask = BitMask::from_layers(&layers);

let maybe_mask = BitMask::try_from_layers(&layers);
```

Mask match:

```rust
if !camera.render_mask.intersects(sprite.render_layers) {
    // render
}
```

## Layers Versus Masks

Layers tag where an object lives.
Fields named `*_mask` or `mask` list layers the object ignores/excludes.
Default mask values are empty (`BitMask::NONE`).

Render:

- `render_layers`: renderable node layer membership.
- `render_mask`: camera ignored-layer filter.
- Node draws when `camera.render_mask` does not intersect `node.render_layers`.
- Default `Node2D` / `Node3D` `render_layers` is `BitMask::ALL`.
- Default `Camera2D` / `Camera3D` `render_mask` is `BitMask::NONE`.
- Add layers to a camera mask to hide those layers.
- `BitMask::NONE` on a camera mask hides nothing.

Physics:

- `collision_layers`: body/area tagged-layer membership.
- `collision_mask`: body/area ignored-layer mask.
- Two colliders interact only when neither side masks the other:
  - `a.collision_mask` does not intersect `b.collision_layers`
  - `b.collision_mask` does not intersect `a.collision_layers`
- Default body/area `collision_layers` is `BitMask::ALL`.
- Default body/area `collision_mask` is `BitMask::NONE`.
- `BitMask::NONE` on a collision mask ignores nothing.
- `BitMask::ALL` on a collision mask ignores all collision partners.
- `BitMask::NONE` on collision layers means the collider belongs to no layers.

Mesh Blend:

- `blend_layers`: mesh tagged-layer membership for 3D mesh blending.
- `blend_mask`: mesh ignored-layer filter for 3D mesh blending.
- A mesh with `blend_enabled = true` fades against target meshes when its `blend_mask` does not intersect their `blend_layers`.
- Target meshes only need explicit `blend_layers`; they do not need `blend_enabled = true`.
- Usually enable blending only on the inserted/top mesh so only that mesh fades.
- Blending is active when `blend_enabled = true`, `blend_layers` is not empty, and `blend_mask` is not `BitMask::ALL`.
- Default `blend_layers` is `BitMask::ALL`.
- Default `blend_mask` is `BitMask::NONE`.
- `BitMask::NONE` on `blend_mask` ignores nothing, so the mesh can blend with any unmasked target layer.
- `BitMask::ALL` on `blend_mask` ignores all target layers.
- Use the same scene syntax as render and collision masks: `blend_layers = [1]`, `blend_mask = only(1, 2)`, or `blend_mask = none`.

Audio:

- `audio_layer`: emitted spatial audio tagged-layer membership.
- `audio_mask`: listener/audio geometry ignored-layer mask.
- Listener options, audio masks, and effect zones apply when `audio_mask` does not intersect emitted `audio_layer`.
- Default emitted `audio_layer` is `BitMask::ALL`.
- Default listener/audio geometry `audio_mask` is `BitMask::NONE`.
- `BitMask::NONE` on an audio mask ignores nothing.
- `BitMask::ALL` on an audio mask ignores all emitted spatial audio.

Query filters:

- `PhysicsQueryFilter.layers`: physics query hit-layer membership.
- `PhysicsQueryFilter.mask`: physics query ignored-layer mask.
- Default `PhysicsQueryFilter.layers` is `BitMask::ALL`.
- Default `PhysicsQueryFilter.mask` is `BitMask::NONE`.
- A raycast or shape cast hits a collider only when `filter.layers` intersects collider `collision_layers` and `filter.mask` does not.

Common fields:

- `render_mask`: camera ignored-layer filter.
- `render_layers`: renderable node membership.
- `collision_layers`: physics body/area tagged-layer membership.
- `collision_mask`: physics body/area ignored-layer mask.
- `PhysicsQueryFilter.layers`: physics query hit-layer membership.
- `PhysicsQueryFilter.mask`: physics query ignored-layer mask.
- `audio_layer`: emitted spatial audio tagged layers.
- `audio_mask`: audio ignored-layer mask.

Scene files use layer arrays:

```text
render_mask = [2]
render_layers = [2]
collision_layers = [1, 2, 3]
collision_mask = []
```

Scene files can also use `only(...)` and `without(...)`:

```text
render_layers = only(1, 2)
render_mask = without(1)
collision_layers = without([1, 32])
collision_mask = only([2, 4])
```
