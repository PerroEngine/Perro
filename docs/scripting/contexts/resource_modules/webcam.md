# Webcam Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| Device Selection | [Device Selection](#device-selection) |
| Examples | [Examples](#examples) |
| Webcam Node | [Webcam Node](#webcam-node) |
| API Reference | [API Reference](#api-reference) |
| Macros | [Macros](#macros) |

## Overview

Use `ctx.res.Webcams()` for live webcam capture.

The module owns native capture and hides the backend crate.
Scripts should not use `nokhwa` directly.

Webcam capture produces:

- a live `TextureID` for rendering
- optional CPU RGBA frames when `cpu_frames` is enabled
- an error string when the OS/backend rejects a device or stream

Capture is available on native desktop builds.
Wasm and Android builds return backend-unavailable errors.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Webcams()`
- Native backend: `nokhwa`
- Native platforms: Windows, Linux, macOS
- Render output: `TextureID`
- CPU readback: opt-in with `WebcamConfig.cpu_frames`

## Device Selection

`WebcamConfig.device` is a device slot string.

Slot rules:

- empty string: default camera, index `0`
- numeric string like `"1"`: camera index `1`
- non-numeric string: backend device ID/name

Use `ctx.res.Webcams().devices()` to list connected cameras.
Each `WebcamDevice.slot` is already shaped for `WebcamConfig.device`.
Use `WebcamDevice::config()` or `open_device` to avoid manual slot mapping.

`WebcamDevice` fields:

| Field | Type | Detail |
| --- | --- | --- |
| `slot` | `String` | Pass to `WebcamConfig.device`. |
| `index` | `Option<u32>` | Numeric index when the backend exposes one. |
| `name` | `String` | Human device name. |
| `description` | `String` | Backend description. |
| `extra` | `String` | Backend-specific stable ID or extra metadata. |

## Examples

Open default camera:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let webcam = webcam_default!(ctx.res).ok();
        if let Some(webcam) = webcam {
            let texture = webcam_texture!(ctx.res, webcam);
            let _ = texture;
        }
    }
});
```

List devices and open the first one:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Ok(devices) = webcam_devices!(ctx.res) {
            if let Some(device) = devices.first() {
                let webcam = webcam_open_device!(ctx.res, device).ok();
                let _ = webcam;
            }
        }
    }
});
```

Open with settings:

```rust
let config = WebcamConfig {
    device: "1".to_string().into(),
    width: 1280,
    height: 720,
    fps: 30,
    mirror: true,
    cpu_frames: true,
};

let webcam = webcam_open!(ctx.res, config)?;
let texture = webcam_texture!(ctx.res, webcam);
let frame = webcam_frame_rgba!(ctx.res, webcam);
let _ = (texture, frame);
```

## Webcam Node

`Webcam` is a resource node.
It does not draw by itself.
Use it as the source for `CameraStream2D`, `CameraStream3D`, or `UiCameraStream`.

When a stream references an enabled visible `Webcam` node, the runtime opens capture automatically.
When the webcam node is disabled, hidden, or no longer used by the stream, the runtime closes the capture slot.

Scene example:

```text
[PlayerCam]
    [Webcam]
        slot = ""
        resolution = (640, 480)
        fps = 30
        mirror = true
        cpu_frames = false
        enabled = true
    [/Webcam]
[/PlayerCam]

[PlayerCamView]
parent = $root
    [UiCameraStream]
        camera = @PlayerCam
        aspect_mode = "fit"
        [UiNode]
            anchor = "center"
            size_ratio = (0.35, 0.35)
        [/UiNode]
    [/UiCameraStream]
[/PlayerCamView]
```

`Webcam` fields:

| Field | Type | Detail |
| --- | --- | --- |
| `slot` / `device` / `device_id` / `name` / `source` / `src` | `String` | Device slot. Empty uses index `0`. |
| `resolution` | `Vec2` | Sets width and height together. |
| `width` | `u32` | Requested capture width. |
| `height` | `u32` | Requested capture height. |
| `fps` / `frame_rate` | `u32` | Requested capture FPS. |
| `mirror` / `flip_x` | `bool` | Mirror frames horizontally before upload. |
| `cpu_frames` / `readback` | `bool` | Keep latest RGBA frame for script reads. |
| `enabled` / `active` | `bool` | Enable automatic capture for streams. |

## API Reference

### `devices`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn devices(&self) -> Result<Vec<WebcamDevice>, String>` |
| Returns | `Result<Vec<WebcamDevice>, String>` |
| Use when | List connected cameras without using `nokhwa` directly. |

### `open`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn open(&self, config: WebcamConfig) -> Result<WebcamID, String>` |
| Returns | `Result<WebcamID, String>` |
| Use when | Start capture with explicit device, size, FPS, mirror, and CPU-frame settings. |

### `open_device`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn open_device(&self, device: &WebcamDevice) -> Result<WebcamID, String>` |
| Returns | `Result<WebcamID, String>` |
| Use when | Open a device returned by `devices()` with default capture settings. |

### `default`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn default(&self) -> Result<WebcamID, String>` |
| Returns | `Result<WebcamID, String>` |
| Use when | Open or reuse the default camera. |

### `texture`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn texture(&self, id: WebcamID) -> TextureID` |
| Returns | `TextureID` |
| Use when | Bind the live camera output to render/UI code. |

### `frame_rgba`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn frame_rgba(&self, id: WebcamID) -> Option<WebcamFrame>` |
| Returns | `Option<WebcamFrame>` |
| Use when | Read the latest CPU RGBA frame after opening with `cpu_frames = true`. |

### `is_open`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn is_open(&self, id: WebcamID) -> bool` |
| Returns | `bool` |
| Use when | Check capture lifetime. |

### `last_error`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn last_error(&self, id: WebcamID) -> Option<String>` |
| Returns | `Option<String>` |
| Use when | Inspect OS/backend capture errors. |

### `close`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Webcams()` |
| Signature | `pub fn close(&self, id: WebcamID) -> bool` |
| Returns | `bool` |
| Use when | Stop manual capture and drop the live texture. |

## Macros

| Macro | Expands to |
| --- | --- |
| `webcam_devices!(ctx.res)` | `ctx.res.Webcams().devices()` |
| `webcam_open!(ctx.res, config)` | `ctx.res.Webcams().open(config)` |
| `webcam_open_device!(ctx.res, device)` | `ctx.res.Webcams().open_device(device)` |
| `webcam_default!(ctx.res)` | `ctx.res.Webcams().default()` |
| `webcam_texture!(ctx.res, id)` | `ctx.res.Webcams().texture(id)` |
| `webcam_frame_rgba!(ctx.res, id)` | `ctx.res.Webcams().frame_rgba(id)` |
