# Water Demo

Scene:

- `res://scenes/demos/water.scn`

Scripts:

- `res://scripts/water_demo.rs`

Projectile:

- `res://scenes/demos/cannon_ball.scn`

Shows:

- one `WaterBody3D` pool
- shooting `RigidBody3D` balls
- runtime scene spawn
- runtime body mass/velocity edits
- runtime collision shape scaling

Why scene works this way:

- Projectiles live in separate scene so spawning is cheap and clean.
- Root script caches camera and projectile parent once at init.
- Ball radius changes mass and collision radius together.
- `RigidBody3D` handles water buoyancy because water affects rigid bodies.
- Separate `Projectiles` parent keeps spawned nodes easy to unload.

Script flow:

| Step                      | Why                                   |
| ------------------------- | ------------------------------------- |
| Read camera transform     | Use aim direction from view.          |
| Load cannon ball scene    | Keep projectile setup reusable.       |
| Reparent to `Projectiles` | Keep runtime nodes grouped.           |
| Set world position        | Spawn in front of camera.             |
| Set body velocity         | Shoot along camera forward.           |
| Scale mesh + shape        | Keep visuals and collision same size. |

Controls:

| Input             | Action             |
| ----------------- | ------------------ |
| Mouse             | Look               |
| `W` `A` `S` `D`   | Move               |
| `Space` / `Shift` | Up / down          |
| Left mouse        | Shoot ball         |
| Mouse wheel       | Change ball radius |
| `Esc`             | Pause              |
