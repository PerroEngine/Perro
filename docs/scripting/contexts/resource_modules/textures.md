# Textures Module

Access:
- `res.Textures()`

Macros:
- `load_texture!(res, source) -> TextureID`
- `reserve_texture!(res, source) -> TextureID`
- `drop_texture!(res, source) -> bool`

Methods:
- `res.Textures().load(source) -> TextureID`
- `res.Textures().reserve(source) -> TextureID`
- `res.Textures().drop(source) -> bool`

What `load` does:
- Returns a stable `TextureID` for `source`.
- If `source` already exists in cache, returns the existing ID immediately.
- If not cached, allocates an ID immediately and queues a renderer create command with `reserved: false`.
- Actual GPU creation is async relative to script call.

What `reserve` does:
- Same as `load`, but marks/creates the texture as reserved (`reserved: true`).
- If texture already exists and is fully created, it queues a "set reserved" command.
- If creation is still pending, reserve is recorded and applied when creation completes.

What `drop` does:
- Removes the source mapping and queues a renderer drop when resource exists.
- If creation is still pending, marks drop-pending so it is dropped right after creation finishes.
- Returns `true` when a pending or existing texture matched `source`.
- Returns `false` when `source` was unknown.

Important behavior:
- Cache key is the exact `source` string.
- Repeated `load` or `reserve` for the same `source` returns the same ID.
- `drop` is source-based, not ID-based.

Example:

```rust
let id = load_texture!(res, "res://textures/smoke.png");
let _same_id = load_texture!(res, "res://textures/smoke.png");
let _ = reserve_texture!(res, "res://textures/smoke.png");
let _ = drop_texture!(res, "res://textures/smoke.png");
```
