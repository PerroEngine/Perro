# Node Collections

Scene node templates were the old copy/paste `.scn` authoring docs.

Runtime code should use [Node Collections](../node_collections.md).

Use node collections for:

- flat batches
- nested trees
- reusable child collections
- mixed 2D/3D/UI graphs
- one `create_nodes!` spawn op

Use `.scn` files when the asset/editor scene format is needed.

Legacy `.scn` node field references still exist in this folder:

- [2D `.scn` fields](2d.md)
- [3D `.scn` fields](3d.md)
- [UI `.scn` fields](ui.md)
- [Extra `.scn` examples](examples.md)

