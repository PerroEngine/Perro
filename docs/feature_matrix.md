# Feature Matrix

This page tracks current Perro capability in plain terms.

Status keys:

- `done`: implemented and documented enough to use.
- `partial`: implemented core path, but important game-making pieces are missing.
- `planned`: not a current engine feature.
- `research`: design needed before implementation.

## Core Runtime

| Area                         | Status  | Notes                                                                                                                                     |
| ---------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Scene tree + `NodeID` access | done    | Parent/child scene model with typed node access.                                                                                          |
| Rust behavior scripts        | done    | Lifecycle, methods, state, and generated script registry.                                                                                 |
| Query layer                  | done    | Query by type, base type, tag, name, and subtree.                                                                                         |
| Scene preload                | done    | `scene_preload!` parses and stores a scene as `PreloadedSceneID`; loading copies nodes from the cached scene instead of reparsing.        |
| Resource load IDs            | done    | Texture/mesh/audio load calls return IDs immediately and queue backend work. A node can hold the ID before the backend resource is ready. |
| Save data helpers            | done    | `perro_modules::file` can read assets and write to `user://` or absolute paths.                                                           |
| Runtime window config API    | planned | `project.toml` covers startup config. Runtime changes to window mode/title/size need an API.                                              |

## 2D

| Area                    | Status  | Notes                                                                                                                            |
| ----------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `Node2D` transform      | done    | Position, rotation, scale, z index, visibility.                                                                                  |
| `Sprite2D`              | partial | Textured quad only. No texture region, atlas frame, or sprite-sheet animation yet.                                               |
| `Camera2D`              | done    | Active camera with zoom and camera post-processing.                                                                              |
| 2D physics bodies       | partial | Static, rigid, area, and collision shapes exist. Raycast, shape cast, layers/masks, joints, and richer contact data are planned. |
| Draw2D transient shapes | partial | Circle/ring/rect debug-style draws exist. Lines, polys, paths, and atlas sprites are planned.                                    |
| Animated sprite sheets  | planned | First target: strip/grid frame fields on an `AnimatedSprite2D` node.                                                             |
| Tile maps               | planned | Target: one texture atlas plus tile ids, with empty tile value `-1`.                                                             |
| 2D particles            | planned | Should mirror `ParticleEmitter3D` where practical.                                                                               |
| 2D lights               | planned | First target: unshadowed point/additive light pass; shadows later.                                                               |
| Parallax2D              | planned | Deferred.                                                                                                                        |

## 3D

| Area                       | Status   | Notes                                                                                                     |
| -------------------------- | -------- | --------------------------------------------------------------------------------------------------------- |
| Mesh rendering             | done     | Mesh instances, surfaces, material bindings, and meshlet path exist.                                      |
| Multi-mesh rendering       | done     | Repeated static mesh instances.                                                                           |
| 3D cameras                 | done     | Perspective, orthographic, and frustum settings.                                                          |
| 3D lights                  | done     | Ambient, sky, ray, point, and spot lights.                                                                |
| 3D shadows                 | partial  | Shadow path exists for 3D lights/casters. More control and docs are still needed.                         |
| 3D particles               | done     | `ParticleEmitter3D` driven by `.ppart` profiles.                                                          |
| 3D physics                 | partial  | Bodies, areas, primitive shapes, and trimesh collision exist. Raycast/layers/contact parity work remains. |
| Skeleton skinning          | done     | A `MeshInstance3D` can bind to a `Skeleton3D` and use mesh weights.                                       |
| Shared-skeleton mesh reuse | done     | Works when the mesh uses the same rig contract: matching joint order/indices and compatible weights.      |
| Automatic retargeting      | research | Bone-name remap, rest-pose solve, and mismatched rig conversion are not implemented.                      |
| LOD                        | planned  | Target: static pipeline generates LODs and packs them into a future `.pmesh` layout.                      |
| Terrain                    | planned  | Deferred because scope is large.                                                                          |
| Decals                     | research | Interesting, but needs render design before roadmap inclusion.                                            |
| Navmesh                    | research | Needs design and use-case clarity.                                                                        |

## UI

| Area                  | Status  | Notes                                                                                |
| --------------------- | ------- | ------------------------------------------------------------------------------------ |
| Panels/buttons/labels | done    | Data nodes plus retained UI render path.                                             |
| Text input            | done    | One-line and multi-line text edit nodes.                                             |
| Layout nodes          | partial | H/V/grid/tree list exist. Scroll containers and richer focus navigation are planned. |
| UI image node         | planned | Needed for image-heavy UI and demo hubs.                                             |
| Themes/font assets    | planned | Current UI styling is mostly per-node scene/script data.                             |

## Tooling And Demos

| Area             | Status  | Notes                                                                             |
| ---------------- | ------- | --------------------------------------------------------------------------------- |
| CLI project flow | done    | `new`, `check`, `dev`, `build`, DLC, profiling, format, clippy, clean.            |
| `perro doctor`   | planned | Toolchain/project health checks.                                                  |
| `perro validate` | planned | Scene/resource/script reference checks.                                           |
| Demo hubs        | planned | Target: `playground/Demo2D` and `playground/Demo3D` with UI scene lists and docs. |
| Visual editor    | planned | Deferred.                                                                         |

## Planned Work Packets

1. Docs truth pass and feature matrix.
2. `AnimatedSprite2D` plus sprite-strip/grid frames.
3. `TileMap2D` plus `TileSet`.
4. 2D physics parity: casts, layers/masks, contact details.
5. 2D particles plus unshadowed 2D light.
6. Demo hubs, CLI doctor/validate, 3D LOD research, material docs.
