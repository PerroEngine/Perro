# Audio Module

Access:

- `ctx.run.Audio()`

Runtime audio is for node-bound sounds.
Point 2D/3D sounds live on `ctx.res.Audio()` because they are resource playback requests.

Shared backend, cache, bus, `.pawdio`, and propagation concepts:

- [Audio](../../../audio.md)

## Attached Audio

Use attached audio when a sound should follow a scene node without adding an audio emitter node.

Methods:

- `ctx.run.Audio().play_attached(audio, node_id, options) -> bool`
- `ctx.run.Audio().stop_attached(node_id, source) -> bool`
- `ctx.run.Audio().midi().play_note_attached(note, node_id, options, spatial) -> bool`
- `ctx.run.Audio().midi().start_note_attached(note, node_id, options, spatial) -> Option<MidiNoteHandle>`
- `ctx.run.Audio().midi().play_file_attached(song, node_id, spatial) -> bool`
- `ctx.run.Audio().midi().release_note(handle) -> bool`
- `ctx.run.Audio().midi().stop_attached(node_id, handle_or_source) -> bool`

MIDI macros:

- `midi_play_attached!(ctx.run, Note::C4, node_id, options, spatial) -> bool`
- `midi_play_attached!(ctx.run, MidiSong::new("res://music/theme.mid"), node_id, spatial) -> bool`
- `midi_start_attached!(ctx.run, Note::C4, node_id, options, spatial) -> Option<MidiNoteHandle>`
- `midi_release_attached!(ctx.run, handle) -> bool`
- `midi_stop_attached!(ctx.run, node_id, handle_or_source) -> bool`

Types:

```rust
RuntimeAudio {
    source: &str,
    looped: bool,
    volume: f32,
    effects: AudioEffects,
    from_start: f32,
    from_end: f32,
}

AudioEffects {
    speed: f32,
    low_pass: f32,
    reverb_send: f32,
    echo: f32,
    reflection: f32,
    occlusion: f32,
    eq: AudioEq,
    compression: AudioCompression,
}

SpatialAudioOptions {
    range: f32,
    bus_id: Option<AudioBusID>,
    occlusion_mask: u32,
    enable_propagation: bool,
}
```

Rules:

- `Node2D`-derived nodes use the 2D propagation solver.
- `Node3D`-derived nodes use the 3D propagation solver.
- Missing node after playback starts freezes the last valid position.
- `stop_attached(node_id, source)` stops only sounds matching both node and source.

Example:

```rust
let sound = RuntimeAudio {
    source: "res://audio/car_loop.wav",
    looped: true,
    volume: 1.0,
    effects: AudioEffects::new(),
    from_start: 0.0,
    from_end: 0.0,
};

let options = SpatialAudioOptions {
    range: 80.0,
    bus_id: Some(audio_bus!("sfx")),
    occlusion_mask: u32::MAX,
    enable_propagation: true,
};

let _ = ctx.run.Audio().play_attached(sound, car_node, options);
let _ = ctx.run.Audio().stop_attached(car_node, "res://audio/car_loop.wav");
```

## Attached MIDI

Attached MIDI derives 2D or 3D from the node spatial type.
The note or MIDI file follows node transforms and uses propagation raycasts.

```rust
let spatial = SpatialAudioOptions::new(40.0);
let opts = MidiNoteOptions {
    program: program::Brass::Trumpet,
    sustain: std::time::Duration::from_millis(350),
    ..MidiNoteOptions::default()
};

let _ = ctx.run.Audio().midi().play_note_attached(Note::C4, node, opts, spatial);

let held = ctx.run.Audio().midi().start_note_attached(Note::G3, node, opts, spatial);
if let Some(handle) = held {
    let _ = ctx.run.Audio().midi().release_note(handle);
}

let song = MidiSong::new("res://music/theme.mid").looped();
let _ = ctx.run.Audio().midi().play_file_attached(song, node, spatial);
let _ = ctx.run.Audio().midi().stop_attached(node, "res://music/theme.mid");
```

## Point Audio

Use `ctx.res.Audio()` for sounds played at points:

```rust
let hit = Audio2D {
    audio: Audio::new("res://audio/hit.wav"),
    position: Vector2::new(128.0, 64.0),
    range: 512.0,
};

let _ = audio_play!(ctx.res, hit);
```

Point sounds also use propagation.
They do not bind to nodes.
