# Performance + Release

Perro keeps dev flexible and release loads fast.

## Goal

Know when to profile, bake assets, and reduce runtime cost.

## Dev Mode

Dev mode favors iteration:

- plain files
- simple edits
- script rebuilds
- dynamic asset reads

Use dev mode while changing content.

## Static Export

Release builds bake supported resources:

- textures
- meshes
- materials
- audio
- CSV
- animations
- scenes

Static export reduces parse and lookup cost.

## What Build Creates

`perro build` generates `.perro/project`.

That crate is the release project wrapper.

Native build compiles it to an executable and copies output to `.output/`.

Web build compiles it to wasm, runs wasm bindgen, and writes browser files to `.output/web/`.

The generated wrapper wires:

- project config
- routes
- input map
- static script registry
- static asset lookup fns

## Static Embed

Static embedded assets are generated data.

Supported assets become lookup functions keyed by hash.

The runtime can skip file IO and source parsing for those assets.

Generic files still use the packed asset path.

This is why web dev/build uses static embedded assets: browser runtime should match release asset behavior.

## Native Vs Web

Native dev:

- dynamic files
- generated dev runner
- scripts loaded from generated registry library

Native build:

- release executable
- generated project crate
- static asset paths where supported

Web dev/build:

- generated wasm bundle
- static embedded assets
- static script registry
- local server for dev

## Profile Targets

Profile:

- scene load time
- script hot paths
- query hot paths
- mesh query cost
- physics step cost
- render frame time
- audio propagation
- asset memory

## Hot Path Rules

Cache IDs.

Avoid name lookups in tight loops.

Avoid rebuilding query filters per frame when stable.

Clone only when borrow rules need owned data.

Keep debug draw and logging out of hot release paths.

## Release Checklist

- run `perro check`
- build target platform
- test static asset build
- test first load
- test scene transitions
- test audio start rules
- test input devices
- test save/write paths

## Reference

- [Performance + Flexibility Philosophy](/docs/project/performance_philosophy.md)
- [Feature Matrix](/docs/project/feature_matrix.md)
- [Resource Management](/docs/resources/resource_management.md)
- [Mesh Query Perf Snapshot](/docs/scripting/mesh_query_perf.md)
- [Perro CLI](/docs/tools/perro_cli.md)
- [Generated Script Glue](generated_script_glue.md)
