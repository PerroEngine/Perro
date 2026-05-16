# Performance + Flexibility Philosophy

Perro tries to keep one trade honest:
performance + simplicity without sacrificing either.

That shows up as three practical goals:

- simple to learn
- flexible to use
- fast in release

That trade depends on Perro owning compiler-pipeline work.
You keep normal project files under `res/`.
You edit scenes, textures, meshes, materials, scripts, UI resources, and other assets directly.
There is no proprietary import database to manage and no manual reimport step as the core workflow.

The engine owns the parse, bake, and load details.
That means you keep authoring simple, then let export move supported assets into the faster static path.
Goal ! trade flexibility away for performance.
Goal = move that cost into Perro compiler/build pipeline so runtime stays lean.

This applies across engine, including 2D + UI.
Perro aims to support both 2D + 3D performantly, w/ high fps as main target.
UI fits same model too: strong layout tools such as relative sizing + clamping in authoring, heavier prep in build, faster runtime loading in release.

## Dev Path

`perro dev` loads project assets dynamically from disk.
Scenes and supported resource files are parsed or baked when the runtime needs them.

That cost is acceptable for development.
Perro still keeps this path fast enough that microseconds and small millisecond costs are normal dev tradeoffs.
If a dynamic load causes a delay, that delay is mostly a developer-only cost.
The player build does not need to pay the same text IO and parse work.

Use `perro dev` for testing and dynamic loading through the dev runner.
Use `perro build` to build the final executable into the project `.output/` folder.

## Static Export Path

`perro build` bakes supported asset types ahead of time.
The static pipeline cost happens during export/build.
It is not runtime load cost.

Static export currently targets core runtime assets such as:

- scenes
- textures
- meshes
- collision trimeshes
- materials
- UI styles
- tilesets
- particles
- animations
- animation trees
- skeletons
- audio
- CSV tables
- shaders
- localization

Scenes become static Rust data.
Their node data, keys, parent/child relationships, and authored refs are already parsed into a runtime-ready shape.
Runtime loading still prepares and merges nodes into the live runtime and attaches scripts, but it avoids scene text IO and scene parsing first.

Baked assets are exposed through generated lookup functions.
Those functions use a `u64` path hash and a Rust `match`, for example `lookup_scene(path_hash)`.
For supported static assets, runtime can ask the correct lookup directly instead of searching inside an archive.

Exported builds also include `assets.perro`.
It is a binary pack for generic `res/` files that do not have static bake support.
Static bake handles the asset types Perro knows how to pre-shape; the generic pack keeps the rest available.
Export also tries to keep shipped bytes small.
Static binary payloads such as `PTEX`, `PMESH`, `PSKEL`, and `PAWDIO` use compressed payloads when compression makes them smaller.
Generic `assets.perro` entries also use compressed entry data when it wins.
Already-prepared Rust data, such as baked scene/material/style tables, is compiled into the executable instead of being stored as source files to parse.

There are two separate costs:

1. Get the data.
2. Turn the data into runtime objects.

In dev, getting data usually means file IO.
Then Perro may parse source formats, build binary/runtime resource data, or prepare scene data.

In static export, getting supported baked data is usually a `u64` hash `match`.
In this local snapshot, static hash lookup is ~6.55 ns to ~8.36 ns.
After that, Perro either uses already-prepared Rust data, such as static scene/material/style data, or decodes compact baked bytes such as `PTEX`, `PMESH`, `PTSET`, `PSKEL`, or `PAWDIO`.

This is the main release win.
The runtime skips the slowest parts of the dev path: file IO, source text parse, and repeated source-to-runtime shaping for assets the static pipeline understands.
In this snapshot, loading a 512-node scene through dynamic fs read + parse + prepare is ~1.58 ms.
Preparing the same already-baked static `Scene` is ~344 us, saving ~1.23 ms and 3292 allocations per load.
For a tiny 1-node scene, static prepare is ~1.2 us.

## Dev vs Release Load Path

| Step             | Dev dynamic path                              | Release static path                                                                           |
| ---------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Find asset       | Resolve path under project `res/`             | Hash path to `u64`                                                                            |
| Get data         | File IO from disk                             | Generated `match` lookup: ~6.55 ns to ~8.36 ns in snapshot                                    |
| Scene format     | `.scn` text                                   | Already-baked Rust `Scene` data                                                               |
| Scene work       | Read text, parse, prepare: 512 nodes ~1.58 ms | Prepare baked `Scene`: 512 nodes ~344 us                                                      |
| Resource format  | Source file or dynamic runtime format         | Baked Rust data or compact binary bytes                                                       |
| Resource work    | Read file, parse/decode, shape runtime data   | Static load into `Vec`: 4 KiB texture ~0.664 us, 64 KiB mesh ~2.08 us, 16 KiB audio ~0.948 us |
| Generic fallback | Read from `res/`                              | Read compressed entries from embedded `assets.perro` when smaller                             |
| Main goal        | Flexibility + edit-run speed                  | Near-imperceptible runtime loading                                                            |

Examples:

| Asset kind        | Dev                                | Release                                |
| ----------------- | ---------------------------------- | -------------------------------------- |
| Scene             | load `.scn` file + parse           | `lookup_scene(hash)` -> baked `Scene`  |
| Material/UI style | parse authored text/object data    | lookup static Rust material/style data |
| Texture           | file IO + image/source decode path | lookup baked `PTEX` bytes              |
| Mesh              | file IO + source mesh path         | lookup baked `PMESH` bytes             |
| Tileset           | parse `.ptileset` text             | lookup baked `PTSET` bytes             |
| Audio             | file IO + source load path         | lookup baked `PAWDIO` bytes            |
| CSV table         | file IO + CSV parse/cache          | `lookup_csv(hash)` -> static table     |

CSV tables also support Rust-side `CSVQuery`.
Current query path filters table rows, optionally sorts matching rows, applies limit, and returns projected row views.
Equality and `in` filters seed from lazy per-column hash indexes, so repeated queries avoid full table scans when a matching filter exists.

## Perf Target

Perro treats load-time CPU work as something worth tightening.
Dynamic dev loading in the microsecond to small millisecond range is fine because it keeps workflow flexible.
Static release loading aims for the nanosecond and microsecond timeline where practical.

The goal is simple:

- dev stays flexible
- export does the heavy lifting
- release runtime loads pre-shaped data
- player-visible load cost becomes near-imperceptible where the engine has a static path

## Bench Snapshot

Local snapshot from this repo.
Numbers depend on machine, build, and benchmark settings.
They measure runtime work, not static pipeline generation time.

Commands:

```powershell
cargo bench -p perro_runtime --bench scene_loading --features bench -- --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench -p perro_io --bench static_asset_load -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Scene load comparison:

| Nodes | Dev: file read + parse + prepare | Release: baked `Scene` prepare | Saved/load | Faster | Alloc saved/load |
| ----- | -------------------------------- | ------------------------------ | ---------- | ------ | ---------------- |
| 1     | ~57.6 us                         | ~1.2 us                        | ~56.4 us   | ~47.9x | 204 allocs       |
| 16    | ~101 us                          | ~14.2 us                       | ~87 us     | ~7.1x  | 123 allocs       |
| 32    | ~150 us                          | ~29.2 us                       | ~121 us    | ~5.1x  | 232 allocs       |
| 64    | ~251 us                          | ~57.8 us                       | ~193 us    | ~4.3x  | 437 allocs       |
| 512   | ~1.58 ms                         | ~344 us                        | ~1.23 ms   | ~4.6x  | 3292 allocs      |
| 2048  | ~5.77 ms                         | ~1.34 ms                       | ~4.43 ms   | ~4.3x  | 13030 allocs     |

Scene cost split:

| Nodes | Dev file IO only, approx | Dev parse + prepare, no file IO | Release prepare from baked `Scene` |
| ----- | ------------------------ | ------------------------------- | ---------------------------------- |
| 1     | ~53.6 us                 | ~4.0 us                         | ~1.2 us                            |
| 16    | ~50.3 us                 | ~51.1 us                        | ~14.2 us                           |
| 32    | ~48.9 us                 | ~102 us                         | ~29.2 us                           |
| 64    | ~53.8 us                 | ~197 us                         | ~57.8 us                           |
| 512   | ~184 us                  | ~1.39 ms                        | ~344 us                            |
| 2048  | ~172 us                  | ~5.60 ms                        | ~1.34 ms                           |

Static asset access:

| Operation                    | Texture            | Mesh               | Audio               |
| ---------------------------- | ------------------ | ------------------ | ------------------- |
| Hash `match` lookup          | ~8.27 ns           | ~8.36 ns           | ~6.55 ns            |
| Load static bytes into `Vec` | 4 KiB in ~0.664 us | 64 KiB in ~2.08 us | 16 KiB in ~0.948 us |

CSV query access:

| Operation                        | 250k rows, 8 cols |
| -------------------------------- | ----------------- |
| Primary string find batch        | ~5.6 us           |
| Primary hash find batch          | ~3.4 us           |
| Header get batch                 | ~2.8 us           |
| Filter/sort/limit query, indexed | ~1.3 ms           |
| Previous scan query before index | ~6.3 ms           |
| First unoptimized query baseline | ~15.3 ms          |

Compression in export:

| Data                       | Release form               | Compression behavior                             |
| -------------------------- | -------------------------- | ------------------------------------------------ |
| Generic files              | `assets.perro` binary pack | compress entry when smaller                      |
| Textures                   | `PTEX`                     | compress payload when smaller                    |
| Meshes/collision trimeshes | `PMESH`                    | compress payload when smaller                    |
| Skeletons                  | `PSKEL`                    | compress payload when smaller                    |
| Audio                      | `PAWDIO`                   | compress payload when smaller                    |
| Scenes/materials/UI styles | Rust static data           | pre-parsed into executable data, not source text |

The scene numbers show runtime loading from each path.
The static scene row means the scene already exists as baked `Scene` data.
It does not include static pipeline bake time, because that time is paid during export.
The file IO-only values are approximated by subtracting in-memory parse+prepare time from fs read+parse+prepare time in the same benchmark.
The asset numbers show static lookup/load timing only.
They are the release-side target path for baked resources such as `PTEX`, `PMESH`, and `PAWDIO`; dynamic file IO and source parsing are avoided before those bytes reach runtime decoders.
