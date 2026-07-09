# Static scene script index

Status: blocked design.

## Goal

Static scene node -> native ctor index.

Skip hash lookup for base release scripts.

Keep dev + DLC hash paths.

## Why no small patch

`SceneNodeEntry` only store script path text.

Prepare step always hash text -> `PendingScript`.

Static scene gen lack registry manifest.

Registry list filter use transpiled ctor export result.

Raw `.rs` scan != registry list.

Static gen run as separate pipeline task.

## DLC blocker

Base registry index + DLC registry index use diff spaces.

DLC scene may ref base script or DLC script.

Plain `u32` index lack source owner.

Dynamic reload also lack stable index guarantee.

## Safe shape

Add compiler-owned manifest:

```text
ScriptRegistryManifest
-> source = Base | Dlc(mount)
-> sorted (path_hash, index)
```

Pass manifest 2 static scene gen.

Add generated-only scene hint:

```text
StaticScriptRef { source, index }
```

Keep `script` path text + hash as fallback.

Parser/dyn scene -> `None` hint.

`PendingScriptAttach` carry opt hint.

Static runtime -> base native slice[index].

DLC/dev -> hash overlay lookup.

Reject source/index OOB -> hash fallback or load err.

## Build order

Base:

`sync_scripts` -> manifest -> static scene gen -> project build.

DLC:

`sync_dlc_scripts` -> DLC manifest -> DLC static scene gen -> pack build.

Current DLC order fit this.

## Required tests

- static base scene -> index equals generated registry order
- no-ctor `.rs` -> no hint
- dynamic disk scene -> no hint + hash resolve
- DLC scene -> DLC source/index only
- DLC scene ref base script -> base source/index or hash fallback
- stale hint -> no wrong ctor
- source/index mismatch -> reject

## Scope

Touch scene public data model, parser fixtures, static pipeline API, compiler script sync, runtime attach path, DLC pack gen.

Land only aft registry facade own base slice + dynamic overlay.

Do ! encode global index b4 source-tag design.
