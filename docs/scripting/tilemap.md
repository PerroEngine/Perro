# TileMap2D

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
        collision_layer = 1
        collision_mask = 4294967295
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
- `collision_layer` / `collision_mask`: physics groups for generated colliders.

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
