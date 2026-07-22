# ResPath

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`ResPath` is Perro's validated virtual resource path type. Game assets are addressed by scheme (`res://` for bundled content, `dlc://` for mounted packs, `user://` for save data) instead of raw OS paths, so the same code loads correctly on desktop and web. Any resource call or scene field that names an asset expects a `ResPath`, and the type rejects malformed paths (bad scheme, backslashes, `.`/`..` segments) at compile time for literals or as a `Result` for dynamic strings.

## Use Cases

- Load a bundled asset: `res_path!("res://textures/player.png")` passed to `texture_load!` or `mesh_load!`.
- Reference a DLC or mod-pack asset by mount name: `ResPathBuf::try_new(format!("dlc://{pack}/textures/player.png"))`.
- Read and write save data: `user://save.dat` paths, which map to `localStorage` on web builds.
- Store a swappable asset path in state: `#[default = res_path!(...)] texture_path: &'static ResPath`, or `ResPathBuf` when the path changes at runtime.
- Catch typos early: `res_path!` / `res_path_buf!` fail the build on an invalid literal; use `try_new` when a path is computed at runtime.
- Persist a path through `Variant`: `ResPathBuf` and `&'static ResPath` implement `DeriveVariant`, and `parse::<ResPathBuf>()` revalidates on read.

## Path Choice

Use `res://` for read-only shipped project content, `dlc://` for mounted packs,
and `user://` for writable player data. Prefer compile-checked path macros for
literals and `try_new` for runtime-built paths. A raw string is suitable at a
scene boundary where the parser resolves it into a typed asset ID; runtime
`set_var!` remains strict.

## Example

Store a validated path in state, then load the texture it points at:

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = res_path_buf!("res://textures/player.png")]
    texture_path: ResPathBuf,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let texture_path = with_state!(ctx.run, PlayerState, ctx.id, |state| {
            state.texture_path.clone()
        });
        let texture = texture_load!(ctx.res, &texture_path);
        let _ = texture;
    }
});
```

Build an owned path for a mounted DLC pack chosen at runtime:

```rust
use perro_api::prelude::*;

let path = ResPathBuf::try_new(format!("dlc://{pack}/textures/player.png"))?;
let texture = texture_load!(ctx.res, &path);
```

## Reference

# ResPath

`ResPath` is Perro virtual resource path type.

Use it for paths with these schemes:

- `res://path/to/file.ext`
- `dlc://NAME/path/to/file.ext`
- `dlc://self/path/to/file.ext`
- `user://path/to/file.ext`

Do not use `std::path::Path` for these paths.

`Path` is for OS files.

`ResPath` is for engine resources.

Web target note:

- `user://...` still valid on web
- browser build maps `user://...` -> `localStorage`
- session/cookie vals use `perro_web::storage`, ! `ResPath`

## API

Borrowed path:

```rust
use perro_api::prelude::*;

let path = res_path!("res://textures/player.png");
let texture = texture_load!(ctx.res, path);
```

Owned path:

```rust
use perro_api::prelude::*;

let path = ResPathBuf::try_new(format!("dlc://{pack}/textures/player.png"))?;
let texture = texture_load!(ctx.res, &path);
```

Promote borrowed to owned:

```rust
use perro_api::prelude::*;

let path = res_path!("res://textures/player.png");
let owned = path.to_buf();
```

`to_res_path_buf()` is also available.

## Rules

- Must start with `res://`, `dlc://`, or `user://`.
- `dlc://` must include mount name: `dlc://NAME/...`.
- `dlc://self/...` means current DLC mount while loading DLC-authored content.
- Use `/`, not `\`.
- `.` and `..` segments are rejected.

Resource APIs use `ResPathSource`.

That accepts `ResPath`, `ResPathBuf`, `&str`, `String`, and `Cow<'static, str>`.

Use `ResPath` when code wants validated resource path data.

## Variant

`ResPathBuf` implements `DeriveVariant`.

`&'static ResPath` also implements `DeriveVariant`.

Variant storage is still a string.

Parsing validates the path:

```rust
use perro_api::prelude::*;

let value = ResPathBuf::new("res://textures/player.png").into_variant();
let path = value.parse::<ResPathBuf>()?;
```

State can store resource paths:

```rust
use perro_api::prelude::*;

#[State]
pub struct PlayerState {
    #[default = res_path!("res://textures/player.png")]
    texture_path: &'static ResPath,

    #[default = res_path_buf!("res://meshes/player.glb")]
    mesh_path: ResPathBuf,
}
```

Use `ResPathBuf` when path changes at runtime.

`ResPath::new` and `ResPathBuf::new` require `&'static str`.

`res_path!` and `res_path_buf!` require string literals and fail at compile time for bad paths.

Use `ResPath::try_new` or `ResPathBuf::try_new` when you want to handle dynamic paths as `Result`.
