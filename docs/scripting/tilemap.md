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
Runtime supports `rect`, `circle`, and `triangle` explicit shapes.
Polygon remains planned.

## Runtime And Static Pipeline

Runtime bake hashes tile grid plus tileset collision data.
It rebuilds only when tile content or collision metadata changes.

Runtime bake hashes tile grid plus tileset collision data.
It rebuilds only when tile content or collision metadata changes.

Static pipeline now emits `.ptileset` source into static assets.
Static runtime loads tilesets from that lookup before disk.

Static collision chunk bake remains planned:

- pre-bake fixed scene tilemap collision chunks
- use static chunks before runtime bake
