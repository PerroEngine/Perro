# Sky Demo

Scenes:

- `res://scenes/demos/sky.scn`

Shows:

- `Sky3D`
- day/evening/night gradients
- horizon colors
- custom sky shader stack
- scene light matching sky

Why scene works this way:

- Each scene has one `Sky3D`.
- `Sky3D` sits at root so it affects whole scene.
- Terrain and marker meshes give horizon/depth reference.
- Time is unpaused with low scale so sky changes slowly.
- Separate ray light shows scene lighting stays explicit.

Scene map:

| Node           | Role                               |
| -------------- | ---------------------------------- |
| `Sky`          | Main sky resource node.            |
| `Terrain`      | Ground reference.                  |
| `SkyMarkerA/B` | Objects for sky lighting contrast. |
| `Sun`          | Explicit directional scene light.  |
| `Ambient`      | Base scene fill.                   |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
