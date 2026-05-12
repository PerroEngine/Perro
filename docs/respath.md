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
