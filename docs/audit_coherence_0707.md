# Coherence Audit — API / Naming / Pattern Drift (2026-07-07)

Scope: `perro_source/` — node definitions, API modules, scene loader, render stack, variant system, and cross-cutting conventions. This is an inventory of things that *vary* across the codebase but express the same concept, with a recommended canonical form for each. No source changes were made.

All paths are relative to `perro_source/` unless otherwise noted.

## Summary Table

| # | Finding | Area | Dominant variant | Recommended canonical | Migration cost |
|---|---------|------|------------------|----------------------|----------------|
| 1.1 | `modulate` vs `tint` vs `color` for node color multiply | Nodes / scene files | `modulate` (base), `tint` (sprites/buttons), `color` (labels/lights) | `modulate` for multiply-color, `color` for intrinsic color (text, light) | Medium (scene parser already aliases all three) |
| 1.2 | `visible` vs `active` vs `enabled` vs `disabled` for on/off state | Nodes | Mixed; no dominant | `visible` = rendering, `active` = simulation/contribution, `enabled` = feature toggle; drop redundant pairs | Medium |
| 1.3 | "Sprite" (2D/3D) vs "Image" (UI, ImageButton2D) for textured quads | Node type names | `Sprite` in world space, `Image` in UI | Keep split but document; rename `ImageButton2D` or `UiImageButton` for parity | Low–Medium |
| 1.4 | `drop_` vs `free_` vs `release_` for resource disposal | APIs | `drop_` | `drop_` everywhere; `release_` only for held handles (MIDI notes) | Low |
| 2.1 | Lights use raw `[f32; 3]` for color; visuals use typed `Color` | Node fields | `Color` elsewhere | `Color` everywhere | Medium |
| 2.2 | `String`/`Cow<str>` asset references vs typed IDs (`TextureID`, `MeshID`) | Node fields | Typed IDs | Typed IDs (`TilesetID`, `ParticleProfileID` or equivalent) | Medium–High |
| 2.3 | Nested settings structs (Decal3D, MeshInstance3D) vs flat fields (Sky3D, particles) | Node fields | Flat (older), nested (newer) | Nested typed settings structs for grouped params (per Decal3D convention) | Medium |
| 2.4 | Some nodes embed `base: Node2D/3D`, others inline transform/visible | Node structs | `base` embedding | `base` embedding | Medium |
| 2.5 | `pub internal_*` state fields on public node structs | Node fields | Only particles | private fields or `#[doc(hidden)]` runtime-state struct | Low |
| 3.1 | `get_` prefix: always (Time), never (resource modules), mixed (Node, Physics) | API methods | Mixed | Bare nouns for getters, `set_` for setters (Rust convention) | High (public API) |
| 3.2 | `signal_connect` / `signal_emit` repeat module name; other modules don't | Runtime API | Un-prefixed elsewhere | Drop `signal_` prefix inside `Signals()` module | Low |
| 3.3 | `tag_set` (verb-last) vs `set_tags` / `set_node_name` (verb-first) | Node API | verb-first | verb-first | Low |
| 3.4 | Module accessor abbreviations: `AnimPlayer`/`AnimTree` (runtime) vs `Animations`/`AnimationTrees` (resource) | API facades | Full words on resource side | Pick one; suggest full words | Low |
| 3.5 | Error style: `Result<_, String>` vs `bool` vs `Option` vs typed error enums | APIs | `bool`/`Option` for node ops, `Result<_, String>` for IO | Typed error enums for IO paths; keep `Option`/`bool` for fast node ops but document | High |
| 4.1 | Scene-key alias sets differ for same concept across node kinds (`flip_h` accepted in one place, not another) | Scene loader | Aliased parsing | Central alias table per concept, reused | Medium |
| 4.2 | Scene keys flattened (`distance_fade_begin`) vs Rust nested (`distance_fade.begin`) | Scene loader vs structs | Flat keys | Accept nested + flat, document one | Low |
| 4.3 | prepare/nodes layout: 3D has `lights.rs`, `particles.rs`, `animation.rs`; 2D folds all into `base.rs` | Scene loader modules | 3D split | Split 2D the same way | Low |
| 5.1 | 2D bridge types (`ParticleProfile2D`, `PointParticles2DState`, `CameraStream3DState`) live in wrong-dimension files | Render bridge | — | Move to matching module | Low |
| 5.2 | Uniform struct naming: `Camera2DUniform`, `CameraUniform`, `Scene3DUniform`, `UiUniformGpu`, `UniformFrameCtx`, `PostUniform` | Graphics | `<Thing><Dim>Uniform` | `<Thing><Dim>Uniform`; Rust-side mirror gets no extra suffix | Low |
| 6.1 | `get_kind()` vs bare `kind_name()` on Variant | Variant | bare `as_*`/`is_*` family | `kind()` (bare) | Low |
| 6.2 | Variant suffix casing: `as_vec2` vs `as_transform2d` vs `as_matrix2x2` | Variant | `2`/`3` numeric suffix | unify on `_2d`/`_3d` or bare digit; pick one | Low |
| 7.1 | `*_version` stragglers after `*_revision` rename | Runtime | `*_revision` | rename remaining `built_at_version`, `physics_synced_node_version_*` | Trivial |
| 7.2 | `RuntimeWindow` (API facade) vs `Window()` (window module) name collision | Runtime API | — | Rename facade (e.g. `RuntimeApiSurface` / `Rt`) | Medium (public) |

---

## 1. Terminology Drift

### 1.1 `modulate` vs `tint` vs `color`

Three names for "color multiplied into the rendered output" coexist:

- **`modulate`** — the base-node system: `NodeModulate { modulate, self_modulate, children_modulate }` in `core/perro_structs/src/structs/modulate.rs:4-7`, embedded in both bases (`core/perro_nodes/src/nodes/node_2d/core/node_2d_base.rs:10`, `node_3d/core/node_3d_base.rs:9`). Also `Decal3D.modulate` (`node_3d/visual/decal_3d.rs:85`) and `MeshSurfaceBinding.modulate` (`node_3d/visual/mesh_instance_3d.rs:95`).
- **`tint`** — sprites and buttons: `NineSlice2D.tint` (`node_2d/visual/sprite_2d.rs:39`), `Sprite3D.tint` (`node_3d/visual/sprite_3d.rs:18`), `ImageButton2D.tint / hover_tint / pressed_tint` (`node_2d/visual/button_2d.rs:86-88`), and UI widgets (`core/perro_ui/src/widgets.rs:61,89,145,595`).
- **`color`** — labels and lights: `Label2D.color` (`node_2d/visual/label_2d.rs:14`), `Label3D.color` (`node_3d/visual/sprite_3d.rs:60`), all light nodes (`node_2d/lights/point_light_2d.rs:7` etc.), and UI text widgets (`core/perro_ui/src/widgets.rs:257,669,790`).

The scene parser papers over this by accepting all three as aliases nearly everywhere (`runtime_project/perro_runtime/src/runtime/scene_loader/prepare/nodes/two_d/base.rs:15` accepts `"tint" | "color" | "modulate"`; same pattern at `three_d/base.rs:20,115,169`), which is user-friendly but means the Rust field names carry no consistent meaning.

**Notable parity gap:** `Sprite2D` has *no* tint field (`node_2d/visual/sprite_2d.rs:25-29` — relies on base `modulate`), while `Sprite3D` carries an explicit `tint` (`sprite_3d.rs:18`). Same node concept, different color plumbing per dimension.

**Recommendation:** reserve `color` for intrinsic color (text glyphs, light emission), and one of `modulate`/`tint` for multiply-color — since `NodeModulate` is the load-bearing base mechanism, `modulate` is the cheaper canonical choice. Either give `Sprite2D` a `tint`→ or drop `Sprite3D.tint` in favor of base modulate so both dimensions match. Keep scene-file aliases for backward compatibility.

### 1.2 `visible` / `active` / `enabled` / `disabled`

Four different on/off vocabularies:

- Base nodes: `visible` (`node_2d_base.rs:8`, `node_3d_base.rs:7`).
- Lights: `active` **in addition to** inherited `visible` (`node_2d/lights/point_light_2d.rs:11`, `node_3d/lights/spot_light_3d.rs:27`) — two booleans that both gate the light.
- Cameras: `active` (`node_2d/camera/camera_2d.rs:23`) — here "active" means "the current camera," a genuinely different meaning from the light case.
- Particles: `active` (`node_2d/visual/particle_emitter_2d.rs:13`) meaning "emitting."
- `Sky3D`: both `visible` and `active` inline (`node_3d/visual/sky_3d.rs:45-46`).
- Audio nodes: `enabled` (`node_2d/audio.rs:9,46,87`).
- Buttons: `disabled` (negative-polarity) plus `input_enabled` on the same struct (`node_2d/visual/button_2d.rs:21-22`) — two overlapping switches with opposite polarity.
- `MeshBlendOptions.enabled` (`node_3d/visual/mesh_instance_3d.rs:58`), `Decal3D.active` (`decal_3d.rs:90`).

**Recommendation:** codify a three-word scheme — `visible` (render gate, always from base), `active` (participates in simulation / is the current one), `enabled` (sub-feature toggle inside a settings struct). Under that scheme: audio nodes' `enabled` → `active`; `Decal3D.active` is fine; buttons should keep exactly one of `disabled`/`input_enabled` (suggest `input_enabled`, positive polarity, and derive the visual disabled style from it or rename `disabled` → style-only).

### 1.3 "Sprite" vs "Image"

World-space nodes say Sprite (`Sprite2D`, `AnimatedSprite2D`, `Sprite3D` — `core/perro_nodes/src/nodes/node_registry.rs:1-13`); UI says Image (`UiImage`, `UiAnimatedImage`, `UiImageButton` — `node_registry.rs:17-18`); and 2D world space *also* has `ImageButton2D` (`button_2d.rs:81`), which borrows the UI word into sprite land. `CustomMaterialImage3D` (`render_stack/perro_render_bridge/src/three_d.rs:582`) uses "Image" for what is sourced as a texture. Fields inside all of these are uniformly `texture: TextureID`, and the resource module is `Textures()` — so "texture" is the resource word, "image/sprite" the widget word.

**Recommendation:** accept the Sprite(world)/Image(UI) split as intentional, but document it, and audit the two crossovers: `ImageButton2D` (world-space, Image word) and `CustomMaterialImage3D` (material input, arguably `CustomMaterialTexture3D`).

### 1.4 `drop_` vs `free_` vs `release_`

- `TextureModule::drop` (`api_modules/perro_resource_api/src/sub_apis/texture.rs:126`), `drop_source` (`resource_api/src/sub_apis/audio.rs:435`).
- Scene module has **both** `free_preloaded` (`api_modules/perro_runtime_api/src/sub_apis/scene.rs:299`) and `drop_preloaded` / `drop_preloaded_hashed` (`scene.rs:303,310`) with overlapping intent.
- `release_note` (`runtime_api/src/sub_apis/audio.rs:278`) — fine, it's a held handle, semantically "release," not "dispose."

**Recommendation:** `drop_` is canonical for disposal; deprecate `free_preloaded` (alias to `drop_preloaded`). Keep `release_` only for the note-off meaning.

### 1.5 `position` vs `translation`

Transforms consistently use `position` (`core/perro_structs/src/structs/structs_2d/transform_2d.rs:7`, `structs_3d/transform_3d.rs:6`) and scene files match (`runtime_project/perro_project/src/templates.rs:93,103,159`). The one place both words appear is UI layout, where they are *different things*: `position: UiVector2` (anchored placement) and `translation: Vector2` (post-layout offset) at `core/perro_ui/src/layout.rs:63-65`. This is defensible but undocumented; a doc comment distinguishing them at the definition site would prevent the next reader from assuming drift.

---

## 2. Node Struct Conventions

### 2.1 Typed `Color` vs raw arrays

Visual nodes use `perro_structs::Color` (`sprite_2d.rs:39`, `label_2d.rs:14`), but every light node uses `color: [f32; 3]` (`node_2d/lights/ambient_light_2d.rs:7`, `point_light_2d.rs:7`, `spot_light_3d.rs:21`, etc.), and `Sky3D` uses `Vec<[f32; 3]>` palettes (`sky_3d.rs:47-50`). Texture regions are `Option<[f32; 4]>` (`sprite_2d.rs:27`) rather than a `Rect` type. The Decal3D memory-noted convention (typed `Vector3`/`Color`/`TextureID` fields) is the newer standard — lights predate it.

**Recommendation:** migrate light `color` to `Color` (alpha ignored or used as intensity is a design choice to make explicitly); introduce/use a `Rect` for `texture_region`. Scene parser is unaffected (it parses tuples either way).

### 2.2 Typed IDs vs strings

The ID system (`core/perro_ids/src/ids.rs`, generational `define_generational!` newtypes) is used for textures, meshes, materials, nodes, signals — but several node fields still reference assets by raw string:

- `TileMap2D.tileset: String` (`node_2d/visual/tilemap_2d.rs:8`)
- `ParticleEmitter2D.profile: String` / `ParticleEmitter3D.profile: String` (`particle_emitter_2d.rs:19`, `particle_emitter_3d.rs:27`)
- `SkyShaderPass.path: Cow<'static, str>` (`sky_3d.rs:6`)

Also inconsistent string container: `String` (particles) vs `Cow<'static, str>` (sky, names, animation names). Names/labels elsewhere are consistently `Cow<'static, str>` (`sprite_2d.rs:102,155`, `label_2d.rs:13`).

**Recommendation:** typed IDs for tileset and particle profile (both go through hashed asset lookup at load anyway); `Cow<'static, str>` as the canonical string field type where a string must remain.

### 2.3 Nested settings structs vs flat fields

The Decal3D convention groups related params into typed sub-structs: `surface: DecalSurfaceSettings`, `distance_fade: DecalDistanceFade` (`decal_3d.rs:86-87`). MeshInstance3D follows: `lod: LODOptions`, `blend: MeshBlendOptions` (`mesh_instance_3d.rs:121-122`). Water nodes wrap everything in `water: WaterSurfaceParams` (`water_2d.rs:7`, `water_3d.rs:7`). Cameras use `post_processing: PostProcessSet`, `audio_options: AudioListenerOptions` (`camera_2d.rs:25-26`).

Divergent: `Sky3D` keeps ~9 flat fields including four color palettes (`sky_3d.rs:44-53` — `day_colors`, `evening_colors`, ... could be a `SkyPalette`), and particle emitters keep flat `spawn_rate`/`seed`/`prewarm`/`looping` plus an opaque `params: Vec<f32>` (`particle_emitter_3d.rs:20-33`), which is the least typed parameter block in the node set.

**Recommendation:** adopt the Decal3D nested-settings convention for new nodes (already memory-documented); opportunistically migrate Sky3D palettes and consider a typed particle-param story to replace `Vec<f32>`.

### 2.4 `base` embedding vs inline base fields

Most nodes embed `pub base: Node2D/Node3D` and Deref to it. Exceptions re-declare base fields inline:

- `AmbientLight2D` / `AmbientLight3D`: inline `transform`, `visible`, `render_layers` (`node_2d/lights/ambient_light_2d.rs:5-11`, `node_3d/lights/ambient_light_3d.rs:5-11`) while sibling `PointLight2D` embeds `base` (`point_light_2d.rs:6`).
- `Sky3D`: inline `transform`, `visible` (`sky_3d.rs:44-45`).

This asymmetry leaks into every generic path (registry macros special-case base access via `__node2d_base_expr!` / `__node3d_base_expr!` in `node_registry.rs:87-112`).

**Recommendation:** embed `base` uniformly; if ambient light / sky genuinely don't need modulate/z-order, that's an argument for a slimmer shared base, not for inlining.

### 2.5 Public `internal_*` fields

Particle emitters expose runtime bookkeeping as public fields with an `internal_` prefix: `internal_simulation_time`, `internal_prev_active`, `internal_finished_emitted`, `internal_lifetime_max` (`particle_emitter_2d.rs:21-24`, `particle_emitter_3d.rs:30-33`). Other nodes keep runtime state out of the struct or private (e.g. `AnimatedSprite2D.frame_accum` at `sprite_2d.rs:161` is public but unprefixed — a second, different convention for the same kind of field).

**Recommendation:** one convention — either private fields with accessors, or a single documented `internal_` prefix applied consistently (then `frame_accum` should be `internal_frame_accum`).

### 2.6 Constructors

Mixed `pub fn new()` + `Default` (Button2D `button_2d.rs:32,60`; particles; Sky3D) vs `Default`-only (Label2D `label_2d.rs:34`; Sprite3D `sprite_3d.rs:35`; TileMap2D; Decal3D). Parameterized `new` where a required field exists (`AnimatedSprite::new(name)` `sprite_2d.rs:117`; `SkyShaderPass::new(path)` `sky_3d.rs:11`) is fine. The zero-arg `new() { ... } + Default { Self::new() }` pairs are pure duplication.

**Recommendation:** `Default`-only for zero-arg construction; `new(args)` reserved for required parameters; builder methods (`with_*`, as in `CustomMaterial3D` — `render_bridge/src/three_d.rs:633-659`) for optional configuration.

---

## 3. API Surface (perro_api / perro_runtime_api / perro_resource_api / perro_input_api)

### 3.1 `get_`/`set_` prefix inconsistency

Three regimes coexist:

- **Always `get_`:** `TimeModule` — `get_delta`, `get_elapsed`, `get_fps`, `get_profiling` (`runtime_api/src/sub_apis/time.rs:118-146`); `Window::get_active_refresh_rate` (`window.rs:93`).
- **Never `get_`:** resource facade — `viewport_size` (`resource_api/src/api.rs:243`), `locale_current` (`api.rs:255`, also noun-verb inverted), texture module `load`/`drop`/`is_loaded` (`texture.rs:83-141`).
- **Mixed within one module:** `NodeModule` has `get_node_name`/`set_node_name` (`runtime_api/src/sub_apis/node.rs:1485,1489`) alongside bare `reparent` (`node.rs:1597`), `tag_set` (`node.rs:1628` — verb-*last*, unique in the codebase, sitting next to `set_tags`-style names), and near-duplicates `get_node_children_ids` (`node.rs:1540`) vs `get_children` (`node.rs:1544`). Physics mixes `get_gravity`/`set_gravity` (`physics.rs:448,452`) with bare `contacts_2d` (`physics.rs:658`) and `pause(bool)`/`is_paused` (`physics.rs:736,740` — a setter without `set_`). Audio: `set_debug_rays` paired with bare getter `debug_rays_enabled` (`audio.rs:195,200`).

**Recommendation:** Rust API convention — bare noun getters, `set_` setters, `is_`/`has_` predicates. That makes the resource facade the model. Rename `tag_set` → `set_tags`, `pause(bool)` → `set_paused`, collapse `get_node_children_ids`/`get_children` to one. Because this is public scripting API, do it with deprecated aliases over a release cycle.

### 3.2 Redundant module prefixes

`SignalModule` methods repeat the module name: `signal_connect`, `signal_disconnect`, `signal_emit` (`runtime_api/src/sub_apis/signal.rs:37,78,117`) even though calls read `rt.Signals().signal_emit(...)`. Every other module drops the prefix (`Scene().load`, `Textures().load`). Similarly `get_node_name`/`set_node_name`/`get_node_tags` inside `Nodes()` carry a `node_` infix that `get_children`/`reparent` do not.

**Recommendation:** methods inside a module never repeat the module noun.

### 3.3 Module accessor naming across facades

Facade accessors are PascalCase in both crates (consistent, evidently intentional), but the two sides abbreviate differently: runtime `AnimPlayer()` / `AnimTree()` (`runtime_api/src/api.rs:116,122`) vs resource `Animations()` / `AnimationTrees()` (`resource_api/src/api.rs:136,142`). Also singular/plural drift: `Scene()` vs `Nodes()` vs `Physics()` on the runtime side, `Csv()` vs `SceneDocs()` on the resource side. And the runtime facade struct is named `RuntimeWindow` (`runtime_api/src/api.rs:45`) while one of its modules is `Window()` (`api.rs:68`) — "Window" means both "the entire API surface" and "the OS window."

**Recommendation:** full words (`AnimationPlayer()`, `AnimationTrees()`); plural for collection-managers, singular for singletons; rename the `RuntimeWindow` facade struct — it is the highest-confusion name in the API layer.

### 3.4 Error handling styles

- IO/parse paths: `Result<_, String>` — scene load (`runtime_api/src/sub_apis/scene.rs:263-295`), resource `scene_load_doc` (`resource_api/src/api.rs:168`), csv/gltf/mic modules. Stringly-typed errors, no matching on kind.
- Node/physics operations: `bool` for setters (`set_node_name` `node.rs:1489`, `apply_force_2d` `physics.rs:444`), `Option` for getters — silent failure on stale `NodeID`, consistent within itself.
- Typed error enums exist only at the periphery: `ResPathError` (`resource_api/src/res_path.rs:22`), networking (`perro_networking/src/error.rs:6`), steamworks (`perro_steamworks/src/error.rs:4`), `VariantParseError` (`core/perro_variant/src/variant.rs:544`).

**Recommendation:** keep `Option`/`bool` for hot node ops (documented as "false/None = stale id"), but replace `Result<_, String>` on load paths with small error enums (or one shared `LoadError`) — the string variants already encode categories in prose.

### 3.5 Physics sub-api shape vs other modules

`PhysicsModule` is the only module offering dimension-generic wrappers alongside suffixed pairs: `apply_force<D>` (`physics.rs:492`) delegating to `apply_force_2d`/`_3d`, but `raycast_*`, `move_body_*`, `contacts_*`, `solve_*` come only in `_2d`/`_3d` pairs (`physics.rs:506-731`). `NodeModule` similarly has `_2d`/`_3d` pairs (`get_local_transform_2d` etc., `node.rs:1666-1728`) with no generic form. Also `get_coefficient`/`set_coefficient` (`physics.rs:464-468`) — coefficient *of what* is not in the name (it's the global damping/restitution-style scalar; every other global has a descriptive name like `get_gravity`).

**Recommendation:** either extend the generic-over-dimension pattern (it's the nicer API) across physics and node transforms, or drop the two lonely generic wrappers; rename `get_coefficient` to something self-describing.

---

## 4. Scene Loader / Templates / Compiler

### 4.1 Alias sets are per-call-site, not per-concept

The parser accepts generous aliases, but the alias set for the *same concept* differs by node:

- Flip: sprites accept `"flip_x" | "flip_h" | "mirror_x"` (`scene_loader/prepare/nodes/three_d/base.rs:105`), mesh instances accept only `"flip_x" | "mirror_x"` (`three_d/base.rs:360`) — `flip_h` silently unsupported there.
- Color: most visuals accept `"tint" | "color" | "modulate"` (`two_d/base.rs:15,864`; `three_d/base.rs:20,115,169`), text accepts `"color" | "text_color" | "modulate"` but *not* `tint` (`two_d/base.rs:735`, `three_d/base.rs:136`).
- Region: `"texture_region" | "region" | "atlas_region"` (`three_d/base.rs:92`) — consistent where present, but only where someone remembered.
- Signals: buttons accept 2–3 aliases each (`hover_signals`/`hovered_signals`/`hover_enter_signals`, `two_d/base.rs:796-808`).

Since alias sets are written inline at each match site, drift is structural: every new node re-invents them.

**Recommendation:** central `mod scene_keys` with one alias-set constant per concept (`COLOR_KEYS`, `FLIP_X_KEYS`, `REGION_KEYS`, ...) used by all prepare modules. Cheap, removes an entire drift class, and makes documented scene-file vocabulary derivable from code.

### 4.2 Flat scene keys vs nested Rust structs

`Decal3D` parses `distance_fade_begin` / `distance_fade_length` as flat keys (`three_d/base.rs:194-199`) into the nested `distance_fade: DecalDistanceFade { begin, length }` (`decal_3d.rs:53-54,87`). As nested settings structs become the convention (§2.3), each will need this ad-hoc flattening.

**Recommendation:** decide once — either scene files stay flat (`group_field`) with a helper that maps a prefix onto a settings struct, or the scene format grows sub-blocks. Document in `docs/`.

### 4.3 2D/3D prepare-module asymmetry

`prepare/nodes/three_d/` splits into `base.rs`, `camera.rs`, `lights.rs`, `particles.rs`, `physics.rs`, `animation.rs`; `prepare/nodes/two_d/` has only `base.rs`, `camera.rs`, `physics.rs` — 2D lights, particles, and visual parsing all live inside `two_d/base.rs` (which is why its string-key census in this audit spans buttons, text, sprites, and nine-slices in one file). Same logical structure, different file layout, so a fix applied to `three_d/lights.rs` has its 2D twin hiding in a 1,000+ line `base.rs`.

**Recommendation:** mirror the 3D split in 2D.

### 4.4 Templates

`runtime_project/perro_project/src/templates.rs` scene templates (`templates.rs:87-115`) use the canonical keys (`position`, `color`, `intensity`, `active`) — no drift found; templates are a good reference corpus for whichever aliases get promoted to canonical in docs.

---

## 5. Render Stack

### 5.1 Types in wrong-dimension modules

`render_stack/perro_render_bridge/src/three_d.rs` defines 2D types: `ParticlePath2D` (`three_d.rs:248`), `ParticleSimulationMode2D` (`three_d.rs:280`), `ParticleProfile2D` (`three_d.rs:336`), `PointParticles2DState` (`three_d.rs:406`). Conversely `two_d.rs` hosts `CameraStreamLighting3DState`, `CameraStreamDraw3DState`, `CameraStream3DState` (`two_d.rs:38,47,82`). Both presumably grew where their sibling already lived. Anyone grepping by module for the 2D particle surface misses it.

**Recommendation:** move each to its dimension's file (or a shared `particles.rs` / `camera_stream.rs` if genuinely cross-dimensional — the camera-stream case arguably is, which argues for the shared-file option).

### 5.2 Uniform struct naming

Rust-side GPU uniform structs follow at least five patterns:

- `Camera2DUniform` (`perro_graphics/src/two_d/renderer.rs:37`) — `<Thing><Dim>Uniform`.
- `Scene3DUniform`, `SkyUniform`, `ShadowUniform` (`three_d/gpu.rs:212,249,559`) — mixed dim-tagged and untagged.
- `CameraUniform` (`three_d/particles/gpu.rs:33`) — untagged despite being 3D-particles-specific.
- `UiUniform` in the WGSL source (`ui/gpu/shaders.rs:2`) vs `UiUniformGpu` for the Rust mirror (`ui/gpu.rs:35`) — the only place a `Gpu` suffix distinguishes Rust from WGSL; elsewhere the names match.
- `UniformFrameCtx`, `PostUniform` (`postprocess/mod.rs:40,122`) — prefix-first and generic.

**Recommendation:** `<Thing><Dim?>Uniform`, Rust struct named identically to its WGSL twin (the `UiUniform`/`UiUniformGpu` pair shows the cost of diverging: two names for one byte layout).

### 5.3 2D commands vs 3D retained state

2D submits `*Command` values per frame (`Sprite2DCommand`, `Rect2DCommand`, `DrawShape2DCommand` — `two_d.rs:102-116`) while 3D uploads `*State` snapshots (`Camera3DState`, `Decal3DState`, `Water3DState` — `three_d.rs:6,74,136`). This reflects a real architectural difference (immediate 2D vs retained 3D), not accidental drift — but the suffix convention (`Command` = per-frame, `State` = retained) should be written down, because `Command2D`/`Command3D`/`RenderCommand` (`commands.rs:91,145,255`) also use "Command" for the channel envelope, a third meaning.

---

## 6. Variant System (`core/perro_variant`)

The `as_*` conversion family is large and mostly uniform (`variant.rs:2652-3108`): `as_<type>() -> Option<T>`, `_lossy` suffix for widening (`as_i64_lossy` `variant.rs:70`), `into_*` for by-value, `parse`/`as_type`/`is_type` generic entry points (`variant.rs:544-590`). Deviations:

- `get_kind()` (`variant.rs:2632`) — the only `get_` method on Variant, next to bare `kind_name()` (`variant.rs:2647`). Should be `kind()`.
- Suffix casing drift: `as_vec2` (`variant.rs:2868`) vs `as_transform2d` (`variant.rs:3044`) vs `as_matrix2x2` (`variant.rs:2988`) — bare digit, `2d` compound, and `NxM` all coexist. Node/API land uses `_2d`/`_3d` (e.g. `get_local_transform_2d`), so `as_transform_2d` would match the rest of the engine; alternatively keep the compact forms but consistently.
- `as_vec2` returns `Vector2` — abbreviation in method, full word in type. Same for `as_quat` → `Quaternion` (`variant.rs:3060`). Minor, but the abbreviations aren't applied to `as_matrix2` → `Matrix2` (not abbreviated to `as_mat2`), so the abbreviation rule is per-type.

**Recommendation:** rename `get_kind` → `kind`; pick one suffix scheme (`_2d`) and one abbreviation policy (suggest: method name mirrors type name exactly — `as_vector2`, `as_quaternion` — or accept the current short forms wholesale and stop at documenting them). Migration is trivial with deprecated aliases; Variant methods are heavily used in scripts, so keep aliases long-term.

---

## 7. General Conventions

### 7.1 `*_version` → `*_revision` stragglers

Commit 84fe85b8 renamed node-arena counters to `*_revision` (`nodes.mutation_revision()`, `nodes.physics_revision()`), but consumer-side names still say "version":

- `built_at_version: u64` — `runtime_project/perro_runtime/src/runtime/mesh_query/accel.rs:41`, used at `mesh_query.rs:659-674` (`let current_version = self.nodes.mutation_revision()` — the local binding drifts too).
- `physics_synced_node_version_2d` / `physics_synced_node_version_3d` and local `node_version` — `runtime_project/perro_runtime/src/runtime/physics.rs:139-180`.

**Recommendation:** finish the rename (`built_at_revision`, `physics_synced_node_revision_*`). Trivial, internal-only.

### 7.2 Typed IDs — coverage is good, gaps listed in §2.2

`core/perro_ids` provides generational newtypes via `define_generational!` (`ids.rs:75-79`), and node/API code uses them pervasively (`NodeID`, `TextureID`, `MeshID`, `SignalID`, `PreloadedSceneID`, ...). The remaining raw-string references are the node fields in §2.2 plus post-processing effects addressed `by_name`/`by_index` (`resource_api/src/api.rs:223-229`) — the latter being the only ID-less handle system in the API layer.

### 7.3 Facade naming collision

Covered in §3.3: `RuntimeWindow` as the whole-API struct name (`runtime_api/src/api.rs:45`) vs the `Window()` module and `perro_input_api`'s `window.rs`. Three "window"s, one of which isn't a window.

### 7.4 Module layout parity between parallel crates

- `perro_runtime_api/src/sub_apis/` and `perro_resource_api/src/sub_apis/` mirror each other well (both have `animation.rs`, `animation_tree.rs`, `audio.rs`, `mod.rs`).
- `core/perro_nodes/src/nodes/node_2d/` and `node_3d/` mirror perfectly (`audio.rs`, `camera/`, `core/`, `lights/`, `physics/`, `skeletal/`, `visual/`) — this is the standard the scene-loader prepare modules (§4.3) and render bridge (§5.1) should be held to.

---

## Suggested Priority

1. **Trivial, do now:** 7.1 revision stragglers; 3.3 `tag_set`; 6.1 `get_kind`; 1.4 `free_preloaded`.
2. **Low cost, high leverage:** 4.1 central scene-key alias table; 5.1 move mis-filed bridge types; 4.3 split 2D prepare modules; 5.2 uniform naming.
3. **Needs a decision + deprecation cycle:** 3.1 getter prefix policy; 3.2/3.3 module accessor + method prefix cleanup; 1.1/1.2 node field vocabulary (`modulate`/`visible`/`active`/`enabled`); 3.4 typed load errors.
4. **Structural, schedule with feature work:** 2.1–2.4 node field typing and base embedding; 2.3 nested settings migration; 3.5 dimension-generic physics/node APIs.
