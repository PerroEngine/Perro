# Lights Demo

Scene:

- `res://scenes/demos/lights.scn`

Shows:

- `AmbientLight3D`
- `PointLight3D`
- `SpotLight3D`
- emissive unlit marker meshes
- lit material response

Why scene works this way:

- Dark ambient makes colored lights obvious.
- Marker spheres show light positions.
- Center sphere shows specular/highlight response.
- Floor catches falloff and spot cone.

Scene map:

| Node                                 | Role                             |
| ------------------------------------ | -------------------------------- |
| `LightsDemo`                         | Demo root.                       |
| `DemoCamera`                         | Fly camera.                      |
| `Ambient`                            | Low base light.                  |
| `RedLightMarker` / `BlueLightMarker` | Visual markers for point lights. |
| `RedPoint` / `BluePoint`             | Colored point lights.            |
| `Spot`                               | Warm cone light.                 |
| `CenterSphere`                       | Main lit test object.            |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
