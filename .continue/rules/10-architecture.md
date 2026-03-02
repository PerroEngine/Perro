# Architecture: Crates and Responsibilities

All crates are inside `/perro_source`.

## Core layer (`/perro_source/core`)

- `perro_ids`: stable ID types, generation/index patterns, ID utilities
- `perro_variant`: lightweight variant/value representation
- `perro_structs`: shared structs, math types, POD-ish data
- `perro_nodes`: node definitions, node traits, node metadata
- `perro_particle_math`: particle math utilities
- `perro_terrain`: terrain data structures + logic (core-side)

Rules:

- Core crates must not depend on runtime/render/devtools/build_pipeline/io stacks.
- Core crates should avoid global state, IO, threads, GPU APIs, and platform-specific code.

## Runtime layer (`/perro_source/runtime_project`)

- `perro_runtime`: runtime orchestration
- `perro_internal_updates`: internal frame/update orchestration
- `perro_scene`: scene representation, load/merge/apply behaviors
- `perro_project`: project-level glue/config

Rules:

- Runtime may depend on core and api_modules.
- Runtime should not directly depend on render_stack except via bridges/modules designed for that boundary.

## API modules (`/perro_source/api_modules`)

- `perro_runtime_context`: runtime-facing API surface exposed to scripts/game code
- `perro_resource_context`: resource loading/application APIs
- `perro_modules`: module registry / module wiring
- `perro_input`: input API surface

Rules:

- API modules are the primary public surface area for scripting/runtime access.
- Prefer thin wrappers and clear ownership over "reach into everything".

## Render stack (`/perro_source/render_stack`)

- `perro_app`: app bootstrap
- `perro_render_bridge`: boundary layer for runtime <-> renderer
- `perro_graphics`: GPU abstractions, pipelines, WGPU/WGSL integration
- `perro_meshlets`: meshlet/cluster rendering concerns

Rules:

- Render stack must not depend on devtools or build_pipeline crates.
- Render stack should not depend on scripting stack directly; connect via runtime_context/bridge.

## Script stack (`/perro_source/script_stack`)

- `perro_scripting`: scripting runtime, script lifecycle entry points
- `perro_scripting_macros`: proc-macros for scripting ergonomics

Rules:

- `perro_scripting_macros` must not depend on engine runtime crates (proc-macro hygiene).
- Scripting should rely on `perro_runtime_context` for engine access, not deep internal crate calls.

## Build pipeline (`/perro_source/build_pipeline`)

- `perro_compiler`: compilation/build logic (not runtime)
- `perro_static_pipeline`: static build/pipeline tasks

Rules:

- Build pipeline must not be required for shipping runtime paths.
- Avoid runtime-only dependencies in build tools.

## IO stack (`/perro_source/io_stack`)

- `perro_io`: IO utilities
- `perro_brk`: format/serialization specifics (if applicable)

Rules:

- IO stack is the only place that should touch filesystem/network directly unless explicitly justified.

## Devtools (`/perro_source/devtools`)

- `perro_cli`: CLI entry points
- `perro_dev_runner`: dev workflows

Rules:

- Devtools can depend on anything, but nothing should depend on devtools.
