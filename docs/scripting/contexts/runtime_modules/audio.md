# Audio Module

Access:

- `ctx.run.Audio()`

Runtime audio is for node-bound sounds.
Point 2D/3D sounds live on `ctx.res.Audio()` because they are resource playback requests.

Shared backend, cache, bus, `.pawdio`, and propagation concepts:

- [Audio](../../../audio.md)

## Attached Audio

Use attached audio when a sound should follow a scene node without adding a dedicated audio node.
Use it for engine loops, moving speakers, projectile hum, held notes, and any sound that must stay glued to a `Node2D` or `Node3D`.
Use point audio on `ctx.res.Audio()` for one-shot sounds that do not move after spawn.

Methods:

- `ctx.run.Audio().play_attached(audio, node_id, options) -> bool`
- `ctx.run.Audio().play_attached_bus(bus_id, audio, node_id, options) -> bool`
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
    occlusion_mask: u32,
    enable_propagation: bool,
    direction_2d: AudioDirection<Vector2>,
    direction_3d: AudioDirection<Vector3>,
}
```

Rules:

- `Node2D`-derived nodes use the 2D propagation solver.
- `Node3D`-derived nodes use the 3D propagation solver.
- `SpatialAudioOptions` has no shorthand ctor; set range and direction fields.
- `AudioDirection::Omni` means default omni playback.
- omni ignores direction.
- `direction_2d: AudioDirection::Directional(forward)` sets 2D directional mode.
- `direction_3d: AudioDirection::Directional(forward)` sets 3D directional mode.
- inverse directional is loudest opposite forward.
- bidirectional is loudest forward and backward.
- direction vectors are normalized by the runtime.
- attached audio uses node rotation as forward when direction is non-omni.
- explicit direction vector is fallback for point use and non-attached use.
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
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};

let _ = ctx.run.Audio().play_attached_bus(audio_bus!("sfx"), sound, car_node, options);
let _ = ctx.run.Audio().stop_attached(car_node, "res://audio/car_loop.wav");
```

Directional attached audio:

```rust
let cone = SpatialAudioOptions {
    range: 60.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Directional(Vector3::new(0.0, 0.0, -1.0)),
};
let _ = ctx.run.Audio().play_attached(sound, speaker_node, cone);

let two_way = SpatialAudioOptions {
    range: 30.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Bidirectional(Vector2::new(0.0, -1.0)),
    direction_3d: AudioDirection::Omni,
};
let _ = ctx.run.Audio().play_attached(sound, siren_node, two_way);
```

Moving machine loop:

```rust
let machine = RuntimeAudio {
    source: "res://audio/machine_loop.ogg",
    looped: true,
    volume: 0.8,
    effects: AudioEffects {
        low_pass: 0.05,
        reverb_send: 0.1,
        ..AudioEffects::new()
    },
    from_start: 0.0,
    from_end: 0.0,
};

let spatial = SpatialAudioOptions {
    range: 48.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};

let _ = ctx.run.Audio().play_attached_bus(audio_bus!("ambience"), machine, machine_node, spatial);
```

Rotating siren:

```rust
let siren = RuntimeAudio::new("res://audio/siren_loop.wav");
let spatial = SpatialAudioOptions {
    range: 96.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Bidirectional(Vector2::new(0.0, -1.0)),
    direction_3d: AudioDirection::Omni,
};

let _ = ctx.run.Audio().play_attached_bus(audio_bus!("sfx"), siren, siren_node, spatial);
```

## Attached MIDI

Attached MIDI derives 2D or 3D from the node spatial type.
The note or MIDI file follows node transforms and uses propagation raycasts.

```rust
let spatial = SpatialAudioOptions {
    range: 40.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};
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

Attached held note:

```rust
let spatial = SpatialAudioOptions {
    range: 24.0,
    occlusion_mask: u32::MAX,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};

let opts = MidiNoteOptions {
    program: program::SynthLead::Square,
    volume: 0.6,
    ..MidiNoteOptions::default()
};

if let Some(handle) = midi_start_attached!(ctx.run, Note::C4, orb_node, opts, spatial) {
    let _ = midi_stop_attached!(ctx.run, orb_node, handle);
}
```

## Point Audio

Use `ctx.res.Audio()` for sounds played at points:

```rust
let hit = Audio2D::new("res://audio/hit.wav", Vector2::new(128.0, 64.0), 512.0);
let _ = audio_play!(ctx.res, hit);
```

Point sounds also use propagation.
They do not bind to nodes.
