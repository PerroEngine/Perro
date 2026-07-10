# DLC Registry ABI v1

## Goal

- list every emitted DLC asset
- use same asset IDs in release + dev
- keep discovery ABI stable across Rust builds
- keep typed Rust data behind exact engine build match

## Shared types

Source: `perro_asset_formats::dlc`

Export symbol:

```text
perro_dlc_pack_registry_api(requested_version: u32) -> *const DlcRegistryApiV1
```

Rules:

- request `1`
- return null for unsupported version
- check `abi_version == 1`
- check `struct_size >= size_of::<DlcRegistryApiV1>()`
- keep pack lib loaded while API, path, or data ptr stays live

## Entry key

Key = `(kind, path_hash)`.

- hash canonical UTF-8 asset URI w/ `perro_ids::string_to_u64`
- allow same URI across kinds
- reject 2 distinct paths w/ same kind + hash
- sort registry by kind ID + path bytes
- keep index order stable for same input

`registry_get` copies metadata into caller storage.

`registry_find` finds metadata by full key.

Path ptr:

- UTF-8
- no NUL req
- pack-owned
- valid until pack unload

## Kind IDs

| ID | Kind |
|---:|---|
| 1 | scene |
| 2 | material |
| 3 | ui_style |
| 4 | tile_set |
| 5 | particle |
| 6 | animation |
| 7 | animation_tree |
| 8 | mesh |
| 9 | collision_trimesh |
| 10 | skeleton |
| 11 | texture |
| 12 | shader |
| 13 | audio |
| 14 | csv |
| 15 | localization |
| 16 | file |
| 17 | navmesh |

Use raw `u32` wrappers.

Never use C ABI enums.

Unknown future IDs stay valid data.

## Access split

`BYTES`:

- call `registry_lookup_bytes(kind, hash, ptr_out, len_out)`
- return pack-owned immutable bytes
- keep ptr valid until pack unload
- use canonical binary form when one exists
- use UTF-8 bytes for shader text

Initial byte kinds:

- tile_set
- mesh
- collision_trimesh
- skeleton
- texture
- shader
- audio
- file
- navmesh

`ENGINE_LOCAL`:

- registry discovery stays stable
- typed data stays outside stable registry ABI
- reject typed lookup unless 32-byte engine ABI fingerprint matches exactly
- never pass Rust `Scene`, `Material3D`, `ParticleProfile3D`, or other Rust obj ptr across unmatched builds

Initial engine-local kinds:

- scene
- material
- ui_style
- particle
- animation
- animation_tree
- csv
- localization

Future canonical encoders may move a kind from `ENGINE_LOCAL` to `BYTES`.

## Engine ABI fingerprint

Use SHA-256 over canonical build facts:

1. Perro engine commit or release source hash
2. Rust compiler verbose version
3. target triple
4. enabled engine features
5. typed lookup schema version

Sort feature names.

Use `\n` between fields.

Use all zeroes only when registry has no `ENGINE_LOCAL` entry.

Fingerprint match permits legacy typed lookup.

Fingerprint match does not make Rust layout part of stable ABI.

## Synthesized assets

Set `SYNTHESIZED` for pipeline-made sub-assets.

Cases:

- GLTF mesh keys
- GLTF material keys
- GLTF skeleton keys
- other importer sub-asset keys

Do not build registry from file ext scan.

Ext scan misses synthesized keys + may claim assets generation drops.

Build inventory from successful pipeline emission results.

## Release flow

1. each static generator ret emitted inventory records
2. merge records after all generators pass
3. sort + hash chk
4. emit pack registry table
5. export v1 API query fn
6. keep old symbols for one compat cycle

## Dev flow

1. use same importer collectors as static pipeline
2. build same `(kind, path, flags, access)` records
3. serve byte kinds from disk/import cache
4. serve engine-local kinds in-process

Dev + release parity test must compare full sorted registry.

## Test gate

Build fixture DLC w/:

- scene
- material
- particle
- animation
- texture
- shader
- GLTF w/ mesh + material + skeleton subkeys

Load pack dylib.

Check:

- version query pass
- bad version -> null
- struct size pass
- all expected keys enumerate
- synthesized flags pass
- `find` pass for every entry
- byte lookup pass for each `BYTES` entry
- typed lookup reject on bad fingerprint
- dev registry = release registry
- null out ptr -> false
- out-of-range index -> false
