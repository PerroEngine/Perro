# `.pnav` navigation meshes

`.pnav` stores a static 3D triangle navigation mesh as UTF-8 text.

```text
pnav 1
v 0 0 0
v 2 0 0
v 0 0 2
v 5 0 0
v 7 0 0
v 5 0 2
tri 0 1 2 layers=1 area=1
tri 3 4 5 layers=1 area=2
link 0.5 0 0.5 5.5 0 0.5 layers=1 cost=1.25 snap=1 bidirectional=true
```

## Records

- `pnav 1` selects text format version 1.
- `v x y z` adds a vertex.
- `tri a b c` adds a triangle using zero-based vertex indices.
- `link sx sy sz ex ey ez` adds an off-mesh link.
- `#` starts a comment.

Triangle options:

- `layers=1,3` sets traversal layers. The default is all layers.
- `area=2` assigns an area ID from 1 through 32. The default is 1.

Link options:

- `layers=1,3` filters the link by query layers. The default is all layers.
- `cost=1.25` scales travel cost across the link. The value must be finite and greater than zero.
- `snap=1` sets maximum XZ distance used to attach each endpoint to a triangle. The default is 1.
- `bidirectional=false` creates a start-to-end link. The default is `true`.

Use links for jumps, doors, ladders, teleports, and separate mesh islands. A link whose endpoint cannot snap to an enabled triangle stays inactive for that layer query.

## Compatibility

`NavMesh3D` keeps the original vertices-and-triangles shape. Existing create, get, write, parse, and path calls keep working.

`NavMeshResource3D` carries the parallel triangle area list and off-mesh links. `parse_pnav_resource_bytes` and resource-aware API methods retain this metadata. Legacy parsers return the geometry and ignore extended metadata.

Static builds validate the full resource, including area and link fields, before embedding the original bytes.
