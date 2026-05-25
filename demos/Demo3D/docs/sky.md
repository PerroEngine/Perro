# Sky Demo

Scenes:

- `res://scenes/demos/sky.scn`
- `res://scenes/demos/sky_wispy.scn`

Shows:

- `Sky3D`
- day/evening/night gradients
- sun and moon sizes
- cloud settings
- cloud shader mode (`VOLUMETRIC` or `WISPY`)
- `DEFAULT` or custom cloud/sun/moon shaders
- wind vector
- star controls
- scene light matching sky

Why scene works this way:

- Each scene has one `Sky3D` mode.
- `Sky3D` sits at root so it affects whole scene.
- Terrain and marker meshes give horizon/depth reference.
- Time is unpaused with low scale so sky changes slowly.
- Separate sun light shows how sky visuals and scene lighting pair.

Scene map:

| Node           | Role                               |
| -------------- | ---------------------------------- |
| `Sky`          | Main sky resource node.            |
| `Terrain`      | Ground reference.                  |
| `SkyMarkerA/B` | Objects for sky lighting contrast. |
| `Sun`          | Directional scene light.           |
| `Ambient`      | Base scene fill.                   |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
