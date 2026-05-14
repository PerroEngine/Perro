# Lights Demo

Scene:

- `res://scenes/demos/lights.scn`

Shows:

- `AmbientLight3D`
- `RayLight3D`
- `PointLight3D`
- `SpotLight3D`
- emissive unlit marker meshes
- standard, toon, metallic, and alpha blend materials
- cube, sphere, pyramid, prism, cylinder, cone, and capsule meshes
- scripted orbit, sweep, bob, and spot tracking motion

Why scene works this way:

- Dark ambient makes colored lights obvious.
- Marker meshes show light positions.
- Eight colored point lights use different motion paths.
- Two spot lights track toward the object cluster.
- Mixed materials show diffuse bands, metal highlights, and alpha blend.
- Floor and back wall catch falloff and spot cones.

Scene map:

| Node                                 | Role                             |
| ------------------------------------ | -------------------------------- |
| `LightsDemo`                         | Demo root.                       |
| `DemoCamera`                         | Fly camera.                      |
| `Ambient`                            | Low base light.                  |
| `KeyRay`                             | Cool directional fill light.     |
| `*Orb`                               | Visual markers for point lights. |
| `*Point`                             | Colored point lights.            |
| `WhiteSpotRig` / `GoldSpotRig`       | Moving spot light rigs.          |
| `MatteSphere` / shape nodes          | Material and shape tests.        |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
