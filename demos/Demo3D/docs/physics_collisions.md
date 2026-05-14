# Physics Collisions

Shows:

- `RigidBody3D` vs `RigidBody3D`
- `RigidBody3D` vs `StaticBody3D`
- `RigidBody3D` through `Area3D`
- `Area3D` overlap signal -> material color swap

Files:

| File | Why |
| --- | --- |
| `res/scenes/demos/physics_collisions.scn` | Bodies, static shapes, trigger area. |
| `res/scripts/physics_collisions_demo.rs` | Reset loop + area signal handlers. |

Signals:

| Signal | Params | Result |
| --- | --- | --- |
| `SignalArea_Entered` | area + body | Area mesh turns orange. |
| `SignalArea_Exited` | area + body | Area mesh returns blue. |

Scene loops every 7 seconds.
