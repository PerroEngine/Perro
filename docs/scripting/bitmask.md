# BitMask

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `BitMask` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
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
