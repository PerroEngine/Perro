# Mic Module

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Use Cases     | [Use Cases](#use-cases)         |
| Overview      | [Overview](#overview)           |
| Context       | [Context](#context)             |
| Pick A Device | [Pick A Device](#pick-a-device) |
| Practical Example | [Practical Example](#practical-example) |
| Send Bytes    | [Send Bytes](#send-bytes)       |
| API Reference | [API Reference](#api-reference) |
| Macros        | [Macros](#macros)               |

## Purpose

`ctx.res.Mic()` captures live microphone audio and hands your script either a rolling `MicClip` or drained packet-ready bytes. The engine owns capture, denoise, and the compact `PMIC` byte codec; your game owns transport, recipients, and playback. Use it for proximity voice chat, voice recording, and any feature that turns the player's mic into gameplay.

## Use Cases

- Mic selection menu: list every connected input with `mic_devices!`, show `label`, and start capture on the pick with `mic_start_on!`.
- Remembering the player's mic: save the chosen `name` in your settings file and reopen it next launch with `mic_resolve_device!`, which falls back to the OS default when the device is unplugged.
- Push-to-talk voice chat: `mic_start_stream!` while the key is held, drain packets with `mic_get_bytes!`, and send them over your own transport.
- Playing received voice: decode a peer's packet with `mic_unpack!` and hand the clip to the audio bus with `audio_play_clip!`.
- Voice memos / clip recording: `mic_start!`, then `mic_stop!` to take the full buffer, and `mic_save_wav!` to store it.
- Noise-gated voice: capture with `MicDenoiseSettings::voice()` or clean an existing clip with `MicClip::denoised`.
- Voice-driven mechanics: read the rolling buffer with `mic_clip!` to measure loudness for a "shout to scare enemies" or lip-sync feature.
- Bandwidth-friendly networking: pack a clip with `mic_pack!` to the smallest `PMIC` codec before sending.

## Ownership And Choice

The microphone module owns capture lifetime and byte delivery; a game system owns consent, UI state, encoding, and network/storage policy. Use it for explicit voice or audio-input features. Do not start capture as a hidden side effect of an unrelated scene. Start once, consume bounded chunks, stop when the feature ends, and treat absent permission/device data as normal failure.

## Overview

Use `ctx.res.Mic()` for live microphone bytes and optional recorded clips.

Mic clips are `MicClip` values:

- PCM16 samples
- input sample rate
- channel count
- optional denoise pass
- compressed packable bytes for UDP, TCP, HTTP, or save data
- WAV save support

The mic is a live stream while capture is active.
Call `get_clip` or `get_bytes` when your game decides it is time to send.
Those calls drain new audio since the last stream/get read.
`clip` returns the full rolling recording buffer.
`stop_listening` stops capture and returns that full buffer.

Packed mic bytes use `PMIC`.
Unpack supports raw v1 and compressed v2.
Pack chooses the smallest engine codec from raw PCM, zlib PCM, delta PCM, and zlib delta PCM.
They are engine bytes, not Opus voice-chat bytes.
Use them for simple send/store first.
Add Opus later for real voice chat bandwidth.

Proximity chat split:

- engine owns mic capture and encode/decode
- audio API owns clip playback
- game/server owns who hears whom, room/team rules, auth, mute, push-to-talk, and net relay
- client captures while push-to-talk or VAD is active
- client drains live bytes and sends packed bytes to server
- server filters recipients by position, team, room, or other game rules
- receiving client decodes frames and gives clips to the audio API

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Mic()`
- Native backend: `cpal` (WASAPI on Windows, CoreAudio on macOS, ALSA on Linux)
- Devices: any OS input works, so USB, XLR through an interface, headset, wireless, and virtual-cable mics all list and open
- Format: rate, channel count, and sample format come from the device; f32, i16, and u16 streams all convert to the `MicClip` i16 format
- Wasm backend: unsupported, device scan returns an empty list and capture returns an error or empty clip
- Audio output: use `ctx.res.Audio()` with `MicClip`

## Pick A Device

Scan first, cache the `name`, start on the cached name.

`ctx.res.Mic().devices()` rescans on every call because wireless and USB mics come and go. Each entry carries a `name` (the selection key), a `label` for the menu, and `is_default`. Duplicate hardware gets a `#2` suffix on the label while both keep their own name.

Build the menu:

```rust
let devices = mic_devices!(ctx.res).unwrap_or_default();
for device in &devices {
    // device.label for the row text, device.name for the value you store.
    let _ = (&device.label, &device.name, device.is_default);
}
```

Start on the pick and store the name in your own settings:

```rust
if let Err(err) = mic_start_on!(ctx.res, &chosen_name) {
    // Device unplugged between the scan and the click.
    let _ = err;
    let _ = mic_start!(ctx.res);
}
```

Reopen a cached name next launch. `resolve_device` returns the cached mic when it is still connected and the OS default when it is gone, so a missing mic never blocks the feature:

```rust
if let Some(device) = mic_resolve_device!(ctx.res, saved_name.as_deref()) {
    let _ = mic_start_with!(ctx.res, device.settings());
}
```

Settings-struct form, for capture options plus a device:

```rust
let settings = MicSettings::default()
    .with_device(&chosen_name)
    .with_max_seconds(8.0)
    .with_denoise(MicDenoiseSettings::voice());
let _ = mic_start!(ctx.res, settings);
```

Rules that keep selection working:

- An empty or absent `device` opens the OS default.
- A name that no longer exists returns `Err`; nothing silently swaps to another mic.
- Matching is by name, never by list position, so a rescan or reorder keeps the cached pick.
- Read the live name with `mic_device!` and the failure text with `mic_last_error!`.
- A mic yanked mid-capture drops `mic_is_listening!` to `false`; `mic_stop!` still returns the audio captured before the loss.

## Practical Example

Hold `R` to record and stream mic bytes, press `T` to stop and play the clip back. The stop handler is split into a `methods!` helper.

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if key_pressed!(ctx.ipt, KeyCode::KeyR) {
            let _ = mic_start!(ctx.res);
        }

        if key_down!(ctx.ipt, KeyCode::KeyR) {
            if let Some(bytes) = mic_get_bytes!(ctx.res) {
                // send bytes over UDP/TCP/HTTP here if you want
                let _ = bytes;
            }
        }

        if key_pressed!(ctx.ipt, KeyCode::KeyT) {
            self.finish_recording(ctx);
        }
    }
});

methods!({
    fn finish_recording(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Some(clip) = mic_stop!(ctx.res) {
            let _ = audio_play!(ctx.res, &clip);
            let bytes = mic_pack!(ctx.res, &clip);
            let copy = mic_unpack!(ctx.res, &bytes).ok();
            let _ = copy;
            let _ = mic_save_wav!(ctx.res, "user://recordings/last.wav", &clip);
        }
    }
});
```

With settings:

```rust
let settings = MicSettings {
    max_seconds: 8.0,
    ..Default::default()
};
let _ = mic_start!(ctx.res, settings);
```

With denoise:

```rust
let settings = MicSettings {
    max_seconds: 8.0,
    denoise: MicDenoiseSettings::voice(),
    ..Default::default()
};
let _ = mic_start!(ctx.res, settings);
```

Clip cleanup:

```rust
if let Some(clip) = mic_clip!(ctx.res) {
    let clean = clip.denoised(MicDenoiseSettings::voice());
    let _ = audio_play!(ctx.res, &clean);
}
```

Audio bus playback:

```rust
if let Some(clip) = mic_clip!(ctx.res) {
    let voice = audio_bus!("voice");
    let _ = audio_play_clip!(ctx.res, voice, &clip, 0.8);
}
```

Live receive:

```rust
// bytes came from a remote speaker packet.
if let Ok(clip) = mic_unpack!(ctx.res, &bytes) {
    let _ = audio_play_clip!(ctx.res, audio_bus!("voice"), &clip, 1.0);
}
```

## Send Bytes

Mic networking is game-owned.
The engine gives packet-ready bytes.
Your game chooses transport, server relay, recipients, and playback position.
The engine does not tick, schedule, or send mic packets.
Your script calls `mic_get_bytes!` at the send rate you choose.

Client talk loop:

```rust
if push_to_talk {
    let _ = mic_start_stream!(ctx.res);

    if let Some(bytes) = mic_get_bytes!(ctx.res) {
        // UdpEndpoint / NetworkWorld send call owned by game net state.
        let _ = voice_udp.send_to(&bytes, server_addr);
    }
} else if mic_is_listening!(ctx.res) {
    let _ = mic_stop_stream!(ctx.res);
}
```

Server relay loop:

```rust
// Decode only if server needs metadata from the bytes.
// Otherwise relay bytes as-is to chosen recipients.
if player_can_hear_talker {
    let _ = voice_udp.send_to(&bytes, listener_addr);
}
```

Client receive loop:

```rust
if let Ok(clip) = mic_unpack!(ctx.res, &bytes) {
    // Pick speaker position from replicated game state.
    let _ = audio_play_clip!(ctx.res, audio_bus!("voice"), &clip, 1.0);
}
```

UDP notes:

- drain every net tick
- keep frames small
- prefer ~20ms to ~60ms ticks for voice
- drop old frames over reliable resend
- use TCP/HTTP only for recorded clips or non-realtime upload

## API Reference

### `devices`

| Field     | Detail                                                            |
| --------- | ----------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                   |
| Signature | `pub fn devices(&self) -> Result<Vec<MicDevice>, String>`         |
| Returns   | `Result<Vec<MicDevice>, String>`                                  |
| Use when  | List the connected input devices for a selection menu. Rescans on every call. |

### `scan`

| Field     | Detail                                                 |
| --------- | ------------------------------------------------------ |
| Access    | `ctx.res.Mic()`                                        |
| Signature | `pub fn scan(&self) -> Result<Vec<MicDevice>, String>` |
| Returns   | `Result<Vec<MicDevice>, String>`                       |
| Use when  | Same as `devices`, named for a refresh button.         |

### `MicDevice`

| Field         | Type     | Detail                                                    |
| ------------- | -------- | --------------------------------------------------------- |
| `name`        | `String` | Selection key. Store this to remember the player's choice. |
| `label`       | `String` | Menu text. Suffixed `#2` when two devices share a name.    |
| `is_default`  | `bool`   | OS default input.                                          |
| `sample_rate` | `u32`    | Device default rate, `0` when the backend hides it.        |
| `channels`    | `u16`    | Device default channel count, `0` when unknown.            |

Call `device.settings()` for default capture settings already aimed at that device.

### `default_device`

| Field     | Detail                                              |
| --------- | --------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                     |
| Signature | `pub fn default_device(&self) -> Option<MicDevice>` |
| Returns   | `Option<MicDevice>`                                 |
| Use when  | Preselect the OS default row in a menu.             |

### `find_device`

| Field     | Detail                                                        |
| --------- | ------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                               |
| Signature | `pub fn find_device(&self, name: &str) -> Option<MicDevice>`  |
| Returns   | `Option<MicDevice>`                                           |
| Use when  | Check whether a cached name is still connected, with no fallback. |

### `resolve_device`

| Field     | Detail                                                                    |
| --------- | ------------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                           |
| Signature | `pub fn resolve_device(&self, name: Option<&str>) -> Option<MicDevice>`   |
| Returns   | `Option<MicDevice>`                                                       |
| Use when  | Reopen a cached pick, falling back to the OS default when it is unplugged. |

### `has_device`

| Field     | Detail                                            |
| --------- | ------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                   |
| Signature | `pub fn has_device(&self, name: &str) -> bool`    |
| Returns   | `bool`                                            |
| Use when  | Grey out a saved device row that is not plugged in. |

### `start_on`

| Field     | Detail                                                                  |
| --------- | ----------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                         |
| Signature | `pub fn start_on<S: Into<String>>(&self, device: S) -> Result<(), String>` |
| Returns   | `Result<(), String>`                                                    |
| Use when  | Capture from one named device. Errs when that device is gone.           |

### `start_on_with`

| Field     | Detail                                                                                            |
| --------- | ------------------------------------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                                                   |
| Signature | `pub fn start_on_with<S: Into<String>>(&self, device: S, settings: MicSettings) -> Result<(), String>` |
| Returns   | `Result<(), String>`                                                                              |
| Use when  | Capture from a named device with custom length and denoise.                                       |

### `device`

| Field     | Detail                                        |
| --------- | --------------------------------------------- |
| Access    | `ctx.res.Mic()`                               |
| Signature | `pub fn device(&self) -> Option<String>`      |
| Returns   | `Option<String>`                              |
| Use when  | Show which mic the current capture runs on.   |

### `last_error`

| Field     | Detail                                                       |
| --------- | ------------------------------------------------------------ |
| Access    | `ctx.res.Mic()`                                              |
| Signature | `pub fn last_error(&self) -> Option<String>`                 |
| Returns   | `Option<String>`                                             |
| Use when  | Report why capture stopped, including a mic unplugged mid-stream. |

### `start_listening`

| Field     | Detail                                                |
| --------- | ----------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                       |
| Signature | `pub fn start_listening(&self) -> Result<(), String>` |
| Returns   | `Result<(), String>`                                  |
| Use when  | Start mic capture with default settings.              |

### `start_stream`

| Field     | Detail                                             |
| --------- | -------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                    |
| Signature | `pub fn start_stream(&self) -> Result<(), String>` |
| Returns   | `Result<(), String>`                               |
| Use when  | Start live mic stream capture.                     |

### `start_with`

| Field     | Detail                                                                  |
| --------- | ----------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                         |
| Signature | `pub fn start_with(&self, settings: MicSettings) -> Result<(), String>` |
| Returns   | `Result<(), String>`                                                    |
| Use when  | Start mic capture with max clip seconds and optional denoise.           |

### `MicSettings`

| Field         | Type                 | Detail                                                        |
| ------------- | -------------------- | ------------------------------------------------------------- |
| `max_seconds` | `f32`                | Rolling capture length.                                       |
| `denoise`     | `MicDenoiseSettings` | Capture-time denoise settings.                                |
| `device`      | `Option<String>`     | Device name from `devices()`. `None` or blank opens the OS default. |
| `channels`    | `MicChannels`        | Clip channel layout.                                          |

Builders: `with_device`, `with_default_device`, `with_max_seconds`, `with_denoise`, `with_channels`.

### `MicChannels`

| Variant  | Detail                                                            |
| -------- | ----------------------------------------------------------------- |
| `Auto`   | Default. Mono and stereo mics stay as-is, wider interfaces fold to mono. |
| `Mono`   | Always fold to one channel. Smallest voice packets.                |
| `Device` | Keep the device layout, including 4 or 8 channel interfaces.       |

Folding averages the channels of each frame, so an 8-in interface with one live XLR input records quieter than the same mic on a mono device. Pick `Device` when the game wants every input channel.

### `MicDenoiseSettings`

| Field         | Type   | Detail                                      |
| ------------- | ------ | ------------------------------------------- |
| `enabled`     | `bool` | Enable denoise pass.                        |
| `noise_floor` | `f32`  | Samples below this level get reduced.       |
| `reduction`   | `f32`  | Quiet-sample gain cut, from `0.0` to `1.0`. |
| `high_pass`   | `bool` | Remove low rumble/DC drift.                 |

Use `MicDenoiseSettings::voice()` for a default voice gate.
Use `MicDenoiseSettings::off()` to disable it.

### `denoised`

| Field     | Detail                                                            |
| --------- | ----------------------------------------------------------------- |
| Access    | `MicClip`                                                         |
| Signature | `pub fn denoised(&self, settings: MicDenoiseSettings) -> MicClip` |
| Returns   | `MicClip`                                                         |
| Use when  | Clean a captured clip without changing the active capture stream. |

### `compressed_bytes`

| Field     | Detail                                              |
| --------- | --------------------------------------------------- |
| Access    | `MicClip`                                           |
| Signature | `pub fn compressed_bytes(&self) -> Vec<u8>`         |
| Returns   | `Vec<u8>`                                           |
| Use when  | Pack with the smallest available `PMIC` byte codec. |

### `raw_bytes`

| Field     | Detail                                      |
| --------- | ------------------------------------------- |
| Access    | `MicClip`                                   |
| Signature | `pub fn raw_bytes(&self) -> Vec<u8>`        |
| Returns   | `Vec<u8>`                                   |
| Use when  | Force legacy raw `PMIC` v1 bytes.           |

### `compression_ratio`

| Field     | Detail                                      |
| --------- | ------------------------------------------- |
| Access    | `MicClip`                                   |
| Signature | `pub fn compression_ratio(&self) -> f32`    |
| Returns   | `f32`                                       |
| Use when  | Compare packed byte length to raw v1 length. |

### `stop_listening`

| Field     | Detail                                            |
| --------- | ------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                   |
| Signature | `pub fn stop_listening(&self) -> Option<MicClip>` |
| Returns   | `Option<MicClip>`                                 |
| Use when  | Stop capture and take the recorded clip.          |

### `stop_stream`

| Field     | Detail                                                   |
| --------- | -------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                          |
| Signature | `pub fn stop_stream(&self) -> Option<MicClip>`           |
| Returns   | `Option<MicClip>`                                        |
| Use when  | Stop live mic stream and take the rolling recorded clip. |

### `clip`

| Field     | Detail                                                    |
| --------- | --------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                           |
| Signature | `pub fn clip(&self) -> Option<MicClip>`                   |
| Returns   | `Option<MicClip>`                                         |
| Use when  | Read a copy of the current clip without stopping capture. |

### `get_clip`

| Field     | Detail                                                 |
| --------- | ------------------------------------------------------ |
| Access    | `ctx.res.Mic()`                                        |
| Signature | `pub fn get_clip(&self) -> Option<MicClip>`            |
| Returns   | `Option<MicClip>`                                      |
| Use when  | Drain new live mic samples since the last stream read. |

### `get_bytes`

| Field     | Detail                                                            |
| --------- | ----------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                   |
| Signature | `pub fn get_bytes(&self) -> Option<Vec<u8>>`                      |
| Returns   | `Option<Vec<u8>>`                                                 |
| Use when  | Drain new live mic samples as compressed `PMIC` bytes for networking. |

### `is_listening`

| Field     | Detail                               |
| --------- | ------------------------------------ |
| Access    | `ctx.res.Mic()`                      |
| Signature | `pub fn is_listening(&self) -> bool` |
| Returns   | `bool`                               |
| Use when  | Check whether capture is active.     |

### `save_wav`

| Field     | Detail                                                                                      |
| --------- | ------------------------------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                                             |
| Signature | `pub fn save_wav<S: ResPathSource>(&self, source: S, clip: &MicClip) -> Result<(), String>` |
| Returns   | `Result<(), String>`                                                                        |
| Use when  | Save a recorded clip as `.wav`.                                                             |

### `pack`

| Field     | Detail                                               |
| --------- | ---------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                      |
| Signature | `pub fn pack(&self, clip: &MicClip) -> Vec<u8>`      |
| Returns   | `Vec<u8>`                                            |
| Use when  | Convert clip to smallest `PMIC` bytes for network or storage. |

### `unpack`

| Field     | Detail                                                          |
| --------- | --------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                 |
| Signature | `pub fn unpack(&self, bytes: &[u8]) -> Result<MicClip, String>` |
| Returns   | `Result<MicClip, String>`                                       |
| Use when  | Convert raw v1 or compressed v2 `PMIC` bytes back to a `MicClip`. |

## Macros

| Macro                                               | Expands to                                          |
| --------------------------------------------------- | --------------------------------------------------- |
| `mic_devices!(ctx.res)`                             | `ctx.res.Mic().devices()`                           |
| `mic_scan!(ctx.res)`                                | `ctx.res.Mic().scan()`                              |
| `mic_default_device!(ctx.res)`                      | `ctx.res.Mic().default_device()`                    |
| `mic_find_device!(ctx.res, name)`                   | `ctx.res.Mic().find_device(name)`                   |
| `mic_resolve_device!(ctx.res, name)`                | `ctx.res.Mic().resolve_device(name)`                |
| `mic_has_device!(ctx.res, name)`                    | `ctx.res.Mic().has_device(name)`                    |
| `mic_start_on!(ctx.res, name)`                      | `ctx.res.Mic().start_on(name)`                      |
| `mic_start_on!(ctx.res, name, settings)`            | `ctx.res.Mic().start_on_with(name, settings)`       |
| `mic_device!(ctx.res)`                              | `ctx.res.Mic().device()`                            |
| `mic_last_error!(ctx.res)`                          | `ctx.res.Mic().last_error()`                        |
| `mic_start!(ctx.res)`                               | `ctx.res.Mic().start_listening()`                   |
| `mic_start!(ctx.res, settings)`                     | `ctx.res.Mic().start_with(settings)`                |
| `mic_start_listening!(ctx.res)`                     | `ctx.res.Mic().start_listening()`                   |
| `mic_start_stream!(ctx.res)`                        | `ctx.res.Mic().start_stream()`                      |
| `mic_start_with!(ctx.res, settings)`                | `ctx.res.Mic().start_with(settings)`                |
| `mic_record!(ctx.res)`                              | `ctx.res.Mic().record()`                            |
| `mic_stop!(ctx.res)`                                | `ctx.res.Mic().stop_listening()`                    |
| `mic_stop_listening!(ctx.res)`                      | `ctx.res.Mic().stop_listening()`                    |
| `mic_stop_stream!(ctx.res)`                         | `ctx.res.Mic().stop_stream()`                       |
| `mic_clip!(ctx.res)`                                | `ctx.res.Mic().clip()`                              |
| `mic_get_clip!(ctx.res)`                            | `ctx.res.Mic().get_clip()`                          |
| `mic_get_bytes!(ctx.res)`                           | `ctx.res.Mic().get_bytes()`                         |
| `mic_stream_clip!(ctx.res)`                         | `ctx.res.Mic().stream_clip()`                       |
| `mic_stream_bytes!(ctx.res)`                        | `ctx.res.Mic().stream_bytes()`                      |
| `mic_frame!(ctx.res)`                               | `ctx.res.Mic().stream_clip()`                       |
| `mic_frame_bytes!(ctx.res)`                         | `ctx.res.Mic().stream_bytes()`                      |
| `mic_is_listening!(ctx.res)`                        | `ctx.res.Mic().is_listening()`                      |
| `mic_save_wav!(ctx.res, path, &clip)`               | `ctx.res.Mic().save_wav(path, &clip)`               |
| `mic_pack!(ctx.res, &clip)`                         | `ctx.res.Mic().pack(&clip)`                         |
| `mic_unpack!(ctx.res, &bytes)`                      | `ctx.res.Mic().unpack(&bytes)`                      |
