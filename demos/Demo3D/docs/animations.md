# Animation Demo

Scene:

- `res://scenes/demos/animations.scn`

Clips:

- `res://animations/demo_bob.panim`
- `res://animations/demo_pulse.panim`
- `res://animations/cube_spin_move.panim`
- `res://animations/demo_tilt_stretch.panim`
- `res://animations/demo_drift_resize.panim`

Shows:

- `AnimationPlayer`
- `.panim` clip playback
- clip-local object names
- scene bindings
- loop playback
- speed variation
- anchor nodes for separated world locations

Most clips declare `Hero = Node3D`.
`cube_spin_move.panim` declares `Cube = Node3D`.
Scene bindings map clip-local names to each visible mesh root.

Why scene works this way:

- One clip can drive many scene nodes through different bindings.
- `.panim` stores clip object names, not scene node IDs.
- `AnimationPlayer` owns playback mode/speed/pause in scene.
- Parent anchor nodes keep each animation in its own world-space area while clips move local child transforms.

Scene map:

| Node                   | Role                                           |
| ---------------------- | ---------------------------------------------- |
| `BobA`                 | Cube driven by `demo_bob.panim`.               |
| `PulseA`               | Sphere driven by `demo_pulse.panim`.           |
| `SpinMove`             | Cube driven by `cube_spin_move.panim`.         |
| `TiltStretch`          | Sphere driven by `demo_tilt_stretch.panim`.    |
| `DriftResize`          | Cube driven by `demo_drift_resize.panim`.      |
| `*Origin`              | Parent anchors with distinct world positions.  |
| `*Player`              | Animation players bound to visible mesh nodes. |
| `ClipNote`             | UI note for clip path.                         |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
