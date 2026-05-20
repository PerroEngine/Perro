# Audio Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `load_source` | [`load_source`](#load_source) |
| `reserve_source` | [`reserve_source`](#reserve_source) |
| `is_loaded` | [`is_loaded`](#is_loaded) |
| `drop_source` | [`drop_source`](#drop_source) |
| `play` | [`play`](#play) |
| `play_bus` | [`play_bus`](#play_bus) |
| `play_master` | [`play_master`](#play_master) |
| `play_master_audio` | [`play_master_audio`](#play_master_audio) |
| `play_panned` | [`play_panned`](#play_panned) |
| `play_master_panned` | [`play_master_panned`](#play_master_panned) |
| `two_d` | [`two_d`](#two_d) |
| `three_d` | [`three_d`](#three_d) |
| `midi` | [`midi`](#midi) |
| `stop_audio` | [`stop_audio`](#stop_audio) |
| `stop_master_audio` | [`stop_master_audio`](#stop_master_audio) |
| `stop_source` | [`stop_source`](#stop_source) |
| `source_length_seconds` | [`source_length_seconds`](#source_length_seconds) |
| `source_length_millis` | [`source_length_millis`](#source_length_millis) |
| `stop_all` | [`stop_all`](#stop_all) |
| `set_master_volume` | [`set_master_volume`](#set_master_volume) |
| `set_bus_volume` | [`set_bus_volume`](#set_bus_volume) |
| `set_bus_speed` | [`set_bus_speed`](#set_bus_speed) |
| `pause_bus` | [`pause_bus`](#pause_bus) |
| `resume_bus` | [`resume_bus`](#resume_bus) |
| `stop_bus` | [`stop_bus`](#stop_bus) |
| `play` | [`play`](#play) |
| `play_master` | [`play_master`](#play_master) |
| `load_soundfont` | [`load_soundfont`](#load_soundfont) |
| `load_soundfont_hashed` | [`load_soundfont_hashed`](#load_soundfont_hashed) |
| `load_soundfont_hashed_with_source` | [`load_soundfont_hashed_with_source`](#load_soundfont_hashed_with_source) |
| `is_soundfont_loaded` | [`is_soundfont_loaded`](#is_soundfont_loaded) |
| `play_note` | [`play_note`](#play_note) |
| `play_note_bus` | [`play_note_bus`](#play_note_bus) |
| `start_note` | [`start_note`](#start_note) |
| `start_note_bus` | [`start_note_bus`](#start_note_bus) |
| `release_note` | [`release_note`](#release_note) |
| `play_file` | [`play_file`](#play_file) |
| `play_note_at` | [`play_note_at`](#play_note_at) |
| `start_note_at` | [`start_note_at`](#start_note_at) |
| `play_file_at` | [`play_file_at`](#play_file_at) |
| `play` | [`play`](#play) |
| `play_master` | [`play_master`](#play_master) |
| `audio_load` | [`audio_load`](#audio_load) |
| `audio_is_loaded` | [`audio_is_loaded`](#audio_is_loaded) |
| `audio_reserve` | [`audio_reserve`](#audio_reserve) |
| `audio_drop` | [`audio_drop`](#audio_drop) |
| `audio_play` | [`audio_play`](#audio_play) |
| `audio_stop` | [`audio_stop`](#audio_stop) |
| `audio_stop_source` | [`audio_stop_source`](#audio_stop_source) |
| `audio_length_seconds` | [`audio_length_seconds`](#audio_length_seconds) |
| `audio_length_millis` | [`audio_length_millis`](#audio_length_millis) |
| `audio_stop_all` | [`audio_stop_all`](#audio_stop_all) |
| `audio_set_master_volume` | [`audio_set_master_volume`](#audio_set_master_volume) |
| `audio_bus_set_volume` | [`audio_bus_set_volume`](#audio_bus_set_volume) |
| `audio_bus_set_speed` | [`audio_bus_set_speed`](#audio_bus_set_speed) |
| `audio_bus_pause` | [`audio_bus_pause`](#audio_bus_pause) |
| `audio_bus_resume` | [`audio_bus_resume`](#audio_bus_resume) |
| `audio_bus_stop` | [`audio_bus_stop`](#audio_bus_stop) |
| `audio_bus` | [`audio_bus`](#audio_bus) |
| `midi_load_soundfont` | [`midi_load_soundfont`](#midi_load_soundfont) |
| `midi_soundfont_is_loaded` | [`midi_soundfont_is_loaded`](#midi_soundfont_is_loaded) |
| `midi_play` | [`midi_play`](#midi_play) |
| `midi_start` | [`midi_start`](#midi_start) |
| `midi_release` | [`midi_release`](#midi_release) |
| `midi_play_at` | [`midi_play_at`](#midi_play_at) |
| `midi_start_at` | [`midi_start_at`](#midi_start_at) |

## Overview

This resource module belongs to `ctx.res` and documents audio calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Audio()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `load_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn load_source<S: ResPathSource>(&self, source: S) -> bool` |
| Params | `&self, source: S` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().load_source("res://path/to/resource");
        let _ = value;
    }
});
```

### `reserve_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn reserve_source<S: ResPathSource>(&self, source: S) -> bool` |
| Params | `&self, source: S` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().reserve_source("res://path/to/resource");
        let _ = value;
    }
});
```

### `is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn is_loaded<S: ResPathSource>(&self, source: S) -> bool` |
| Params | `&self, source: S` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().is_loaded("res://path/to/resource");
        let _ = value;
    }
});
```

### `drop_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn drop_source<S: ResPathSource>(&self, source: S) -> bool` |
| Params | `&self, source: S` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().drop_source("res://path/to/resource");
        let _ = value;
    }
});
```

### `play`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool` |
| Params | `&self, bus_id: AudioBusID, audio: Audio<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `play_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play_bus<C>(&self, bus_id: AudioBusID, audio: C) -> bool where C: AudioPlayConfig<R>,` |
| Params | `&self, bus_id: AudioBusID, audio: C` |
| Returns | `bool where C: AudioPlayConfig<R>,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play_bus(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `play_master`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play_master<C>(&self, audio: C) -> bool where C: AudioPlayConfig<R>,` |
| Params | `&self, audio: C` |
| Returns | `bool where C: AudioPlayConfig<R>,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play_master(0.1);
        let _ = value;
    }
});
```

### `play_master_audio`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play_master_audio(&self, audio: Audio<'_>) -> bool` |
| Params | `&self, audio: Audio<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play_master_audio(0.1);
        let _ = value;
    }
});
```

### `play_panned`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play_panned(&self, bus_id: AudioBusID, audio: Audio<'_>, pan: AudioPan) -> bool` |
| Params | `&self, bus_id: AudioBusID, audio: Audio<'_>, pan: AudioPan` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play_panned(ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `play_master_panned`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn play_master_panned(&self, audio: Audio<'_>, pan: AudioPan) -> bool` |
| Params | `&self, audio: Audio<'_>, pan: AudioPan` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().play_master_panned(0.0, 0.1);
        let _ = value;
    }
});
```

### `two_d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn two_d(&self) -> Audio2DModule<'res, R>` |
| Params | `&self` |
| Returns | `Audio2DModule<'res, R>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().two_d();
        let _ = value;
    }
});
```

### `three_d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn three_d(&self) -> Audio3DModule<'res, R>` |
| Params | `&self` |
| Returns | `Audio3DModule<'res, R>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().three_d();
        let _ = value;
    }
});
```

### `midi`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn midi(&self) -> MidiModule<'res, R>` |
| Params | `&self` |
| Returns | `MidiModule<'res, R>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi();
        let _ = value;
    }
});
```

### `stop_audio`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn stop_audio(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool` |
| Params | `&self, bus_id: AudioBusID, audio: Audio<'_>` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().stop_audio(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `stop_master_audio`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn stop_master_audio(&self, audio: Audio<'_>) -> bool` |
| Params | `&self, audio: Audio<'_>` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().stop_master_audio(0.1);
        let _ = value;
    }
});
```

### `stop_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn stop_source<S: ResPathSource>(&self, source: S) -> bool` |
| Params | `&self, source: S` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().stop_source("res://path/to/resource");
        let _ = value;
    }
});
```

### `source_length_seconds`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn source_length_seconds<S: ResPathSource>(&self, source: S) -> Option<f32>` |
| Params | `&self, source: S` |
| Returns | `Option<f32>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().source_length_seconds("res://path/to/resource");
        let _ = value;
    }
});
```

### `source_length_millis`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn source_length_millis<S: ResPathSource>(&self, source: S) -> Option<u64>` |
| Params | `&self, source: S` |
| Returns | `Option<u64>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().source_length_millis("res://path/to/resource");
        let _ = value;
    }
});
```

### `stop_all`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn stop_all(&self)` |
| Params | `&self` |
| Returns | `()` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().stop_all();
        let _ = value;
    }
});
```

### `set_master_volume`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn set_master_volume(&self, volume: f32) -> bool` |
| Params | `&self, volume: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().set_master_volume(1.0);
        let _ = value;
    }
});
```

### `set_bus_volume`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool` |
| Params | `&self, bus_id: AudioBusID, volume: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().set_bus_volume(ctx.id, 1.0);
        let _ = value;
    }
});
```

### `set_bus_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool` |
| Params | `&self, bus_id: AudioBusID, speed: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().set_bus_speed(ctx.id, 1.0);
        let _ = value;
    }
});
```

### `pause_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn pause_bus(&self, bus_id: AudioBusID) -> bool` |
| Params | `&self, bus_id: AudioBusID` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().pause_bus(ctx.id);
        let _ = value;
    }
});
```

### `resume_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn resume_bus(&self, bus_id: AudioBusID) -> bool` |
| Params | `&self, bus_id: AudioBusID` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().resume_bus(ctx.id);
        let _ = value;
    }
});
```

### `stop_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `pub fn stop_bus(&self, bus_id: AudioBusID) -> bool` |
| Params | `&self, bus_id: AudioBusID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().stop_bus(ctx.id);
        let _ = value;
    }
});
```

### `play`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().two_d()` |
| Signature | `pub fn play(&self, bus_id: AudioBusID, audio: Audio2D<'_>) -> bool` |
| Params | `&self, bus_id: AudioBusID, audio: Audio2D<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().two_d().play(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `play_master`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().two_d()` |
| Signature | `pub fn play_master(&self, audio: Audio2D<'_>) -> bool` |
| Params | `&self, audio: Audio2D<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().two_d().play_master(0.1);
        let _ = value;
    }
});
```

### `load_soundfont`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn load_soundfont<S: ResPathSource>(&self, source: S) -> SoundFontID` |
| Params | `&self, source: S` |
| Returns | `SoundFontID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().load_soundfont("res://path/to/resource");
        let _ = value;
    }
});
```

### `load_soundfont_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn load_soundfont_hashed(&self, source_hash: u64) -> SoundFontID` |
| Params | `&self, source_hash: u64` |
| Returns | `SoundFontID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().load_soundfont_hashed(0);
        let _ = value;
    }
});
```

### `load_soundfont_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn load_soundfont_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> SoundFontID` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `SoundFontID` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().load_soundfont_hashed_with_source(0, "res://path/to/resource");
        let _ = value;
    }
});
```

### `is_soundfont_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn is_soundfont_loaded(&self, id: SoundFontID) -> bool` |
| Params | `&self, id: SoundFontID` |
| Returns | `bool` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().is_soundfont_loaded(0.1);
        let _ = value;
    }
});
```

### `play_note`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn play_note(&self, note: Note, options: MidiNoteOptions) -> bool` |
| Params | `&self, note: Note, options: MidiNoteOptions` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().play_note(0.0, 0.1);
        let _ = value;
    }
});
```

### `play_note_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn play_note_bus( &self, bus_id: AudioBusID, note: Note, mut options: MidiNoteOptions, ) -> bool` |
| Params | `&self, bus_id: AudioBusID, note: Note, mut options: MidiNoteOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().play_note_bus(ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `start_note`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn start_note(&self, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle>` |
| Params | `&self, note: Note, options: MidiNoteOptions` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().start_note(0.0, 0.1);
        let _ = value;
    }
});
```

### `start_note_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn start_note_bus( &self, bus_id: AudioBusID, note: Note, mut options: MidiNoteOptions, ) -> Option<MidiNoteHandle>` |
| Params | `&self, bus_id: AudioBusID, note: Note, mut options: MidiNoteOptions,` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().start_note_bus(ctx.id, 0.0, 0.1);
        let _ = value;
    }
});
```

### `release_note`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn release_note(&self, handle: MidiNoteHandle) -> bool` |
| Params | `&self, handle: MidiNoteHandle` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().release_note(0.1);
        let _ = value;
    }
});
```

### `play_file`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn play_file(&self, song: MidiSong) -> bool` |
| Params | `&self, song: MidiSong` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().play_file(0.1);
        let _ = value;
    }
});
```

### `play_note_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn play_note_at<P: MidiSpatialPos>( &self, note: Note, position: P, range: f32, options: MidiNoteOptions, ) -> bool` |
| Params | `&self, note: Note, position: P, range: f32, options: MidiNoteOptions,` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().play_note_at(0.0, 0.1, 1.0, 0.1);
        let _ = value;
    }
});
```

### `start_note_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn start_note_at<P: MidiSpatialPos>( &self, note: Note, position: P, range: f32, options: MidiNoteOptions, ) -> Option<MidiNoteHandle>` |
| Params | `&self, note: Note, position: P, range: f32, options: MidiNoteOptions,` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().start_note_at(0.0, 0.1, 1.0, 0.1);
        let _ = value;
    }
});
```

### `play_file_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().midi()` |
| Signature | `pub fn play_file_at<P: MidiSpatialPos>(&self, song: MidiSong, position: P, range: f32) -> bool` |
| Params | `&self, song: MidiSong, position: P, range: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().midi().play_file_at(0.0, 0.1, 1.0);
        let _ = value;
    }
});
```

### `play`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().three_d()` |
| Signature | `pub fn play(&self, bus_id: AudioBusID, audio: Audio3D<'_>) -> bool` |
| Params | `&self, bus_id: AudioBusID, audio: Audio3D<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().three_d().play(ctx.id, 0.1);
        let _ = value;
    }
});
```

### `play_master`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio().three_d()` |
| Signature | `pub fn play_master(&self, audio: Audio3D<'_>) -> bool` |
| Params | `&self, audio: Audio3D<'_>` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = ctx.res.Audio().three_d().play_master(0.1);
        let _ = value;
    }
});
```

### `audio_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_load!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_is_loaded!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_is_loaded!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_reserve`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_reserve!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_reserve!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_drop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_drop!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_drop!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_play`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_play!(ctx.res.res, bus_id, audio)` |
| Params | `ctx.res, bus_id, audio` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_play!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `audio_stop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_stop!(ctx.res.res, bus_id, audio)` |
| Params | `ctx.res, bus_id, audio` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_stop!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `audio_stop_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_stop_source!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_stop_source!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_length_seconds`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_length_seconds!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_length_seconds!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_length_millis`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_length_millis!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_length_millis!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_stop_all`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_stop_all!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_stop_all!(ctx.res);
        let _ = value;
    }
});
```

### `audio_set_master_volume`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_set_master_volume!(ctx.res.res, volume)` |
| Params | `ctx.res, volume` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_set_master_volume!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_bus_set_volume`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus_set_volume!(ctx.res.res, bus_id, volume)` |
| Params | `ctx.res, bus_id, volume` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus_set_volume!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `audio_bus_set_speed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus_set_speed!(ctx.res.res, bus_id, speed)` |
| Params | `ctx.res, bus_id, speed` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus_set_speed!(ctx.res, 0.0, 0.1);
        let _ = value;
    }
});
```

### `audio_bus_pause`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus_pause!(ctx.res.res, bus_id)` |
| Params | `ctx.res, bus_id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus_pause!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_bus_resume`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus_resume!(ctx.res.res, bus_id)` |
| Params | `ctx.res, bus_id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus_resume!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_bus_stop`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus_stop!(ctx.res.res, bus_id)` |
| Params | `ctx.res, bus_id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus_stop!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `audio_bus`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `audio_bus!(name)` |
| Params | `name` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = audio_bus!(ctx.res);
        let _ = value;
    }
});
```

### `midi_load_soundfont`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_load_soundfont!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_load_soundfont!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `midi_soundfont_is_loaded`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_soundfont_is_loaded!(ctx.res.res, id)` |
| Params | `ctx.res, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_soundfont_is_loaded!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `midi_play`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_play!(ctx.res.res, bus_id, note, options)` |
| Params | `ctx.res, bus_id, note, options` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_play!(ctx.res, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `midi_start`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_start!(ctx.res.res, bus_id, note, options)` |
| Params | `ctx.res, bus_id, note, options` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_start!(ctx.res, 0.0, 0.1, 0.1);
        let _ = value;
    }
});
```

### `midi_release`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_release!(ctx.res.res, handle)` |
| Params | `ctx.res, handle` |
| Returns | `same as backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_release!(ctx.res, 0.1);
        let _ = value;
    }
});
```

### `midi_play_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_play_at!(ctx.res.res, note, pos, range, options)` |
| Params | `ctx.res, note, pos, range, options` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_play_at!(ctx.res, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```

### `midi_start_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Audio()` |
| Signature | `midi_start_at!(ctx.res.res, note, pos, range, options)` |
| Params | `ctx.res, note, pos, range, options` |
| Returns | `same as backing method` |
| Use when | Use when this exact typed operation matches the system state the script needs to read or change. |
| Fails when / edge behavior | `Option` returns `None` for missing data. `Result` returns source error details. `bool` returns `false` when the operation cannot apply. ID-based calls fail when the ID is stale or wrong for the requested type. |

Example:

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let value = midi_start_at!(ctx.res, 0.0, 0.1, 0.0, 0.1);
        let _ = value;
    }
});
```
