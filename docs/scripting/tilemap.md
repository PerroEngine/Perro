# TileMap2D

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Practical Example | [Practical Example](#practical-example) |
| Reference | [Reference](#reference) |

## Purpose

`TileMap2D` builds a 2D level out of a grid of atlas tiles from a `.ptileset` instead of thousands of hand-placed sprites. Solid tiles can bake into static colliders and cast 2D light shadows, so one node gives you the level's visuals, collision, and shadow casters at once. It suits both hand-authored stages and grids generated at runtime.

## Use Cases

- Hand-authored platformer or top-down stages: set `tileset` and the row-major `tiles` grid in the scene, with `collision_enabled = true` so solid tiles become static colliders and shadow casters.
- Procedurally generated dungeon floors, caves, or terrain: write `width`, `height`, and the `tiles` array on the node at runtime with `with_node_mut!(ctx.run, TileMap2D, id, ...)`; the runtime only re-bakes collision when tile content changes.
- Sloped, spiked, or rounded tiles beyond box collision: give the tile an explicit `collision_shape` (`rect`, `circle`, `triangle`, or convex `polygon`) in the tileset; auto tiles merge into larger rect colliders.
- Filter what the level collides with: `collision_layers` / `collision_mask` on the tilemap (see [BitMask](bitmask.md)).

## Practical Example

Generate a walled room at load time by writing the tile grid directly on the node. Tile id `1` is a solid wall (collision) and id `0` is open floor.

```rust
use perro_api::prelude::*;

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (w, h) = (16u32, 12u32);
        let mut tiles = vec![0i32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                tiles[(y * w + x) as usize] = if edge { 1 } else { 0 };
            }
        }

        let _ = with_node_mut!(ctx.run, TileMap2D, ctx.id, |map| {
            map.width = w;
            map.height = h;
            map.tiles = tiles;
            map.collision_enabled = true;
        });
    }
});
```

## Reference

`TileMap2D` is the runtime 2D tile map node.
It draws atlas tiles from a `.ptileset` and can emit static 2D colliders.

## Scene

```text
[level]
    [TileMap2D]
        tileset = "res://tiles/world.ptileset"
        width = 8
        height = 4
        empty_tile = -1
        tiles = [
            1, 1, 1, 1, 1, 1, 1, 1,
            1, -1, -1, -1, -1, -1, -1, 1,
            1, -1, -1, -1, -1, -1, -1, 1,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]
        collision_enabled = true
        collision_layers = [1]
        collision_mask = []
    [/TileMap2D]
[/level]
```

## Fields

- `tileset`: `.ptileset` path.
- `width` / `height`: tile grid dimensions.
- `tiles`: row-major tile ids.
- `empty_tile`: id skipped by draw and collision; default `-1`.
- `visible`: draw toggle.
- `z_index`: 2D draw order.
- `collision_enabled`: enables generated static colliders.
- `collision_layers`: generated collider tagged layers.
- `collision_mask`: generated collider ignored layers.

Collision tiles also cast 2D light shadows when `collision_enabled = true`.
Auto rectangles share the collision bake merge; explicit shapes keep their
circle, triangle, or convex polygon silhouette. See [2D Shadows](../resources/shadows2d.md).

## Collision Bake

Collision comes from the tileset.
Each tile decides if it wants collider generation.

`collision = true` with no shape uses `collision_shape = "auto"`.
`auto` builds a full tile bounds collider.
Adjacent auto tiles merge into larger rect colliders at runtime.

Explicit tile shapes stay as per-tile colliders.
Runtime supports `rect`, `circle`, `triangle`, and convex `polygon` explicit shapes.

Polygon example:

```text
collision_shape = { polygon = { points = [(0, 0), (16, 0), (8, 16)] offset = (0, 0) } }
```

## Runtime And Static Pipeline

Runtime bake hashes tile grid plus tileset collision data.
It rebuilds only when tile content or collision metadata changes.

Static pipeline parses `.ptileset` into packed `PTSET` bytes.
Those bytes include per-tile collision metadata.
Static runtime loads binary tilesets by path hash before disk.

Static release builds load binary `.ptileset` data from the static asset lookup.
Dynamic or edited tilemaps still use runtime collision bake.
