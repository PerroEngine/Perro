# Physics Bones Demo

Scene:

- `res://scenes/demos/physics_bones.scn`

Files:

- `res/models/bone_cubes.gltf`: glTF 2.0 test rig with three cube segments and three bones.
- `res/animations/bone_cubes_wave.panim`: imported bone clip from the glTF animation.
- `res/animations/cube_spin_move.panim`: generic `Node3D` cube motion clip.
- `res/scenes/demos/physics_bones.scn`: example scene.
- `res/scenes/demos/physics_bone_projectile.scn`: projectile collider scene.
- `res/scripts/physics_bones_demo.rs`: camera shoot flow.
- `res/scripts/physics_bone_projectile.rs`: moving `BoneCollider3D` projectile.

Shows:

- glTF 2.0 skeleton import.
- glTF animation import via `import_anim`.
- `.panim` bone tracks on `Skeleton3D`.
- generic `.panim` `Node3D` cube movement.
- `PhysicsBoneChain3D` with collisions.
- moving `BoneCollider3D` projectile.

Run:

```powershell
cargo run -p perro_cli -- dev --path demos\Demo3D
```

Load `res://scenes/demos/physics_bones.scn` from the demo manager or set it as a test scene.

Import clip again:

```powershell
cargo run -p perro_cli -- import_anim demos\Demo3D\res\models\bone_cubes.gltf --output demos\Demo3D\res\animations\bone_cubes_wave.panim --clip BoneCubesWave --fps 30 --skeleton Rig
```

Scene wiring:

- `Skeleton3D.skeleton = "res://models/bone_cubes.gltf:skeleton[0]"`
- `MeshInstance3D.mesh = "res://models/bone_cubes.gltf:mesh[0]"`
- `MeshInstance3D.skeleton = @Rig`
- `AnimationPlayer.bindings = { Rig = @Rig }`
- `PhysicsBoneChain3D.skeleton = @Rig`
- `PhysicsBoneChain3D.bone = 2`
- projectile root uses `BoneCollider3D`
- projectile child uses `CollisionShape3D`

Why scene works this way:

- glTF owns mesh, skin, bones, and source animation in one file.
- `import_anim` converts glTF clip to `.panim` so Perro runtime uses normal animation path.
- `Skeleton3D` loads `:skeleton[0]`.
- `MeshInstance3D` loads `:mesh[0]` and binds to `@Rig`.
- `AnimationPlayer` binds clip object `Rig` to scene node `@Rig`.
- `PhysicsBoneChain3D` writes solved positions back into bone pose.
- Projectile uses `BoneCollider3D` because bone chains query bone colliders, not rigid bodies.

Script flow:

| Step                                | Why                                          |
| ----------------------------------- | -------------------------------------------- |
| Cache camera/projectile parent      | Avoid per-shot scene lookups.                |
| On left click load projectile scene | Keep projectile setup reusable.              |
| Reparent projectile                 | Keep runtime objects grouped.                |
| Call `launch`                       | Let projectile own velocity/radius setup.    |
| Projectile moves `BoneCollider3D`   | Chain collision sees it during fixed update. |

Physics note:

`PhysicsBoneChain3D` collision source is `BoneCollider3D`, not `RigidBody3D`.
The projectile moves a `BoneCollider3D` sphere so chain points push out during fixed update.

Controls:

| Input             | Action                         |
| ----------------- | ------------------------------ |
| Mouse             | Look                           |
| `W` `A` `S` `D`   | Move                           |
| `Space` / `Shift` | Up / down                      |
| Left mouse        | Shoot bone collider projectile |
| Mouse wheel       | Change projectile radius       |
| `Esc`             | Pause                          |
