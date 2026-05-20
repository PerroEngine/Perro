# Mesh + Materials Demo

Scene:

- `res://scenes/demos/mesh_materials.scn`

Shows:

- Built-in cube and sphere meshes.
- Inline `standard` material.
- Inline `toon` material.
- Roughness and metallic changes.
- Runtime asymmetric mesh mirrored by `flip_x` / `flip_y`.
- Shared free camera.

Why scene works this way:

- Inline materials keep example self-contained.
- Simple built-in meshes remove asset dependency.
- Mirror samples share one runtime `Mesh3D`; only node flip bits differ.
- Three objects sit under one root for easy unload.
- Floor gives lighting and material contrast.

Scene map:

| Node                   | Role                              |
| ---------------------- | --------------------------------- |
| `MeshMaterialsDemo`    | Root for unload/restart.          |
| `DemoCamera`           | Shared freecam script.            |
| `Ambient` + `KeyLight` | Base readable lighting.           |
| `Floor`                | Reference plane.                  |
| `RedCube`              | Standard rough nonmetal material. |
| `BlueSphere`           | Smoother metallic-ish material.   |
| `GreenCube`            | Toon material path.               |
| `MirrorOriginal`       | Runtime asymmetric mesh.          |
| `MirrorFlipX`          | Same mesh, GPU mirrored on X.     |
| `MirrorFlipY`          | Same mesh, GPU mirrored on Y.     |
| `MirrorFlipXY`         | Same mesh, GPU mirrored on X + Y. |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
