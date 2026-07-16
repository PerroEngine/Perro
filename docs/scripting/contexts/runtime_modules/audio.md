# Audio Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `set_debug_rays` | [`set_debug_rays`](#set_debug_rays) |
| `debug_rays_enabled` | [`debug_rays_enabled`](#debug_rays_enabled) |
| `play_attached` | [`play_attached`](#play_attached) |
| `play_attached_bus` | [`play_attached_bus`](#play_attached_bus) |
| `stop_attached` | [`stop_attached`](#stop_attached) |
| `midi` | [`midi`](#midi) |
| `play_note_attached` | [`play_note_attached`](#play_note_attached) |
| `start_note_attached` | [`start_note_attached`](#start_note_attached) |
| `play_file_attached` | [`play_file_attached`](#play_file_attached) |
| `release_note` | [`release_note`](#release_note) |
| `stop_attached` | [`stop_attached`](#stop_attached) |
| `audio_play_attached` | [`audio_play_attached`](#audio_play_attached) |
| `midi_play_attached` | [`midi_play_attached`](#midi_play_attached) |
| `midi_start_attached` | [`midi_start_attached`](#midi_start_attached) |
| `midi_release_attached` | [`midi_release_attached`](#midi_release_attached) |
| `midi_stop_attached` | [`midi_stop_attached`](#midi_stop_attached) |

## Purpose

The runtime audio module plays sound positioned in the world. Instead of a flat
2D clip, a sound is attached to a scene node so it pans and attenuates as the
listener moves, which is what makes footsteps, gunfire, and ambience feel like
they come from a place. It also drives MIDI playback for dynamic or interactive
music and instrument notes, including sustained notes you release later.

Clips themselves are described by `RuntimeAudio` (a `res://` source plus volume,
looping, and effects); this module is the runtime side that actually triggers
and stops them on nodes, optionally through a named mixer bus.

## Use Cases

- Positional gunshot or footstep: attach the clip to the emitting node with `audio_play_attached!(ctx.run, sound, muzzle_node, options)` so it spatializes correctly.
- Mix through a bus for a volume slider or ducking: route with the bus form `audio_play_attached!(ctx.run, sfx_bus, sound, node, options)`.
- Stop a looping sound on demand (engine idle, alarm, charging hum): `ctx.run.Audio().stop_attached(node, source)`.
- Interactive / procedural music: play a MIDI song attached to a node with `midi_play_attached!(ctx.run, song, node, spatial)`.
- Sustained instrument notes with a real release (held organ chord, charge-up): `midi_start_attached!` returns a handle you later end with `midi_release_attached!`.
- Debug why a sound is occluded: toggle propagation ray visualization with `ctx.run.Audio().set_debug_rays(true)`.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Audio()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

Fire a spatialized gunshot from the weapon muzzle when a `shoot` method runs.
The sound plays from the muzzle node, so it pans and fades with the camera.

```rust
methods!({
    fn shoot(&self, ctx: &mut ScriptContext<'_, API>, muzzle: NodeID) {
        let sound = RuntimeAudio::new("res://audio/gunshot.ogg");
        let options = SpatialAudioOptions {
            range: 25.0,
            audio_layer: BitMask::ALL,
            enable_propagation: true,
            direction_2d: AudioDirection::Omni,
            direction_3d: AudioDirection::Omni,
        };
        audio_play_attached!(ctx.run, sound, muzzle, options);
    }
});
```

## API Reference

### `set_debug_rays`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn set_debug_rays(&mut self, enabled: bool)` |
| Params | `&mut self, enabled: bool` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `debug_rays_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn debug_rays_enabled(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn play_attached( &mut self, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `play_attached_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn play_attached_bus( &mut self, bus_id: AudioBusID, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, bus_id: AudioBusID, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn stop_attached(&mut self, node: NodeID, source: &str) -> bool` |
| Params | `&mut self, node: NodeID, source: &str` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `midi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn midi(&mut self) -> RuntimeMidiModule<'_, RT>` |
| Params | `&mut self` |
| Returns | `RuntimeMidiModule<'_, RT>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `play_note_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn play_note_attached( &mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `start_note_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn start_note_attached( &mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions, ) -> Option<MidiNoteHandle>` |
| Params | `&mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions,` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `play_file_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn play_file_attached( &mut self, song: MidiSong, node: NodeID, spatial: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, song: MidiSong, node: NodeID, spatial: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `release_note`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn release_note(&mut self, handle: MidiNoteHandle) -> bool` |
| Params | `&mut self, handle: MidiNoteHandle` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn stop_attached<T: Into<AttachedMidiTarget<'rt>>>( &mut self, node: NodeID, target: T, ) -> bool` |
| Params | `&mut self, node: NodeID, target: T,` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `audio_play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `audio_play_attached!(ctx.run, bus_id, audio, node, options)` (bus form) or `audio_play_attached!(ctx.run, audio, node, options)` |
| Params | `ctx.run, [bus_id,] audio, node, options` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `midi_play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_play_attached!(ctx.run, note, node, options, spatial)` (note form) or `midi_play_attached!(ctx.run, song, node, spatial)` (song form) |
| Params | `ctx.run, note, node, options, spatial` / `ctx.run, song, node, spatial` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `midi_start_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_start_attached!(ctx.run, note, node, options, spatial)` |
| Params | `ctx.run, note, node, options, spatial` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `midi_release_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_release_attached!(ctx.run, handle)` |
| Params | `ctx.run, handle` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `midi_stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_stop_attached!(ctx.run, node, target)` |
| Params | `ctx.run, node, target` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

