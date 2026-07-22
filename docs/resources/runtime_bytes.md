# Runtime Bytes Resources

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| When To Use | [When To Use](#when-to-use) |
| Supported Calls | [Supported Calls](#supported-calls) |
| Decode Rules | [Decode Rules](#decode-rules) |
| Examples | [Examples](#examples) |

## Purpose

Runtime byte resource calls create engine resources directly from in-memory bytes.

Use these when data comes from a platform SDK, save data, network payload, mod pack, DLC blob, generated asset, or any source that is not a `res://` file.

File path loading still uses the normal `*_load!` and `*_reserve!` macros.
Byte loading skips path IO and feeds bytes into the same runtime decode/cache paths.

## When To Use

- Steam avatar RGBA bytes -> `TextureID`
- downloaded PNG/JPEG/WebP/PTEX bytes -> `TextureID`
- generated `Mesh3D` bytes in `PMESH` or glTF/GLB form -> `MeshID`
- in-memory `.pmat` or glTF/GLB material bytes -> `MaterialID`
- in-memory `.panim` bytes -> `AnimationID`
- in-memory `.panimtree` bytes -> `AnimationTreeID`
- in-memory audio bytes -> runtime audio source string
- in-memory `.sf2` bytes -> `SoundFontID`
- in-memory CSV bytes -> `&'static Csv`
- in-memory `.pskel2d` / `.pskel3d` bytes -> bone arrays

## Supported Calls

| Data | Method | Macro | Return |
| --- | --- | --- | --- |
| texture image bytes | `ctx.res.Textures().create_from_bytes(bytes)` | `texture_create_from_bytes!(ctx.res, bytes)` | `TextureID` |
| raw RGBA texture | `ctx.res.Textures().create_from_rgba(w, h, rgba)` | `texture_create_from_rgba!(ctx.res, w, h, rgba)` | `TextureID` |
| replace mutable RGBA texture | `ctx.res.Textures().write_rgba(id, w, h, rgba)` | `texture_write_rgba!(ctx.res, id, w, h, rgba)` | `bool` |
| update mutable RGBA region | `ctx.res.Textures().write_rgba_region(id, x, y, w, h, rgba)` | `texture_write_rgba_region!(ctx.res, id, x, y, w, h, rgba)` | `bool` |
| mesh bytes | `ctx.res.Meshes().create_from_bytes(bytes)` | `mesh_create_from_bytes!(ctx.res, bytes)` | `MeshID` |
| material bytes | `ctx.res.Materials().create_from_bytes(bytes)` | `material_create_from_bytes!(ctx.res, bytes)` | `MaterialID` |
| animation bytes | `ctx.res.Animations().create_from_bytes(bytes)` | `animation_create_from_bytes!(ctx.res, bytes)` | `AnimationID` |
| animation tree bytes | `ctx.res.AnimationTrees().create_from_bytes(bytes)` | `animation_tree_create_from_bytes!(ctx.res, bytes)` | `AnimationTreeID` |
| audio bytes | `ctx.res.Audio().create_source_from_bytes(bytes)` | `audio_create_from_bytes!(ctx.res, bytes)` | `Option<String>` |
| soundfont bytes | `ctx.res.Audio().midi().load_soundfont_from_bytes(bytes)` | `midi_load_soundfont_from_bytes!(ctx.res, bytes)` | `SoundFontID` |
| CSV bytes | `ctx.res.Csv().load_bytes(bytes)` | `csv_load_bytes!(ctx.res, bytes)` | `&'static Csv` |
| 2D skeleton bytes | `ctx.res.Skeletons().load_bones_2d_from_bytes(bytes)` | `skeleton_load_bones_2d_from_bytes!(ctx.res, bytes)` | `Vec<Bone2D>` |
| 3D skeleton bytes | `ctx.res.Skeletons().load_bones_3d_from_bytes(bytes)` | `skeleton_load_bones_3d_from_bytes!(ctx.res, bytes)` | `Vec<Bone3D>` |

## Decode Rules

Texture bytes accept regular image formats supported by Perro image decode plus `PTEX`.
Use `create_from_rgba` when bytes are already uncompressed RGBA8.
`rgba.len()` must equal `width * height * 4`; invalid sizes return `TextureID::nil()`.

Mutable writes require a known runtime texture. Region bytes use tight RGBA8 rows and must fit inside the current texture. Successful writes invalidate 2D, UI, and 3D material texture caches.

Mesh bytes accept `PMESH` or glTF/GLB mesh index `0`.
Invalid bytes return a `MeshID` request that fails in the render backend; `mesh_is_loaded!` stays false.

Material bytes accept `.pmat` text or glTF/GLB material index `0`.
Invalid bytes return `MaterialID::nil()`.

Animation bytes accept `.panim` text.
Animation tree bytes accept `.panimtree` text.
Invalid bytes return nil IDs.

Audio bytes are cached under a runtime source string.
Use the returned string with normal audio playback calls.
Invalid or unsupported bytes may fail later at audio decode/play time.

Soundfont bytes accept `.sf2`.
Invalid bytes return a `SoundFontID` request that does not become loaded.

CSV bytes parse immediately.
Invalid CSV returns `EMPTY_CSV`.

Skeleton bytes accept packed skeleton bytes.
Use the 2D call for `.pskel2d` and the 3D call for `.pskel3d`.
Invalid bytes return an empty bone array.

## Choice + Lifetime

Use bytes APIs for downloaded, generated, save-backed, platform, or mod data.
Use path loads for authored project/DLC assets because paths preserve cache keys,
build-time baking, and readable scene wiring. Byte APIs that return resource IDs
follow the same readiness and lifetime rules as path-loaded resources. CSV and
skeleton calls return decoded CPU data immediately; audio returns an optional
source name; texture writes return success/failure.

Validate size/format at the system boundary. A decode failure must keep the old
or fallback gameplay state instead of leaving a half-applied resource choice.

## Examples

Create a texture from Steam avatar RGBA bytes:

```rust
let texture = texture_create_from_rgba!(ctx.res, avatar.width, avatar.height, &avatar.rgba);
```

Create a texture from image bytes:

```rust
let texture = texture_create_from_bytes!(ctx.res, png_bytes.as_slice());
```

Create audio source from bytes, then play it:

```rust
if let Some(source) = audio_create_from_bytes!(ctx.res, wav_bytes.as_slice()) {
    audio_play!(ctx.res, Audio::new(source.as_str()));
}
```

Create a material from `.pmat` bytes:

```rust
let mat = material_create_from_bytes!(ctx.res, pmat_bytes.as_slice());
```

Load CSV bytes:

```rust
let table = csv_load_bytes!(ctx.res, csv_bytes.as_slice());
```
