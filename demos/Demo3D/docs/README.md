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
| Mouse wheel     | Change speed   |
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
| Water            | `res://scenes/demos/water.scn`            | `water_demo.rs`     | Rigid projectiles into one water body.              | [water.md](water.md)                       |
| Animations       | `res://scenes/demos/animations.scn`       | shared camera              | `.panim` clips on `AnimationPlayer`.                | [animations.md](animations.md)             |
| Sky3D            | `res://scenes/demos/sky.scn`              | shared camera              | `Sky3D` gradient, horizon colors, shader stack.      | [sky.md](sky.md)                           |
| Mesh Blending    | `res://scenes/demos/mesh_blending.scn`    | shared camera              | Mesh blend flags/layers/distances.                  | [mesh_blending.md](mesh_blending.md)       |
| MultiMesh        | `res://scenes/demos/multimesh.scn`        | shared camera              | Batched mesh instances.                             | [multimesh.md](multimesh.md)               |
| Particles        | `res://scenes/demos/particles.scn`        | shared camera              | `ParticleEmitter3D` profiles.                       | [particles.md](particles.md)               |
| Positional Audio | `res://scenes/demos/positional_audio.scn` | `positional_audio_demo.rs` | Audio mask, reverb zone, debug rays.                | [positional_audio.md](positional_audio.md) |
| Physics Bones    | `res://scenes/demos/physics_bones.scn`    | `physics_bones_demo.rs`    | glTF rig, `.panim` bones, physics chain collisions. | [physics_bones.md](physics_bones.md)       |
| Physics Collisions | `res://scenes/demos/physics_collisions.scn` | `physics_collisions_demo.rs` | Rigid/static contacts and `Area3D` color signal. | [physics_collisions.md](physics_collisions.md) |
| Decals           | `res://scenes/demos/decals.scn`           | `decals_demo.rs`           | `Decal3D` albedo/normal/emission projected onto lit geometry. | [decals.md](decals.md)                     |

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

This split follows ownership, not a one-role-per-script rule. A static lane has
no state owner beyond its scene, so another script would add dispatch without a
behavior boundary. Interactive roots get a script because they own mutable
state, lifecycle, or methods.

## Read A Lane

1. open the linked `.scn` -> inspect node topology, assets, and `script_vars`
2. open the linked script -> identify state owner and lifecycle hook
3. follow methods/signals/timers -> identify target and failure behavior
4. run the lane -> compare observed behavior with the authored flow

Use the [Script Authoring Guide](/docs/scripting/authoring/index.md) for
the fixed-ref/relation/query and method/signal/dynamic-var decisions.
