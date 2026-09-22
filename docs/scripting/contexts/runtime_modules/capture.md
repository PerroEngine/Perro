# Capture Runtime Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Context + imports | [Context + imports](#context-imports) |
| Capture config | [Capture config](#capture-config) |
| Script example | [Script example](#script-example) |
| Sources | [Sources](#sources) |
| Size + framing | [Size + framing](#size-framing) |
| Timing | [Timing](#timing) |
| Lifecycle + state | [Lifecycle + state](#lifecycle-state) |
| Replay actions | [Replay actions](#replay-actions) |
| Output + errors | [Output + errors](#output-errors) |
| Host helpers | [Host helpers](#host-helpers) |
| API reference | [API reference](#api-reference) |

## Purpose

`ctx.run.Capture()` starts and observes a bounded capture session from script or
QA code. The app owns GPU readback, RGBA preparation, encoding, safe stop, and
final output commit. Script code supplies capture policy and asks the app to
stop at a frame boundary.

Use this module for director actions, QA clips, offline replay, and scripted
capture. Use [`perro capture`](../../../tools/perro_cli.md) for a CLI run that
builds and launches the graphics runner for you.

## Context + imports

The module lives on `ctx.run`. Public capture types come from the `perro_api`
prelude; `Duration` comes from the Rust standard library:

```rust
use perro_api::prelude::*;
use std::time::Duration;
```

`ScriptContext` borrows `ctx.run` for one callback. Create the module, call it,
and let its borrow end before another runtime operation.

## Capture config

Build a `CaptureConfig`, set `output`, then call `ctx.run.Capture().start(config)`.
The normal API resolves project source dimensions and creates staging under the
output's parent `.perro-capture` directory.

| Field | Values | Default / rule |
| --- | --- | --- |
| `source` | `MainWindow`, `Camera2D`, `Camera3D`, `UISubView`, `RenderTarget` | `MainWindow` |
| `width`, `height` | `Option<u32>` | Omit both to keep source size; set one to derive the other from `aspect_ratio` |
| `aspect_ratio` | `AspectRatio::Preserve` or `AspectRatio::Ratio { width, height }` | `Preserve` |
| `framing` | `Fit`, `Crop`, `Expand`, `Stretch` | `Fit` |
| `transparent` | `bool` | `false` |
| `supersample` | `u32` | `2`; must be > 0 and fit the render-size multiplication |
| `fps` | `u32` | `60`; used when `frame_rate` stays `None` |
| `frame_rate` | `Option<FrameRate>` | Exact reduced rational rate when set |
| `mode` | `Realtime` or `Offline` | `Realtime` |
| `duration` | `Option<Duration>` | Optional schedule source |
| `frame_count` | `Option<u64>` | Overrides `duration` schedule when set |
| `output` | `Option<OutputSpec>` | Required by normal `stop()` |
| `parallel_workers` | `usize` | `2`; core clamps worker count to `1..=32` |

`CaptureConfig::default()` sets `supersample: 2`, so the renderer produces 2x
the final width and height. The capture core downsamples to final dimensions
before PNG or animation encode.

## Script example

Keep capture control in one state owner. `stop()` only requests a safe app
boundary; poll `state()` or `last_output()` on later updates.

```rust
use perro_api::prelude::*;
#[derive(Default, Variant)]
struct CaptureState {
    started: bool,
    stop_requested: bool,
    finished: bool,
    started_at: f32,
}

#[State]
struct GameState {
    capture: CaptureState,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let started_at = elapsed_time!(ctx.run);
        let config = CaptureConfig {
            mode: CaptureMode::Realtime,
            fps: 30,
            output: Some(OutputSpec::new(
                ".output/director.gif",
                OutputFormat::Gif,
            )),
            ..CaptureConfig::default()
        };
        match ctx.run.Capture().start(config) {
            Ok(()) => {
                with_state_mut!(ctx.run, GameState, ctx.id, |state| {
                    state.capture.started = true;
                    state.capture.started_at = started_at;
                });
            }
            Err(error) => log_error!("capture start fail: {error}"),
        }
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let elapsed = elapsed_time!(ctx.run);
        let stop = with_state_mut!(ctx.run, GameState, ctx.id, |state| {
            if state.capture.started
                && !state.capture.stop_requested
                && elapsed - state.capture.started_at >= 2.0
            {
                state.capture.stop_requested = true;
                true
            } else {
                false
            }
        }).unwrap_or(false);
        if stop {
            self.request_stop(ctx);
        }

        if ctx.run.Capture().last_output().is_some() {
            with_state_mut!(ctx.run, GameState, ctx.id, |state| {
                if !state.capture.finished {
                    state.capture.finished = true;
                    log_info!("capture GIF ready");
                }
            });
        }
    }
});

methods!({
    fn request_stop(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Err(error) = ctx.run.Capture().stop() {
            log_error!("capture stop request fail: {error}");
        }
    }
});
```

`OutputFormat::Gif` uses the native encoder and needs no external process. It
uses one palette for the full clip, keeps exact colors when they fit, applies a
fixed alpha cutoff, and varies GIF delays to keep the requested average rate.
This avoids per-frame palette shimmer and transparent-frame trails. GIF still
has 256-color and one-bit-alpha format limits. `OutputFormat::AnimatedWebP`
uses ffmpeg lossless BGRA output with full alpha and infinite looping.
`OutputSpec::new` accepts a file path for GIF and encoded animation formats;
PNG sequence output uses a directory path.

## Sources

Use these constructors from the prelude:

| Source | Constructor | Route |
| --- | --- | --- |
| Main compositor | `CaptureSource::main_window()` | Final scene + UI compositor |
| 2D camera | `CaptureSource::camera_2d("Camera2D")` | Named `Camera2D` node |
| 3D camera | `CaptureSource::camera_3d("Camera3D")` | Named `Camera3D` node |
| 2D camera by ID | `CaptureSource::camera_2d_node(camera_id)` | Exact live `NodeID`; no name lookup |
| 3D camera by ID | `CaptureSource::camera_3d_node(camera_id)` | Exact live `NodeID`; no name lookup |
| UI sub-view | `CaptureSource::ui_sub_view("Hud")` | Named `UiSubView` node |
| Render target | `CaptureSource::render_target("Viewport")` | Existing raster target |

Names resolve against live scene nodes. Empty names, missing nodes, and wrong
node types return an error from `start`. A render target already contains
raster pixels; `Framing::Expand` rejects it because no extra view exists.

## Typed camera GIF guide

Pass a live `NodeID` when duplicate node names or stale name routes create risk.
The runtime validates generation + type, and owns a capture stream sized for
supersampling. Set GIF output from a caller-selected path:

```rust
use std::path::PathBuf;
use perro_api::prelude::*;

methods!({
    fn capture_camera_gif(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        camera_id: NodeID,
        output_path: PathBuf,
    ) -> Result<(), String> {
        let config = CaptureConfig {
            source: CaptureSource::camera_2d_node(camera_id),
            output: Some(OutputSpec::new(output_path, OutputFormat::Gif)),
            ..CaptureConfig::default()
        };
        ctx.run.Capture().start(config)
    }
});
```

Use `CaptureSource::camera_3d_node(camera_id)` for a `Camera3D` node. A stale
ID or wrong camera kind fails at `start`; runtime never falls back to a name.

## Size + framing

`width` and `height` describe final output size. Set both for exact dimensions,
set one to derive the other from `aspect_ratio`, or omit both to keep source
dimensions. `render_size()` reports the pre-downsample dimensions:

```text
render_width  = output_width  * supersample
render_height = output_height * supersample
```

Framing applies when source and output aspects differ:

- `Fit` keeps all source pixels and letterboxes as needed.
- `Crop` fills output and crops excess source pixels.
- `Stretch` scales each axis independently.
- `Expand` widens a camera view to fill output; raster targets reject it.

Transparent capture clears background alpha to zero. World and IBL lighting
still shade 3D objects; opaque environment background stays out of the alpha
output.

## Timing

`FrameRate` stores a reduced exact rational. Build one with
`FrameRate::new(30000, 1001)` or parse decimal / `num/den` text with
`FrameRate::parse`.

Offline mode uses a fixed schedule. `duration * frame_rate` yields its frame
count, and frame timestamps start at `0` with a step of `1 / rate`. Set
`frame_count` to provide an exact count directly. Replay actions apply before
the target fixed tick.

Realtime mode samples elapsed wall time at the requested rate. Missed sample
slots reuse the latest completed frame; metadata records the duplicate count.
The app bounds submissions per present and the capture core bounds encoder
queue capacity.

## Lifecycle + state

Normal script flow:

1. Set `config.output` and call `start(config)`.
2. Read `state()`, `progress()`, `source_route()`, and `render_size()` while active.
3. Add director events with `record_action(...)` when needed.
4. Call `stop()` to request a safe app-boundary stop.
5. Poll `last_output()` after the boundary.

`state()` reports the active session state. App-boundary stop drains and packs
on a background worker; `state()` reports `Draining` until the app observes
completion. A new capture cannot start during this phase. After commit,
`state()` returns `None` and `last_output()` holds the committed output.
Raw host `finish_raw()` still waits for drain and commit.

`progress()` reports submitted frames, completed encoded frames, and the
expected schedule count. `completed_frames()` reports only fully encoded PNG
frames.

## Replay actions

`record_action(timestamp, action, payload)` inserts a timestamped event into the
ordered action timeline. The capture metadata stores the timeline for replay.
Supported runtime action names include:

| Action | Payload |
| --- | --- |
| `input:key_down` / `input:key_up` | Key name, such as `Space` |
| `input:mouse_down` / `input:mouse_up` | Mouse button name |
| `input:text` | Text payload |
| `signal` / `signal:<name>` | Signal name |

## Output + errors

`OutputFormat::PngSequence` keeps numbered PNG files in an output directory.
`Gif` uses the native global-palette encoder. `WebM`, `Mp4`, and
`AnimatedWebP` invoke `ffmpeg` from `PATH`; use
`OutputSpec::with_ffmpeg(path)` for an explicit executable. Animated WebP uses
lossless BGRA encoding and infinite looping.

The core writes canonical PNG frames into a staging directory. Final media and
metadata use temporary sibling files and atomic rename. Successful commit
removes staging. Failed or abandoned sessions also remove their intermediate
frames after workers stop; failed packs remove temporary output. Final PNG
sequence frames remain in the requested output directory. Cleanup on drop is
best effort if the filesystem rejects removal. No output commit means `last_output()` stays
`None` for that session.

`start`, `record_action`, `submit_rgba`, `drain`, `stop`, and raw host methods
return `Result` for immediate validation or request errors. Source lookup,
invalid dimensions, zero supersample, invalid rate, schedule overflow, output
existence, encoder absence, and encoder failure surface as errors. Renderer or
async pack errors also print through the app/CLI capture error path; the public
module has no separate `last_error()` accessor.

The CLI limits `--supersample` to integer `1..=8`. The public config only
requires a positive value and rejects multiplication overflow. Keep worker
counts bounded; the core clamps `parallel_workers` to `1..=32`.

## Host helpers

Renderer hosts that own source dimensions and staging paths use the explicit
raw surface:

```rust
capture.start_raw(config, source_size, staging_root)?;
let size = capture.render_size().expect("active capture");
capture.submit_rgba(frame_index, size.width, size.height, rgba)?;
capture.drain()?;
let result = capture.finish_raw(output)?;
```

Raw helpers bypass the normal app bridge. Do not use them for ordinary gameplay
scripts; the graphics runner owns callback setup and readback draining.

## API reference

| Method | Signature | Behavior |
| --- | --- | --- |
| `start` | `fn start(&mut self, config: CaptureConfig) -> Result<(), String>` | Resolve project source/staging and start session |
| `start_raw` | `fn start_raw(&mut self, config: CaptureConfig, source_size: OutputSize, staging_root: &str) -> Result<(), String>` | Start with host-owned integration values |
| `state` | `fn state(&self) -> Option<CaptureSessionState>` | Read lifecycle state |
| `progress` | `fn progress(&self) -> Option<CaptureProgress>` | Read queue progress |
| `last_output` | `fn last_output(&self) -> Option<FinalizedCapture>` | Read committed paths and dimensions |
| `source` | `fn source(&self) -> Option<CaptureSource>` | Read active source |
| `source_route` | `fn source_route(&self) -> Option<CaptureSourceRoute>` | Read resolved node route and source size |
| `render_size` | `fn render_size(&self) -> Option<OutputSize>` | Read pre-downsample size |
| `completed_frames` | `fn completed_frames(&self) -> u64` | Read encoded frame count |
| `record_action` | `fn record_action(&mut self, timestamp: Duration, action: impl Into<String>, payload: Option<String>) -> Result<(), String>` | Add replay event |
| `stop` | `fn stop(&mut self) -> Result<(), String>` | Request configured output stop at app boundary |
| `request_stop` | `fn request_stop(&mut self, output: OutputSpec)` | Request output override stop |
| `submit_rgba` | `fn submit_rgba(&mut self, frame_index: u64, width: u32, height: u32, rgba: &[u8]) -> Result<u64, String>` | Raw host frame submit |
| `drain` | `fn drain(&self) -> Result<(), String>` | Raw host encode wait |
| `finish_raw` | `fn finish_raw(&mut self, output: OutputSpec) -> Result<FinalizedCapture, String>` | Raw host drain + commit |
