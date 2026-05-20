# Audio Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

## Overview

This runtime module belongs to `ctx.run` and documents audio calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Audio()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `set_debug_rays`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn set_debug_rays(&mut self, enabled: bool)` |
| Params | `&mut self, enabled: bool` |
| Returns | `()` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().set_debug_rays(true);
        let _ = value;
    }
});
```

### `debug_rays_enabled`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn debug_rays_enabled(&mut self) -> bool` |
| Params | `&mut self` |
| Returns | `bool` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().debug_rays_enabled();
        let _ = value;
    }
});
```

### `play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn play_attached( &mut self, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().play_attached(Default::default(), ctx.id, 0.1);
        let _ = value;
    }
});
```

### `play_attached_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn play_attached_bus( &mut self, bus_id: AudioBusID, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, bus_id: AudioBusID, audio: RuntimeAudio<'_>, node: NodeID, options: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().play_attached_bus(ctx.id, Default::default(), ctx.id, 0.1);
        let _ = value;
    }
});
```

### `stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn stop_attached(&mut self, node: NodeID, source: &str) -> bool` |
| Params | `&mut self, node: NodeID, source: &str` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().stop_attached(ctx.id, "name");
        let _ = value;
    }
});
```

### `midi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `pub fn midi(&mut self) -> RuntimeMidiModule<'_, RT>` |
| Params | `&mut self` |
| Returns | `RuntimeMidiModule<'_, RT>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi();
        let _ = value;
    }
});
```

### `play_note_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn play_note_attached( &mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi().play_note_attached(Default::default(), ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `start_note_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn start_note_attached( &mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions, ) -> Option<MidiNoteHandle>` |
| Params | `&mut self, note: Note, node: NodeID, options: MidiNoteOptions, spatial: SpatialAudioOptions,` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi().start_note_attached(Default::default(), ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `play_file_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn play_file_attached( &mut self, song: MidiSong, node: NodeID, spatial: SpatialAudioOptions, ) -> bool` |
| Params | `&mut self, song: MidiSong, node: NodeID, spatial: SpatialAudioOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi().play_file_attached(Default::default(), ctx.id, 0.1);
        let _ = value;
    }
});
```

### `release_note`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn release_note(&mut self, handle: MidiNoteHandle) -> bool` |
| Params | `&mut self, handle: MidiNoteHandle` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi().release_note(0.1);
        let _ = value;
    }
});
```

### `stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio().midi()` |
| Signature | `pub fn stop_attached<T: Into<AttachedMidiTarget<'rt>>>( &mut self, node: NodeID, target: T, ) -> bool` |
| Params | `&mut self, node: NodeID, target: T,` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.run.Audio().midi().stop_attached(ctx.id, ctx.id);
        let _ = value;
    }
});
```

### `audio_play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `audio_play_attached!(ctx.run.run, bus_id, audio, node, options)` |
| Params | `ctx.run, bus_id, audio, node, options` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_play_attached!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `midi_play_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_play_attached!(ctx.run.run, note, node, options, spatial)` |
| Params | `ctx.run, note, node, options, spatial` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_play_attached!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `midi_start_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_start_attached!(ctx.run.run, note, node, options, spatial)` |
| Params | `ctx.run, note, node, options, spatial` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_start_attached!(ctx.run, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `midi_release_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_release_attached!(ctx.run.run, handle)` |
| Params | `ctx.run, handle` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_release_attached!(ctx.run, 0.1);
        let _ = value;
    }
});
```

### `midi_stop_attached`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Audio()` |
| Signature | `midi_stop_attached!(ctx.run.run, node, target)` |
| Params | `ctx.run, node, target` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_stop_attached!(ctx.run, 0.0, 0.1);
        let _ = value;
    }
});
```
