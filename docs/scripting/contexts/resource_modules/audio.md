# Audio Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Buses | [Buses](#buses) |
| Playback Types | [Playback Types](#playback-types) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference: Audio | [API Reference: Audio](#api-reference-audio) |
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
| `play_clip` | [`play_clip`](#play_clip) |
| `play_clip_bus` | [`play_clip_bus`](#play_clip_bus) |
| `play_clip_bus_volume` | [`play_clip_bus_volume`](#play_clip_bus_volume) |
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
| Positional (`two_d` / `three_d`) | [Positional Audio](#positional-audio) |
| API Reference: MIDI | [API Reference: MIDI](#api-reference-midi) |
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
| API Reference: Macros | [API Reference: Macros](#api-reference-macros) |

## Purpose

`ctx.res.Audio()` plays sound: music, one-shot effects, positional 2D/3D audio, captured mic clips, and MIDI. Sources load from a path or in-memory bytes, and playback routes through named buses so a script can control music and SFX volumes separately. Positional playback attaches a sound to a world position with a falloff range; MIDI plays notes and songs through a loaded soundfont.

## Use Cases

- Background music with a volume slider: `audio_play!` a looped track on a `"music"` bus, then `audio_bus_set_volume!` from the options menu.
- One-shot SFX: `audio_play!(ctx.res, bus, Audio::new("res://sfx/click.wav"))` for UI and gameplay hits.
- Positional world audio: `Audio3D::new(source, position, range)` played through `three_d()` so a distant sound is quieter and panned; use `Audio2D` for top-down games.
- Ducking and pausing: `pause_bus!` / `resume_bus!` a music bus during dialogue, or `set_bus_speed!` for a slow-motion effect.
- Voice chat playback: `audio_play_clip!` a `MicClip` decoded from the mic module through a `"voice"` bus.
- Dynamic/interactive music: play MIDI notes with `midi_play!` and a loaded soundfont, or spatialize a stinger with `midi_play_at!`.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Audio()`; positional sub-modules via `.two_d()` / `.three_d()`; MIDI via `.midi()`.
- Buses are identified by `AudioBusID`; build one at compile time with `audio_bus!("name")`.
- A missing `bus_id` (the `play_master*` variants) routes to the master output.
- Types: `Audio`, `Audio2D`, `Audio3D`, `AudioPan`, `MicClip`, `Note`, `MidiNoteOptions`, `MidiSong`, `SoundFontID`.
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Buses

A bus is a named mixer channel. Route related sounds (music, sfx, voice) to their own bus so each can be muted, ducked, or slowed independently:

```rust
let music = audio_bus!("music"); // AudioBusID, resolved at compile time
audio_bus_set_volume!(ctx.res, music, 0.5);
```

The `play_master*` methods and the single-argument `audio_play!` form skip the bus and mix straight to master.

## Playback Types

| Type | Build with | Purpose |
| --- | --- | --- |
| `Audio` | `Audio::new(source)`, `.with_speed(s)`, `.with_effects(e)` | A non-positional sound (music, UI, one-shots). |
| `Audio2D` | `Audio2D::new(source, position, range)` | A sound at a 2D world position with falloff. |
| `Audio3D` | `Audio3D::new(source, position, range)` | A sound at a 3D world position with falloff. |
| `AudioPan` | `AudioPan::new(x, y, z)`, `AudioPan::CENTER` | Manual stereo/spatial pan for `play_panned`. |
| `MicClip` | from `ctx.res.Mic()` or `mic_unpack!` | Captured PCM audio played with `play_clip*`. |

## Runtime Bytes

Use runtime bytes when audio data is already in memory. Use `MicClip` when data already came from `ctx.res.Mic()`.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Audio().create_source_from_bytes(bytes)` | `Option<String>` | Returns a runtime source string for normal playback calls. |
| `ctx.res.Audio().play_clip(&clip)` | `bool` | Plays a mic/captured clip through the audio backend. |
| `ctx.res.Audio().midi().load_soundfont_from_bytes(bytes)` | `SoundFontID` | Loads in-memory `.sf2` bytes. |
| `audio_create_from_bytes!(ctx.res, bytes)` | `Option<String>` | Macro form. |
| `audio_play!(ctx.res, &clip)` | `bool` | Macro form for master clip playback. |
| `audio_play_clip!(ctx.res, bus, &clip, volume)` | `bool` | Macro form for bus + volume clip playback. |
| `midi_load_soundfont_from_bytes!(ctx.res, bytes)` | `SoundFontID` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## Practical Example

Start looping music at init on a named bus, and play a hit sound from a `methods!` helper.

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let music = audio_bus!("music");
        let track = Audio { looped: true, ..Audio::new("res://music/theme.ogg") };
        audio_play!(ctx.res, music, track);
        audio_bus_set_volume!(ctx.res, music, 0.6);
    }
});

methods!({
    fn play_hit(&self, ctx: &mut ScriptContext<'_, API>) {
        let sfx = audio_bus!("sfx");
        audio_play!(ctx.res, sfx, Audio::new("res://sfx/hit.wav"));
    }
});
```

## API Reference: Audio

Access on `ctx.res.Audio()`. Playback calls return `bool` (`false` when the audio backend is unavailable or the source cannot be found); a missing `bus_id` routes to master.

### `load_source`

| Field | Detail |
| --- | --- |
| Signature | `pub fn load_source<S: ResPathSource>(&self, source: S) -> bool` |
| Returns | `bool` |
| Use when | Preloading a source so the first `play` does not hitch. |
| Fails when / edge behavior | Returns `false` when the source cannot be loaded. |

### `reserve_source`

| Field | Detail |
| --- | --- |
| Signature | `pub fn reserve_source<S: ResPathSource>(&self, source: S) -> bool` |
| Returns | `bool` |
| Use when | Pinning a source so it stays resident. |
| Fails when / edge behavior | Returns `false` when the source cannot be reserved. |

### `is_loaded`

| Field | Detail |
| --- | --- |
| Signature | `pub fn is_loaded<S: ResPathSource>(&self, source: S) -> bool` |
| Returns | `bool` |
| Use when | Checking whether a source finished loading. |
| Fails when / edge behavior | Returns `false` while loading or when the source is unknown. |

### `drop_source`

| Field | Detail |
| --- | --- |
| Signature | `pub fn drop_source<S: ResPathSource>(&self, source: S) -> bool` |
| Returns | `bool` |
| Use when | Releasing a source the game no longer plays. |
| Fails when / edge behavior | Returns `false` when the source is unknown. |

### `play`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool` |
| Returns | `bool` |
| Use when | Playing a non-positional sound on a specific bus. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_bus<C: AudioPlayConfig<R>>(&self, bus_id: AudioBusID, audio: C) -> bool` |
| Returns | `bool` |
| Use when | Playing any playable (`Audio`, `Audio2D`, `Audio3D`, `AudioClip`, `&MicClip`) on a bus; the `audio_play!` macro uses this. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_master`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_master<C: AudioPlayConfig<R>>(&self, audio: C) -> bool` |
| Returns | `bool` |
| Use when | Playing any playable straight to master output. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_master_audio`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_master_audio(&self, audio: Audio<'_>) -> bool` |
| Returns | `bool` |
| Use when | Playing an `Audio` on master output. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_panned`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_panned(&self, bus_id: AudioBusID, audio: Audio<'_>, pan: AudioPan) -> bool` |
| Returns | `bool` |
| Use when | Playing a sound on a bus with a manual stereo/spatial pan. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_master_panned`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_master_panned(&self, audio: Audio<'_>, pan: AudioPan) -> bool` |
| Returns | `bool` |
| Use when | Playing a manually panned sound on master output. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_clip`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_clip(&self, clip: &MicClip) -> bool` |
| Returns | `bool` |
| Use when | Playing captured mic audio on master output. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_clip_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_clip_bus(&self, bus_id: AudioBusID, clip: &MicClip) -> bool` |
| Returns | `bool` |
| Use when | Playing captured mic audio through a bus, for example a voice channel. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `play_clip_bus_volume`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_clip_bus_volume(&self, bus_id: AudioBusID, clip: &MicClip, volume: f32) -> bool` |
| Returns | `bool` |
| Use when | Playing captured mic audio through a bus at a set volume. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `two_d`

| Field | Detail |
| --- | --- |
| Signature | `pub fn two_d(&self) -> Audio2DModule<'res, R>` |
| Returns | `Audio2DModule` |
| Use when | Accessing 2D positional playback. See [Positional Audio](#positional-audio). |

### `three_d`

| Field | Detail |
| --- | --- |
| Signature | `pub fn three_d(&self) -> Audio3DModule<'res, R>` |
| Returns | `Audio3DModule` |
| Use when | Accessing 3D positional playback. See [Positional Audio](#positional-audio). |

### `midi`

| Field | Detail |
| --- | --- |
| Signature | `pub fn midi(&self) -> MidiModule<'res, R>` |
| Returns | `MidiModule` |
| Use when | Accessing soundfont/MIDI playback. See [API Reference: MIDI](#api-reference-midi). |

### `stop_audio`

| Field | Detail |
| --- | --- |
| Signature | `pub fn stop_audio(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool` |
| Returns | `bool` |
| Use when | Stopping a specific sound on a bus. |
| Fails when / edge behavior | Returns `false` when no matching voice is playing. |

### `stop_master_audio`

| Field | Detail |
| --- | --- |
| Signature | `pub fn stop_master_audio(&self, audio: Audio<'_>) -> bool` |
| Returns | `bool` |
| Use when | Stopping a specific sound on master output. |
| Fails when / edge behavior | Returns `false` when no matching voice is playing. |

### `stop_source`

| Field | Detail |
| --- | --- |
| Signature | `pub fn stop_source<S: ResPathSource>(&self, source: S) -> bool` |
| Returns | `bool` |
| Use when | Stopping every voice playing a given source. |
| Fails when / edge behavior | Returns `false` when nothing is playing that source. |

### `source_length_seconds`

| Field | Detail |
| --- | --- |
| Signature | `pub fn source_length_seconds<S: ResPathSource>(&self, source: S) -> Option<f32>` |
| Returns | `Option<f32>` |
| Use when | Reading a clip's duration, for example to schedule the next cue. |
| Fails when / edge behavior | Returns `None` when the source length is unknown. |

### `source_length_millis`

| Field | Detail |
| --- | --- |
| Signature | `pub fn source_length_millis<S: ResPathSource>(&self, source: S) -> Option<u64>` |
| Returns | `Option<u64>` |
| Use when | Reading a clip's duration in milliseconds. |
| Fails when / edge behavior | Returns `None` when the source length is unknown. |

### `stop_all`

| Field | Detail |
| --- | --- |
| Signature | `pub fn stop_all(&self)` |
| Returns | `()` |
| Use when | Silencing everything, for example on a scene change. |

### `set_master_volume`

| Field | Detail |
| --- | --- |
| Signature | `pub fn set_master_volume(&self, volume: f32) -> bool` |
| Returns | `bool` |
| Use when | Applying a master volume slider. |
| Fails when / edge behavior | Returns `false` when the backend is unavailable. |

### `set_bus_volume`

| Field | Detail |
| --- | --- |
| Signature | `pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool` |
| Returns | `bool` |
| Use when | Applying a per-channel volume (music, sfx, voice sliders). |
| Fails when / edge behavior | Returns `false` when the bus is unknown. |

### `set_bus_speed`

| Field | Detail |
| --- | --- |
| Signature | `pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool` |
| Returns | `bool` |
| Use when | Pitch/tempo shifting a bus, for example a slow-motion effect. |
| Fails when / edge behavior | Returns `false` when the bus is unknown. |

### `pause_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn pause_bus(&self, bus_id: AudioBusID) -> bool` |
| Returns | `bool` |
| Use when | Pausing a channel, for example ducking music during dialogue. |
| Fails when / edge behavior | Returns `false` when the bus is unknown. |

### `resume_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn resume_bus(&self, bus_id: AudioBusID) -> bool` |
| Returns | `bool` |
| Use when | Resuming a paused channel. |
| Fails when / edge behavior | Returns `false` when the bus is unknown. |

### `stop_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn stop_bus(&self, bus_id: AudioBusID) -> bool` |
| Returns | `bool` |
| Use when | Stopping every voice on a channel. |
| Fails when / edge behavior | Returns `false` when the bus is unknown. |

### Positional Audio

`ctx.res.Audio().two_d()` and `.three_d()` play sounds that attenuate and pan by world position.

| Access | Signature | Use when |
| --- | --- | --- |
| `two_d()` | `pub fn play(&self, bus_id: AudioBusID, audio: Audio2D<'_>) -> bool` | Positional sound on a bus in a 2D game. |
| `two_d()` | `pub fn play_master(&self, audio: Audio2D<'_>) -> bool` | Positional 2D sound on master. |
| `three_d()` | `pub fn play(&self, bus_id: AudioBusID, audio: Audio3D<'_>) -> bool` | Positional sound on a bus in a 3D game. |
| `three_d()` | `pub fn play_master(&self, audio: Audio3D<'_>) -> bool` | Positional 3D sound on master. |

All four return `bool` (`false` when the backend is unavailable). Build the payload with `Audio2D::new(source, position, range)` or `Audio3D::new(source, position, range)`.

## API Reference: MIDI

Access on `ctx.res.Audio().midi()`. Note playback needs a loaded soundfont.

### `load_soundfont`

| Field | Detail |
| --- | --- |
| Signature | `pub fn load_soundfont<S: ResPathSource>(&self, source: S) -> SoundFontID` |
| Returns | `SoundFontID` |
| Use when | Loading an `.sf2` soundfont by path. |
| Fails when / edge behavior | Returns a nil `SoundFontID` when the file is missing. |

### `load_soundfont_hashed`

| Field | Detail |
| --- | --- |
| Signature | `pub fn load_soundfont_hashed(&self, source_hash: u64) -> SoundFontID` |
| Returns | `SoundFontID` |
| Use when | A precomputed path hash is available. |
| Fails when / edge behavior | Returns a nil `SoundFontID` when no soundfont is registered for the hash. |

### `load_soundfont_hashed_with_source`

| Field | Detail |
| --- | --- |
| Signature | `pub fn load_soundfont_hashed_with_source<S: ResPathSource>(&self, source_hash: u64, source: S) -> SoundFontID` |
| Returns | `SoundFontID` |
| Use when | The `midi_load_soundfont!` literal path builds a compile-time hash and passes the source. |
| Fails when / edge behavior | Returns a nil `SoundFontID` when the file is missing. |

### `is_soundfont_loaded`

| Field | Detail |
| --- | --- |
| Signature | `pub fn is_soundfont_loaded(&self, id: SoundFontID) -> bool` |
| Returns | `bool` |
| Use when | Checking a soundfont finished loading before playing notes. |
| Fails when / edge behavior | Returns `false` while loading or when the ID is unknown. |

### `play_note`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_note(&self, note: Note, options: MidiNoteOptions) -> bool` |
| Returns | `bool` |
| Use when | Firing a one-shot note (fire-and-forget). |
| Fails when / edge behavior | Returns `false` when no soundfont is available. |

### `play_note_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_note_bus(&self, bus_id: AudioBusID, note: Note, options: MidiNoteOptions) -> bool` |
| Returns | `bool` |
| Use when | Firing a one-shot note routed to a bus. |
| Fails when / edge behavior | Returns `false` when no soundfont is available. |

### `start_note`

| Field | Detail |
| --- | --- |
| Signature | `pub fn start_note(&self, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle>` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Holding a note; keep the handle to release it later. |
| Fails when / edge behavior | Returns `None` when the note cannot start. |

### `start_note_bus`

| Field | Detail |
| --- | --- |
| Signature | `pub fn start_note_bus(&self, bus_id: AudioBusID, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle>` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Holding a note on a bus. |
| Fails when / edge behavior | Returns `None` when the note cannot start. |

### `release_note`

| Field | Detail |
| --- | --- |
| Signature | `pub fn release_note(&self, handle: MidiNoteHandle) -> bool` |
| Returns | `bool` |
| Use when | Releasing a held note started with `start_note*`. |
| Fails when / edge behavior | Returns `false` when the handle is unknown or already released. |

### `play_file`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_file(&self, song: MidiSong) -> bool` |
| Returns | `bool` |
| Use when | Playing a full MIDI song, for example `MidiSong::new("res://song.mid")`. |
| Fails when / edge behavior | Returns `false` when no soundfont is available or the song is missing. |

### `play_note_at`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_note_at<P: MidiSpatialPos>(&self, note: Note, position: P, range: f32, options: MidiNoteOptions) -> bool` |
| Returns | `bool` |
| Use when | Firing a one-shot note at a `Vector2` or `Vector3` world position. |
| Fails when / edge behavior | Returns `false` when no soundfont is available. |

### `start_note_at`

| Field | Detail |
| --- | --- |
| Signature | `pub fn start_note_at<P: MidiSpatialPos>(&self, note: Note, position: P, range: f32, options: MidiNoteOptions) -> Option<MidiNoteHandle>` |
| Returns | `Option<MidiNoteHandle>` |
| Use when | Holding a positional note; keep the handle to release it. |
| Fails when / edge behavior | Returns `None` when the note cannot start. |

### `play_file_at`

| Field | Detail |
| --- | --- |
| Signature | `pub fn play_file_at<P: MidiSpatialPos>(&self, song: MidiSong, position: P, range: f32) -> bool` |
| Returns | `bool` |
| Use when | Playing a MIDI song from a world position. |
| Fails when / edge behavior | Returns `false` when no soundfont is available or the song is missing. |

## API Reference: Macros

All audio macros take `ctx.res` as the first argument. Return types match the method each macro expands to.

| Macro | Expands to | Returns |
| --- | --- | --- |
| `audio_load!(ctx.res, source)` | `Audio().load_source(source)` | `bool` |
| `audio_is_loaded!(ctx.res, source)` | `Audio().is_loaded(source)` | `bool` |
| `audio_reserve!(ctx.res, source)` | `Audio().reserve_source(source)` | `bool` |
| `audio_drop!(ctx.res, source)` | `Audio().drop_source(source)` | `bool` |
| `audio_create_from_bytes!(ctx.res, bytes)` | `Audio().create_source_from_bytes(bytes)` | `Option<String>` |
| `audio_play!(ctx.res, bus_id, audio)` | `Audio().play_bus(bus_id, audio)` | `bool` |
| `audio_play!(ctx.res, audio)` | `Audio().play_master(audio)` | `bool` |
| `audio_play_clip!(ctx.res, clip)` | `Audio().play_clip(clip)` | `bool` |
| `audio_play_clip!(ctx.res, bus_id, clip)` | `Audio().play_clip_bus(bus_id, clip)` | `bool` |
| `audio_play_clip!(ctx.res, bus_id, clip, volume)` | `Audio().play_clip_bus_volume(bus_id, clip, volume)` | `bool` |
| `audio_play_stream_clip!(ctx.res, bus_id, stream_id, clip, volume)` | `Audio().play_stream_clip_bus_volume(...)` | `bool` |
| `audio_stop!(ctx.res, bus_id, audio)` | `Audio().stop_audio(bus_id, audio)` | `bool` |
| `audio_stop!(ctx.res, audio)` | `Audio().stop_master_audio(audio)` | `bool` |
| `audio_stop_source!(ctx.res, source)` | `Audio().stop_source(source)` | `bool` |
| `audio_length_seconds!(ctx.res, source)` | `Audio().source_length_seconds(source)` | `Option<f32>` |
| `audio_length_millis!(ctx.res, source)` | `Audio().source_length_millis(source)` | `Option<u64>` |
| `audio_stop_all!(ctx.res)` | `Audio().stop_all()` | `()` |
| `audio_set_master_volume!(ctx.res, volume)` | `Audio().set_master_volume(volume)` | `bool` |
| `audio_bus_set_volume!(ctx.res, bus_id, volume)` | `Audio().set_bus_volume(bus_id, volume)` | `bool` |
| `audio_bus_set_speed!(ctx.res, bus_id, speed)` | `Audio().set_bus_speed(bus_id, speed)` | `bool` |
| `audio_bus_pause!(ctx.res, bus_id)` | `Audio().pause_bus(bus_id)` | `bool` |
| `audio_bus_resume!(ctx.res, bus_id)` | `Audio().resume_bus(bus_id)` | `bool` |
| `audio_bus_stop!(ctx.res, bus_id)` | `Audio().stop_bus(bus_id)` | `bool` |
| `audio_bus!("name")` | compile-time `AudioBusID` | `AudioBusID` |
| `midi_load_soundfont!(ctx.res, source)` | `Audio().midi().load_soundfont(source)` | `SoundFontID` |
| `midi_load_soundfont_from_bytes!(ctx.res, bytes)` | `Audio().midi().load_soundfont_from_bytes(bytes)` | `SoundFontID` |
| `midi_soundfont_is_loaded!(ctx.res, id)` | `Audio().midi().is_soundfont_loaded(id)` | `bool` |
| `midi_play!(ctx.res, note, options)` | `Audio().midi().play_note(note, options)` | `bool` |
| `midi_play!(ctx.res, bus_id, note, options)` | `Audio().midi().play_note_bus(bus_id, note, options)` | `bool` |
| `midi_play!(ctx.res, song)` | `Audio().midi().play_file(song)` | `bool` |
| `midi_start!(ctx.res, note, options)` | `Audio().midi().start_note(note, options)` | `Option<MidiNoteHandle>` |
| `midi_start!(ctx.res, bus_id, note, options)` | `Audio().midi().start_note_bus(bus_id, note, options)` | `Option<MidiNoteHandle>` |
| `midi_release!(ctx.res, handle)` | `Audio().midi().release_note(handle)` | `bool` |
| `midi_play_at!(ctx.res, note, pos, range, options)` | `Audio().midi().play_note_at(...)` | `bool` |
| `midi_play_at!(ctx.res, song, pos, range)` | `Audio().midi().play_file_at(...)` | `bool` |
| `midi_start_at!(ctx.res, note, pos, range, options)` | `Audio().midi().start_note_at(...)` | `Option<MidiNoteHandle>` |
