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
| Steamworks integration       | partial | Project `[steam]` config, runtime init/callback pump, achievements, stats, leaderboards, apps/DLC entitlement, friends, rich presence, overlay/invites, lobbies, P2P/networking, server browser helpers, cloud saves, workshop/UGC, input, remote play, screenshots, timeline, utils, and event polling exist through `perro_api::prelude::*`. See [Steamworks](steamworks.md). |

## 2D

| Area                    | Status  | Notes                                                                                                                            |
| ----------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `Node2D` transform      | done    | Position, rotation, scale, z index, visibility.                                                                                  |
| `Sprite2D`              | done    | Textured quad with optional pixel texture region for atlas frames.                                                               |
| `Camera2D`              | done    | Active camera with zoom and camera post-processing.                                                                              |
| 2D physics bodies       | done | Static, rigid, area, collision shapes, layers/masks, joints, raycast, shape cast, query filters, contact data, and area/body signals exist. See [Physics Module](scripting/contexts/runtime_modules/physics.md). |
| Draw2D transient shapes | done | Circle/ring/rect, lines, polylines, polygon outlines, paths, and transient sprites exist.                                    |
| `AnimatedSprite2D`      | done    | Sprite-sheet playback from normal texture paths plus named strip/grid animation definitions.                                     |
| Tile maps               | partial | `TileMap2D` plus `.ptileset` runtime/static path exists: one texture atlas, tile ids, empty tile `-1`, draw extraction, merged runtime 2D collision bake for `collision = true` auto tiles, and explicit rect/circle/triangle/polygon collision shapes. Pre-baked static collision chunks remain. See [TileMap2D](scripting/tilemap.md) and [`.ptileset`](resources/ptileset.md). |
| 2D skeleton nodes       | done    | `Skeleton2D` owns `Vec<Bone2D>` data loaded from `.pskel2d`; attachment, IK target, physics chain, and bone collider nodes mirror the 3D rig workflow. |
| 2D particles            | done    | `ParticleEmitter2D` uses `.ppart` profiles; `z` fields are ignored.                                                              |
| 2D lights               | done | `AmbientLight2D`, `RayLight2D`, `PointLight2D`, and `SpotLight2D` render unshadowed additive 2D lights. Shadows remain future work.   |

## 3D

| Area                       | Status   | Notes                                                                                                                                                               |
| -------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mesh rendering             | done     | Mesh instances, surfaces, material bindings, and meshlet path exist.                                                                                                |
| Multi-mesh rendering       | done     | Repeated static mesh instances.                                                                                                                                     |
| 3D cameras                 | done     | Perspective, orthographic, and frustum settings.                                                                                                                    |
| 3D lights                  | done     | Ambient, sky, ray, point, and spot lights.                                                                                                                          |
| 3D shadows                 | partial  | Shadow path exists for 3D lights/casters. More control and docs are still needed.                                                                                   |
| 3D particles               | done     | `ParticleEmitter3D` driven by `.ppart` profiles.                                                                                                                    |
| 3D physics                 | done  | Bodies, areas, primitive shapes, trimesh collision, raycast, primitive shape cast, query filters, contacts, area overlap signals, layers/masks, and core joint nodes exist. Trimesh as cast shape is not a 1.0 target. See [Physics Module](scripting/contexts/runtime_modules/physics.md). |
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
| UI animated image node | done | `UiAnimatedImage` renders sprite-sheet animations in UI space with `UiImage` scale/alignment behavior. |
| UI style resources    | done | Inline `style = { ... }` blocks and `res://path/to/style.uistyle` load for normal/hover/pressed/focused state styles, mirroring material resource flow. See [`.uistyle`](resources/uistyle.md). |

## Tooling And Demos

| Area             | Status  | Notes                                                                             |
| ---------------- | ------- | --------------------------------------------------------------------------------- |
| CLI project flow | done    | `new`, `check`, `dev`, `build`, DLC, profiling, format, clippy, clean.            |
| `perro doctor`   | done    | Checks project config, scene/resource refs, and user script path/member warnings. |
| Demo hubs        | planned | Target: `playground/Demo2D` and `playground/Demo3D` with UI scene lists and docs. |

## Planned Work Packets

1. Static tilemap collision chunks for fixed scene `TileMap2D` data.
2. Demo hubs, 3D LOD controls, and material docs.
3. Joint polish: optional limits/motors/springs if needed.
