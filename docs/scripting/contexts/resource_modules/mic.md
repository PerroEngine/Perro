# Mic Module

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Overview      | [Overview](#overview)           |
| Context       | [Context](#context)             |
| Example       | [Example](#example)             |
| Send Bytes    | [Send Bytes](#send-bytes)       |
| API Reference | [API Reference](#api-reference) |
| Macros        | [Macros](#macros)               |

## Overview

Use `ctx.res.Mic()` for live microphone bytes and optional recorded clips.

Mic clips are `MicClip` values:

- PCM16 samples
- input sample rate
- channel count
- optional denoise pass
- compressed packable bytes for UDP, TCP, HTTP, or save data
- WAV save support
- playback through `perro_pawdio`

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

- engine owns mic capture, encode/decode, jitter buffers, and 2D/3D playback hooks
- game/server owns who hears whom, room/team rules, auth, mute, push-to-talk, and net relay
- client captures while push-to-talk or VAD is active
- client drains live bytes and sends packed bytes to server
- server filters recipients by position, team, room, or other game rules
- receiving client decodes frames and plays them from speaker entity space

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Mic()`
- Native backend: `cpal`
- Wasm backend: unsupported, returns an error or empty clip
- Audio output: mic clip playback goes through the audio backend and buses

## Example

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
            if let Some(clip) = mic_stop!(ctx.res) {
                let _ = mic_play!(ctx.res, &clip);
                let bytes = mic_pack!(ctx.res, &clip);
                let copy = mic_unpack!(ctx.res, &bytes).ok();
                let _ = copy;
                let _ = mic_save_wav!(ctx.res, "user://recordings/last.wav", &clip);
            }
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
};
let _ = mic_start!(ctx.res, settings);
```

Clip cleanup:

```rust
if let Some(clip) = mic_clip!(ctx.res) {
    let clean = clip.denoised(MicDenoiseSettings::voice());
    let _ = mic_play!(ctx.res, &clean);
}
```

Bus playback:

```rust
if let Some(clip) = mic_clip!(ctx.res) {
    let voice = audio_bus!("voice");
    let _ = mic_play!(ctx.res, voice, &clip, 0.8);
}
```

Live receive:

```rust
// bytes came from a remote speaker packet.
if let Ok(clip) = mic_unpack!(ctx.res, &bytes) {
    let _ = mic_play!(ctx.res, audio_bus!("voice"), &clip, 1.0);
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
    let _ = mic_play_bus_volume!(ctx.res, audio_bus!("voice"), &clip, 1.0);
}
```

UDP notes:

- drain every net tick
- keep frames small
- prefer ~20ms to ~60ms ticks for voice
- drop old frames over reliable resend
- use TCP/HTTP only for recorded clips or non-realtime upload

## API Reference

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

| Field         | Type                 | Detail                         |
| ------------- | -------------------- | ------------------------------ |
| `max_seconds` | `f32`                | Rolling capture length.        |
| `denoise`     | `MicDenoiseSettings` | Capture-time denoise settings. |

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

### `play_master`

| Field     | Detail                                              |
| --------- | --------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                     |
| Signature | `pub fn play_master(&self, clip: &MicClip) -> bool` |
| Returns   | `bool`                                              |
| Use when  | Play recorded clip on master output.                |

### `play_bus`

| Field     | Detail                                                               |
| --------- | -------------------------------------------------------------------- |
| Access    | `ctx.res.Mic()`                                                      |
| Signature | `pub fn play_bus(&self, bus_id: AudioBusID, clip: &MicClip) -> bool` |
| Returns   | `bool`                                                               |
| Use when  | Play recorded clip through an audio bus.                             |

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
| `mic_play!(ctx.res, &clip)`                         | `ctx.res.Mic().play_master(&clip)`                  |
| `mic_play!(ctx.res, bus, &clip)`                    | `ctx.res.Mic().play_bus(bus, &clip)`                |
| `mic_play!(ctx.res, bus, &clip, volume)`            | `ctx.res.Mic().play_bus_volume(bus, &clip, volume)` |
| `mic_play_master!(ctx.res, &clip)`                  | `ctx.res.Mic().play_master(&clip)`                  |
| `mic_play_master_volume!(ctx.res, &clip, volume)`   | `ctx.res.Mic().play_master_volume(&clip, volume)`   |
| `mic_play_bus!(ctx.res, bus, &clip)`                | `ctx.res.Mic().play_bus(bus, &clip)`                |
| `mic_play_bus_volume!(ctx.res, bus, &clip, volume)` | `ctx.res.Mic().play_bus_volume(bus, &clip, volume)` |
| `mic_save_wav!(ctx.res, path, &clip)`               | `ctx.res.Mic().save_wav(path, &clip)`               |
| `mic_pack!(ctx.res, &clip)`                         | `ctx.res.Mic().pack(&clip)`                         |
| `mic_unpack!(ctx.res, &bytes)`                      | `ctx.res.Mic().unpack(&bytes)`                      |
