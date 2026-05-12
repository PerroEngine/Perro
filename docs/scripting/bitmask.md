# BitMask

`BitMask` is the shared 32-bit layer mask type used by render, physics, and audio.

Layer numbers are `1..=32`.
Layer `1` maps to bit `0`.
Layer `32` maps to bit `31`.

Use layer helpers when authoring Rust code:

```rust
const PLAYER: BitMask = BitMask::with([1]);
const WORLD_AND_PROPS: BitMask = BitMask::with([2, 3]);
const ALL_BUT_DEBUG: BitMask = BitMask::ALL.without([32]);
```

Or use the macro:

```rust
const NONE: BitMask = bitmask!([]);
const PLAYER: BitMask = bitmask!([1]);
const WORLD_AND_PROPS: BitMask = bitmask!([2, 3]);
```

Invalid layer numbers in `BitMask::layer`, `BitMask::with`, or `BitMask::without` fail at const eval.
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
if camera.render_mask.intersects(sprite.render_layers) {
    // render
}
```

Common fields:

- `render_mask`: camera visibility filter.
- `render_layers`: renderable node membership.
- `collision_layers`: physics body/area membership.
- `collision_mask`: physics body/area collide-with mask.
- `PhysicsQueryFilter.mask`: physics query hit mask.
- `audio_layer`: emitted spatial audio layer bits.
- `audio_mask`: audio geometry layer filter.

Scene files use layer arrays:

```text
render_mask = [1]
render_layers = [1]
collision_layers = [1]
collision_mask_layers = [1, 2, 3]
```
