# Water Demo

Scene:

- `res://scenes/demos/water.scn`

Shows:

- `WaterBody3D`
- cube water bounds
- simulation resolution
- flow/wind/wave settings
- shallow/deep color
- sample readback config

Why scene works this way:

- One large water body makes all water fields easy to see.
- Lake bed sits under water so depth/color has context.
- `FloatMarker` gives a visible reference near water surface.
- LOD/readback fields show real runtime tuning knobs.

Scene map:

| Node              | Role                      |
| ----------------- | ------------------------- |
| `WaterDemo`       | Demo root.                |
| `DemoCamera`      | Fly camera.               |
| `LakeBed`         | Visual bottom.            |
| `Water`           | Main `WaterBody3D`.       |
| `FloatMarker`     | Surface height reference. |
| `Sun` / `Ambient` | Readable water lighting.  |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
