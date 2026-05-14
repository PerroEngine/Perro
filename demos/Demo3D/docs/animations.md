# Animation Demo

Scene:

- `res://scenes/demos/animations.scn`

Clips:

- `res://animations/demo_bob.panim`
- `res://animations/demo_pulse.panim`

Shows:

- `AnimationPlayer`
- `.panim` clip playback
- clip-local object names
- scene bindings
- loop playback
- speed variation

Each clip declares `Hero = Node3D`.
Scene bindings map `Hero` to each visible mesh root.

Why scene works this way:

- One clip can drive many scene nodes through different bindings.
- `.panim` stores clip object names, not scene node IDs.
- `AnimationPlayer` owns playback mode/speed/pause in scene.
- Several copies show reuse without duplicating clip files.

Scene map:

| Node           | Role                                     |
| -------------- | ---------------------------------------- |
| `BobA/B/C`     | Meshes driven by `demo_bob.panim`.       |
| `PulseA/B`     | Meshes driven by `demo_pulse.panim`.     |
| `Bob*Player`   | Animation players bound to cube nodes.   |
| `Pulse*Player` | Animation players bound to sphere nodes. |
| `ClipNote`     | UI note for clip path.                   |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
