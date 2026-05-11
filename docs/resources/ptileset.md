# `.ptileset` Format

`.ptileset` is the Perro 2D tile set resource.
It defines atlas layout and per-tile collision metadata for `TileMap2D`.

## Shape

```text
texture = "res://tiles/world.png"
tile_size = (16, 16)
columns = 8
rows = 8

tiles = [
    { id = 0 atlas = (0, 0) },
    { id = 1 atlas = (1, 0) collision = true },
    { id = 2 atlas = (2, 0) collision = true collision_shape = "auto" },
    { id = 3 atlas = (3, 0) collision = true collision_shape = { rect = { size = (16, 8) offset = (0, -4) } } },
    { id = 4 atlas = (4, 0) collision = true collision_shape = { polygon = { points = [(0, 0), (16, 0), (8, 16)] } } },
]
```

## Fields

- `texture`: atlas image path.
- `tile_size`: tile width and height in atlas pixels.
- `columns` / `rows`: atlas grid size.
- `tiles`: tile metadata array.

Tile fields:

- `id`: tile id used by `TileMap2D.tiles`.
- `atlas`: tile cell in the atlas grid.
- `collision`: opt in to collider generation.
- `collision_shape`: `auto` or explicit shape.

## Collision

`collision = false` is the default.
`collision = true` with no `collision_shape` means `collision_shape = "auto"`.

`auto` builds a full tile bounds collider.
Adjacent auto full-tile colliders are merge candidates.
Runtime tilemap bake merges them into maximal rect chunks.

Explicit shapes are kept per tile in 1.0.
They are not merged.

Explicit shapes:

- `rect`
- `circle`
- `triangle`
- `polygon`

Polygon points are local tile-space points.
Use at least 3 points.
Runtime builds a convex collider from the points.

## Static Bake

Static pipeline parses `.ptileset` into packed `PTSET` v1 bytes.
It bakes atlas layout and per-tile collision data into those bytes.
It emits those bytes into `static_assets::tilesets`.
Static runtime reads binary tileset bytes by path hash before disk.
Old binary version numbers are unsupported before public asset compatibility starts.
Rerun the static compiler to regenerate packed bytes.
