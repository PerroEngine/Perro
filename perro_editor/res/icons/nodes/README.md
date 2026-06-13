# Node Icons

Editor node picker icons live here.

Fallback keys:

- `node.png`: base `Node`
- `node_2d.png`: any `Node2D`
- `node_3d.png`: any `Node3D`
- `ui_node.png`: any `UiNode`
- `resource.png`: resource nodes

Exact keys:

- `sprite_2d.png`: `Sprite2D`
- `mesh_3d.png`: `MeshInstance3D`, `MultiMeshInstance3D`
- `camera.png`: `Camera2D`, `Camera3D`
- `light.png`: light nodes
- `audio.png`: audio nodes
- `physics.png`: body/area/collider nodes

Picker source of truth stays `NodeType::ALL`.
Missing exact icon falls back by base type.
