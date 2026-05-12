# Audio Module

Access:

- `res.Audio()`

Shared backend, cache, bus, `.pawdio`, and propagation concepts:

- [Audio](../../../audio.md)

## Shared Macros

- `audio_bus!("name") -> AudioBusID`
- `audio_load!(res, source) -> bool`
- `audio_reserve!(res, source) -> bool`
- `audio_drop!(res, source) -> bool`
- `audio_stop_source!(res, source) -> bool`
- `audio_length_seconds!(res, source) -> Option<f32>`
- `audio_length_millis!(res, source) -> Option<u64>`
- `audio_stop_all!(res)`
- `audio_set_master_volume!(res, volume) -> bool`
- `audio_bus_set_volume!(res, bus_id, volume) -> bool`
- `audio_bus_set_speed!(res, bus_id, speed) -> bool`
- `audio_bus_pause!(res, bus_id) -> bool`
- `audio_bus_resume!(res, bus_id) -> bool`
- `audio_bus_stop!(res, bus_id) -> bool`

## Base Audio

Base audio uses explicit pan values.

Macros:

- `audio_play!(res, bus_id, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `audio_play!(res, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `audio_play!(res, bus_id, PannedAudio { audio, pan }) -> bool`
- `audio_play!(res, (audio, pan)) -> bool`
- `audio_stop!(res, bus_id, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `audio_stop!(res, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`

Type:

```rust
Audio {
    source: &str,      // literal or ResPath::as_str()
    looped: bool,
    volume: f32,      // 1.0 normal, 0.0 silent, >1 amplified
    effects: AudioEffects,
    from_start: f32,  // seconds trimmed from the start (>= 0.0)
    from_end: f32,    // seconds trimmed from the end (>= 0.0)
}

AudioEffects {
    speed: f32,        // 1.0 normal playback speed (also changes pitch)
    low_pass: f32,
    reverb_send: f32,
    echo: f32,
    reflection: f32,
    occlusion: f32,
    eq: AudioEq,                  // low_gain, mid_gain, high_gain
    compression: AudioCompression // threshold, ratio, attack, release
}

PannedAudio {
    audio: Audio,
    pan: AudioPan,    // x left/right, y down/up, z back/front
}
```

Methods:

- `res.Audio().play_bus(bus_id, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `res.Audio().play_master(Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `res.Audio().play_bus(bus_id, PannedAudio { audio, pan }) -> bool`
- `res.Audio().play_panned(bus_id, audio, pan) -> bool`
- `res.Audio().play_master_panned(audio, pan) -> bool`
- `res.Audio().stop_audio(bus_id, Audio { source, looped, volume, effects, from_start, from_end }) -> bool`
- `res.Audio().stop_master_audio(Audio { source, looped, volume, effects, from_start, from_end }) -> bool`

Rules:

- plain `Audio` plays centered.
- `PannedAudio.pan.x` is clamped to `-1.0..1.0` for left/right.
- `PannedAudio.pan.y` is clamped to `-1.0..1.0` for down/up.
- `PannedAudio.pan.z` is clamped to `-1.0..1.0` for back/front spatial flavor.

## Audio2D

Audio2D uses `Vector2` position and enters the runtime audio propagation solver.
The active `Camera2D` is the listener.

Macros:

- `audio_play!(res, bus_id, Audio2D::new(source, position, range)) -> bool`
- `audio_play!(res, Audio2D::new(source, position, range)) -> bool`

Type:

```rust
Audio2D {
    audio: Audio,
    position: Vector2,
    range: f32,
    audio_layer: BitMask,
    enable_propagation: bool,
    direction: Option<AudioDirection<Vector2>>,
}
```

Methods:

- `res.Audio().two_d().play(bus_id, audio_2d) -> bool`
- `res.Audio().two_d().play_master(audio_2d) -> bool`
- `res.Audio().play_bus(bus_id, audio_2d) -> bool`
- `res.Audio().play_master(audio_2d) -> bool`

Rules:

- `Audio2D.position` is a one-shot point emitter.
- `direction: None` means omni playback.
- `direction: Some(AudioDirection::Directional(forward))` sets direction.
- omni ignores direction.
- `AudioDirection::Directional(forward)` is loudest toward `forward`.
- `AudioDirection::InverseDirectional(forward)` is loudest opposite `forward`.
- `AudioDirection::Bidirectional(forward)` is loudest toward both `forward` and `-forward`.
- Audio2D has no manual pan; pan comes from the propagated perceived direction.
- Audio outside `range` or `[audio].listener_max_distance` is skipped.
- Direct volume fades linearly from full at camera to silent at `range`.
- Propagation uses listener, physics audio materials, masks, zones, and portals.
- Use Runtime Audio when the sound should be bound to a moving node.

## Audio3D

Audio3D uses `Vector3` position and enters the runtime audio propagation solver.
The active `Camera3D` is the listener.

Macros:

- `audio_play!(res, bus_id, Audio3D::new(source, position, range)) -> bool`
- `audio_play!(res, Audio3D::new(source, position, range)) -> bool`

Type:

```rust
Audio3D {
    audio: Audio,
    position: Vector3,
    range: f32,
    audio_layer: BitMask,
    enable_propagation: bool,
    direction: Option<AudioDirection<Vector3>>,
}
```

Methods:

- `res.Audio().three_d().play(bus_id, audio_3d) -> bool`
- `res.Audio().three_d().play_master(audio_3d) -> bool`
- `res.Audio().play_bus(bus_id, audio_3d) -> bool`
- `res.Audio().play_master(audio_3d) -> bool`

Rules:

- `Audio3D.position` is a one-shot point emitter.
- `direction: None` means omni playback.
- `direction: Some(AudioDirection::Directional(forward))` sets direction.
- omni ignores direction.
- `AudioDirection::Directional(forward)` is loudest toward `forward`.
- `AudioDirection::InverseDirectional(forward)` is loudest opposite `forward`.
- `AudioDirection::Bidirectional(forward)` is loudest toward both `forward` and `-forward`.
- Audio3D has no manual pan; pan comes from the propagated perceived direction.
- Audio outside `range` or `[audio].listener_max_distance` is skipped.
- Direct volume fades linearly from full at camera to silent at `range`.
- Propagation uses listener, physics audio materials, zones, and portals.
- Use Runtime Audio when the sound should follow a moving node.

## MIDI

MIDI lives under `res.Audio().midi()`.
It can play live notes, held notes, and `.mid` / `.midi` files.

Macros:

- `midi_load_soundfont!(res, "res://soundfonts/game.sf2") -> bool`
- `midi_play!(res, Note::C4, MidiNoteOptions::default()) -> bool`
- `midi_play!(res, bus_id, Note::C4, options) -> bool`
- `midi_play!(res, MidiSong::new("res://music/theme.mid")) -> bool`
- `midi_start!(res, Note::C4, options) -> Option<MidiNoteHandle>`
- `midi_release!(res, handle) -> bool`
- `midi_play_at!(res, Note::C4, Vector2::new(4.0, 2.0), 20.0, options) -> bool`
- `midi_play_at!(res, Note::C4, Vector3::new(4.0, 2.0, 0.0), 20.0, options) -> bool`
- `midi_start_at!(res, Note::C4, position, range, options) -> Option<MidiNoteHandle>`
- `midi_play_at!(res, MidiSong::new("res://music/theme.mid"), position, range) -> bool`

Methods:

- `res.Audio().midi().play_note(Note::C4, options) -> bool`
- `res.Audio().midi().start_note(Note::C4, options) -> Option<MidiNoteHandle>`
- `res.Audio().midi().release_note(handle) -> bool`
- `res.Audio().midi().play_file(MidiSong::new("res://music/theme.mid")) -> bool`
- `res.Audio().midi().play_note_at(Note::C4, position, range, options) -> bool`
- `res.Audio().midi().start_note_at(Note::C4, position, range, options) -> Option<MidiNoteHandle>`
- `res.Audio().midi().play_file_at(song, position, range) -> bool`

Types:

```rust
MidiNoteOptions {
    velocity: u8,          // 0..127
    sustain: Duration,     // auto note-off for play_note
    channel: MidiChannel,  // 0..15, channel 9 for drums
    program: MidiProgram,  // GM patch number
    sound: MidiSound,      // BuiltIn or SoundFont(soundfont_id)
    bus_id: Option<AudioBusID>,
    volume: f32,
    pan: AudioPan,
}

MidiSong {
    source: &str,          // literal or ResPath::as_str()
    sound: MidiSound,
    bus_id: Option<AudioBusID>,
    volume: f32,
    looped: bool,
}
```

Sound choices:

```rust
MidiSound::BuiltIn
MidiSound::SoundFont(soundfont_id)
```

Note helpers:

```rust
Note::C4              // middle C, MIDI key 60
Note::A4              // 440 Hz
Note::from_midi(36)   // raw MIDI key
Note::C4.midi_key()
Note::A4.frequency_hz()
```

Program groups:

```rust
program::Piano::AcousticGrand
program::Organ::Drawbar
program::Guitar::Nylon
program::Brass::Trumpet
program::SynthLead::Square
program::DrumKit::Standard
```

Rules:

- `Note` is pitch, for example `Note::C4`.
- `velocity` is hit strength.
- `sustain` is note length for `play_note`; held notes use `start_note` + `release_note`.
- `channel` is shared MIDI lane state.
- `program` is instrument patch.
- `MidiSound::BuiltIn` uses procedural GM-ish patches.
- `MidiSound::SoundFont(soundfont_id)` uses a loaded project soundfont.
- `Vector2` position routes to 2D propagation.
- `Vector3` position routes to 3D propagation.
- positional live notes, held notes, and MIDI files use the same raycast propagation path as audio.
- `midi_load_soundfont!` loads `.sf2` and returns `SoundFontID`.
- static builds embed `.mid`, `.midi`, and `.sf2` files under `embedded/audios/`.

Built-in vs soundfont:

- `MidiSound` chooses the synth.
- `program` chooses the patch inside that synth.
- built-in synth uses simple generated waveforms.
- soundfont uses samples/patches from the `.sf2` bank.
- same `program` value can sound very different per `.sf2`.
- full program table lives in [Audio](../../../audio.md#midi-program-table).

## MIDI Examples

Built-in note:

```rust
let opts = MidiNoteOptions {
    program: program::SynthLead::Square,
    velocity: 110,
    sustain: std::time::Duration::from_millis(120),
    ..MidiNoteOptions::default()
};

let _ = midi_play!(res, Note::C4, opts);
```

Held note:

```rust
let opts = MidiNoteOptions {
    program: program::Bass::Finger,
    ..MidiNoteOptions::default()
};

let held = midi_start!(res, Note::C2, opts);

if let Some(handle) = held {
    let _ = midi_release!(res, handle);
}
```

Soundfont notes:

```rust
let font = "res://soundfonts/game.sf2";
let font_id = midi_load_soundfont!(res, font);

let opts = MidiNoteOptions {
    sound: MidiSound::SoundFont(font_id),
    program: program::Piano::AcousticGrand,
    sustain: std::time::Duration::from_millis(400),
    ..MidiNoteOptions::default()
};

let _ = midi_play!(res, Note::C4, opts);
let _ = midi_play!(res, Note::E4, opts);
let _ = midi_play!(res, Note::G4, opts);
```

Soundfont MIDI file:

```rust
let font = "res://soundfonts/game.sf2";
let font_id = midi_load_soundfont!(res, font);
let song = MidiSong::new("res://music/theme.mid")
    .with_sound(MidiSound::SoundFont(font_id))
    .looped();

let _ = res.Audio().midi().play_file(song);
```

Positional note:

```rust
let opts = MidiNoteOptions {
    program: program::Brass::Trumpet,
    ..MidiNoteOptions::default()
};

let _ = midi_play_at!(
    res,
    Note::C5,
    Vector3::new(0.0, 2.0, -6.0),
    40.0,
    opts
);
```

## Shared Methods

- `res.Audio().load_source(source) -> bool`
- `res.Audio().reserve_source(source) -> bool`
- `res.Audio().drop_source(source) -> bool`
- `res.Audio().stop_source(source) -> bool`
- `res.Audio().source_length_seconds(source) -> Option<f32>`
- `res.Audio().source_length_millis(source) -> Option<u64>`
- `res.Audio().stop_all()`
- `res.Audio().set_master_volume(volume) -> bool`
- `res.Audio().set_bus_volume(bus_id, volume) -> bool`
- `res.Audio().set_bus_speed(bus_id, speed) -> bool`
- `res.Audio().pause_bus(bus_id) -> bool`
- `res.Audio().resume_bus(bus_id) -> bool`
- `res.Audio().stop_bus(bus_id) -> bool`

## Macro/Method Parity

- `audio_load!(res, source)` is equivalent to `res.Audio().load_source(source)`.
- `audio_reserve!(res, source)` is equivalent to `res.Audio().reserve_source(source)`.
- `audio_drop!(res, source)` is equivalent to `res.Audio().drop_source(source)`.
- `audio_play!(res, bus_id, cfg)` is equivalent to `res.Audio().play_bus(bus_id, cfg)`.
- `audio_play!(res, cfg)` is equivalent to `res.Audio().play_master(cfg)`.
- `audio_play!(...)` accepts `Audio`, panned `Audio`, `Audio2D`, or `Audio3D`.
- `audio_stop!(res, bus_id, cfg)` is equivalent to `res.Audio().stop_audio(bus_id, cfg)`.
- `audio_stop!(res, cfg)` is equivalent to `res.Audio().stop_master_audio(cfg)`.
- Other audio macros map directly to same-named `res.Audio()` methods.

## Spatial Examples

Use point audio for short sounds with fixed world positions.
Use runtime attached audio when the sound must follow a node.
Bus ids live in the play call.
Point spatial data lives on `Audio2D` or `Audio3D`.

One-shot 2D hit:

```rust
let sfx = audio_bus!("sfx");

let hit = Audio2D {
    audio: Audio::new("res://audio/hit.wav"),
    position: enemy_pos,
    range: 256.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction: None,
};

let _ = audio_play!(res, sfx, hit);
```

3D speaker cone:

```rust
let speaker = Audio3D {
    audio: Audio::new("res://audio/alert.wav"),
    position: speaker_pos,
    range: 35.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction: Some(AudioDirection::Directional(Vector3::new(0.0, 0.0, -1.0))),
};

let _ = audio_play!(res, audio_bus!("sfx"), speaker);
```

No ray propagation:

```rust
let ui_world_ping = Audio2D {
    audio: Audio::new("res://audio/ping.wav"),
    position: marker_pos,
    range: 128.0,
    audio_layer: BitMask::ALL,
    enable_propagation: false,
    direction: None,
};

let _ = audio_play!(res, ui_world_ping);
```

## Example

```rust
let music = audio_bus!("music");
let _ = audio_set_master_volume!(res, 1.0);
let _ = audio_bus_set_volume!(res, music, 0.7);
let _ = audio_bus_set_speed!(res, music, 1.0);

let base = Audio {
    source: "res://groantube.mp3",
    looped: true,
    volume: 1.0,
    effects: AudioEffects::new(),
    from_start: 0.0,
    from_end: 0.0,
};

let _ = audio_play!(res, music, base);
let _ = res.Audio().play_bus(music, base);
let _ = audio_stop!(res, music, base);
let _ = audio_play!(res, base);
let _ = audio_play!(res, music, (base, AudioPan::new(-0.5, 0.0, 0.25)));

let hit = Audio2D::new("res://hit.wav", Vector2::new(128.0, 64.0), 512.0);
let _ = audio_play!(res, hit);

let step = Audio3D {
    audio: Audio::new("res://step.wav"),
    position: Vector3::new(0.0, 0.0, -5.0),
    range: 20.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction: Some(AudioDirection::Directional(Vector3::new(0.0, 0.0, 1.0))),
};
let _ = audio_play!(res, music, step);

if let Some(length) = audio_length_seconds!(res, "res://groantube.mp3") {
    let half = Audio {
        source: "res://groantube.mp3",
        looped: false,
        volume: 1.0,
        effects: AudioEffects::new(),
        from_start: 0.0,
        from_end: length * 0.5,
    };
    let _ = audio_play!(res, music, half);
}
```
