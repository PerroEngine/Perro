# Feature Matrix

This page tracks current Perro capability in plain terms.

Status keys:

- `done`: implemented and documented enough to use.
- `partial`: implemented core path, but important game-making pieces are missing.
- `planned`: not a current engine feature.
- `research`: design needed before implementation.

## Core Runtime

| Area                         | Status | Notes                                                                                                                                     |
| ---------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Scene tree + `NodeID` access | done   | Parent/child scene model with typed node access.                                                                                          |
| Rust behavior scripts        | done   | Lifecycle, methods, state, and generated script registry.                                                                                 |
| Query layer                  | done   | Query by type, base type, tag, name, and subtree.                                                                                         |
| Scene preload                | done   | `scene_preload!` parses and stores a scene as `PreloadedSceneID`; loading copies nodes from the cached scene instead of reparsing.        |
| Resource load IDs            | done   | Texture/mesh/audio load calls return IDs immediately and queue backend work. A node can hold the ID before the backend resource is ready. |
| Save data helpers            | done   | `perro_modules::file` can read assets and write to `user://` or absolute paths.                                                           |
| Runtime window config API    | done   | `RuntimeWindow::Window()` queues runtime changes to window mode/title/size.                                                               |

## 2D

| Area                    | Status  | Notes                                                                                                                            |
| ----------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `Node2D` transform      | done    | Position, rotation, scale, z index, visibility.                                                                                  |
| `Sprite2D`              | done    | Textured quad with optional pixel texture region for atlas frames.                                                               |
| `Camera2D`              | done    | Active camera with zoom and camera post-processing.                                                                              |
| 2D physics bodies       | partial | Static, rigid, area, collision shapes, layers/masks, and core joint nodes exist. 1.0 parity target still needs 2D raycast, shape cast, and contact data. See [Physics Module](scripting/contexts/runtime_modules/physics.md). |
| Draw2D transient shapes | partial | Circle/ring/rect debug-style draws exist. Lines, polys, paths, and atlas sprites are planned.                                    |
| `AnimatedSprite2D`      | done    | Sprite-sheet playback from normal texture paths plus named strip/grid animation definitions.                                     |
| Tile maps               | partial | `TileMap2D` plus `.ptileset` runtime path exists: one texture atlas, tile ids, empty tile `-1`, draw extraction, and merged runtime 2D collision bake for `collision = true` auto tiles. Static pipeline bake and explicit collision shapes remain. See [TileMap2D](scripting/tilemap.md) and [`.ptileset`](resources/ptileset.md). |
| 2D skeleton nodes       | done    | `Skeleton2D` is a 2D transform parent. `Bone2D` is a child `Node2D` with rest/pose/inv_bind data and normal `Node2D` animation tracks. |
| 2D particles            | done    | `ParticleEmitter2D` uses `.ppart` profiles; `z` fields are ignored.                                                              |
| 2D lights               | planned | First target: unshadowed point/additive light pass; shadows later.                                                               |

## 3D

| Area                       | Status   | Notes                                                                                                                                                               |
| -------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mesh rendering             | done     | Mesh instances, surfaces, material bindings, and meshlet path exist.                                                                                                |
| Multi-mesh rendering       | done     | Repeated static mesh instances.                                                                                                                                     |
| 3D cameras                 | done     | Perspective, orthographic, and frustum settings.                                                                                                                    |
| 3D lights                  | done     | Ambient, sky, ray, point, and spot lights.                                                                                                                          |
| 3D shadows                 | partial  | Shadow path exists for 3D lights/casters. More control and docs are still needed.                                                                                   |
| 3D particles               | done     | `ParticleEmitter3D` driven by `.ppart` profiles.                                                                                                                    |
| 3D physics                 | partial  | Bodies, areas, primitive shapes, trimesh collision, raycast, contacts, area overlap signals, layers/masks, and core joint nodes exist. 1.0 parity target still needs shape cast and richer contact data. See [Physics Module](scripting/contexts/runtime_modules/physics.md). |
| Skeleton skinning          | done     | A `MeshInstance3D` can bind to a `Skeleton3D` and use mesh weights.                                                                                                 |
| Shared-skeleton mesh reuse | done     | Works when the mesh uses the same rig contract: matching joint order/indices and compatible weights.                                                                |
| Automatic retargeting      | research | Bone-name remap, rest-pose solve, and mismatched rig conversion are not implemented.                                                                                |
| LOD                        | done     | Automatic mesh LOD works for dynamic/dev loads and static `.pmesh` v8 builds. Skinned mesh LOD, per-node controls, and smarter simplify remain future improvements. |
| Decals                     | research | Interesting, but needs render design before roadmap inclusion.                                                                                                      |
| Navmesh                    | research | Needs design and use-case clarity.                                                                                                                                  |

## UI

| Area                  | Status  | Notes                                                             |
| --------------------- | ------- | ----------------------------------------------------------------- |
| Panels/buttons/labels | done    | Data nodes plus retained UI render path.                          |
| Text input            | done    | One-line and multi-line text edit nodes.                          |
| Layout nodes          | done    | H/V/grid/tree list layout nodes exist with retained invalidation. |
| Scroll containers     | done    | `UiScrollContainer` offsets child content and clips to its rect.  |
| Focus navigation      | partial | Text focus exists. Keyboard/controller traversal remains.         |
| UI image node         | done    | `UiImage` renders texture IDs with tint, region, scale mode, alignment, and aspect ratio. |
| UI style resources    | done | Inline `style = { ... }` blocks and `res://path/to/style.uistyle` load for normal/hover/pressed/focused state styles, mirroring material resource flow. See [`.uistyle`](resources/uistyle.md). |

## Tooling And Demos

| Area             | Status  | Notes                                                                             |
| ---------------- | ------- | --------------------------------------------------------------------------------- |
| CLI project flow | done    | `new`, `check`, `dev`, `build`, DLC, profiling, format, clippy, clean.            |
| `perro doctor`   | done    | Checks project config, scene/resource refs, and user script path/member warnings. |
| Demo hubs        | planned | Target: `playground/Demo2D` and `playground/Demo3D` with UI scene lists and docs. |

## Planned Work Packets

1. Docs truth pass and feature matrix.
2. Physics parity: 2D raycast, shape cast, and contact details.
3. `TileMap2D` plus `.ptileset`: atlas tiles, `empty_tile = -1`, `collision_shape = "auto"`, runtime bake, static pipeline bake.
4. UI style resources: inline `style = { ... }` plus `res://path/to/style.uistyle` for normal/hover/pressed/focused state styles.
5. 2D lights, demo hubs, 3D LOD controls, and material docs.
