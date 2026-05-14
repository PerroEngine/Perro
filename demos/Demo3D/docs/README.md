# Demo3D Docs

Run:

```powershell
cargo run -p perro_cli -- dev --path demos\Demo3D
```

Open hub.
Pick demo button.
Press `Esc` for pause.

Common controls:

| Input           | Action         |
| --------------- | -------------- |
| Mouse           | Look           |
| `W` `A` `S` `D` | Move camera    |
| `Space`         | Move up        |
| `Shift`         | Move down      |
| `Esc`           | Pause / resume |

Shared files:

| File                             | Why                                                    |
| -------------------------------- | ------------------------------------------------------ |
| `res/main.scn`                   | Loads hub root and `DemoManager`.                      |
| `res/scripts/demo_manager.rs`    | Preloads demos, swaps active demo, owns pause/fade UI. |
| `res/scripts/demo_freecam_3d.rs` | Gives each demo same fly camera controls.              |
| `res/scenes/demos/*.scn`         | One isolated scene per feature.                        |

Demo table:

| Demo             | Scene                                     | Script                     | Shows                                               | Docs                                       |
| ---------------- | ----------------------------------------- | -------------------------- | --------------------------------------------------- | ------------------------------------------ |
| Mesh + Materials | `res://scenes/demos/mesh_materials.scn`   | shared camera              | Built-in meshes, standard/toon/unlit materials.     | [mesh_materials.md](mesh_materials.md)     |
| Lights           | `res://scenes/demos/lights.scn`           | shared camera              | Ambient, point, spot, emissive markers.             | [lights.md](lights.md)                     |
| Water            | `res://scenes/demos/water.scn`            | shared camera              | `WaterBody3D` visual + sim settings.                | [water.md](water.md)                       |
| Water Cannon     | `res://scenes/demos/water_cannon.scn`     | `water_cannon_demo.rs`     | Rigid projectiles into water bodies.                | [water_cannon.md](water_cannon.md)         |
| Animations       | `res://scenes/demos/animations.scn`       | shared camera              | `.panim` clips on `AnimationPlayer`.                | [animations.md](animations.md)             |
| Sky              | `res://scenes/demos/sky.scn`              | shared camera              | `Sky3D` time, clouds, sun/moon/stars.               | [sky.md](sky.md)                           |
| Mesh Blending    | `res://scenes/demos/mesh_blending.scn`    | shared camera              | Mesh blend flags/layers/distances.                  | [mesh_blending.md](mesh_blending.md)       |
| MultiMesh        | `res://scenes/demos/multimesh.scn`        | shared camera              | Batched mesh instances.                             | [multimesh.md](multimesh.md)               |
| Particles        | `res://scenes/demos/particles.scn`        | shared camera              | `ParticleEmitter3D` profiles.                       | [particles.md](particles.md)               |
| Positional Audio | `res://scenes/demos/positional_audio.scn` | `positional_audio_demo.rs` | Audio mask, reverb zone, debug rays.                | [positional_audio.md](positional_audio.md) |
| Physics Bones    | `res://scenes/demos/physics_bones.scn`    | `physics_bones_demo.rs`    | glTF rig, `.panim` bones, physics chain collisions. | [physics_bones.md](physics_bones.md)       |

Why scenes stay split:

- Each demo loads fast.
- Each feature has small `.scn`.
- Shared camera/script logic avoids repeat.
- Hub can preload all demo scenes.
- Restart just reloads active scene.

Why scripts stay small:

- Static demos need no script beyond camera.
- Interactive demos own one script at root.
- Projectile behavior stays in projectile scene.
- Scene files keep data visible.
- Rust scripts keep runtime logic typed.
