# Audio Module

Access:

- `res.Audio()`

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

- `audio_play!(res, bus_id, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `audio_play!(res, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `audio_play!(res, bus_id, PannedAudio { audio, pan }) -> bool`
- `audio_play!(res, (audio, pan)) -> bool`
- `audio_stop!(res, bus_id, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `audio_stop!(res, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`

Type:

```rust
Audio {
    source: &str,      // res://...
    looped: bool,
    volume: f32,      // 1.0 normal, 0.0 silent, >1 amplified
    speed: f32,       // 1.0 normal playback speed (also changes pitch)
    from_start: f32,  // seconds trimmed from the start (>= 0.0)
    from_end: f32,    // seconds trimmed from the end (>= 0.0)
}

PannedAudio {
    audio: Audio,
    pan: AudioPan,    // x left/right, y down/up, z back/front
}
```

Methods:

- `res.Audio().play_bus(bus_id, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `res.Audio().play_master(Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `res.Audio().play_bus(bus_id, PannedAudio { audio, pan }) -> bool`
- `res.Audio().play_panned(bus_id, audio, pan) -> bool`
- `res.Audio().play_master_panned(audio, pan) -> bool`
- `res.Audio().stop_audio(bus_id, Audio { source, looped, volume, speed, from_start, from_end }) -> bool`
- `res.Audio().stop_master_audio(Audio { source, looped, volume, speed, from_start, from_end }) -> bool`

Rules:

- plain `Audio` plays centered.
- `PannedAudio.pan.x` is clamped to `-1.0..1.0` for left/right.
- `PannedAudio.pan.y` is clamped to `-1.0..1.0` for down/up.
- `PannedAudio.pan.z` is clamped to `-1.0..1.0` for back/front spatial flavor.

## Audio2D

Audio2D uses `Vector2` position and resolves pan/volume against the active `Camera2D`.

Macros:

- `audio_play!(res, bus_id, Audio2D { audio, position, range }) -> bool`
- `audio_play!(res, Audio2D { audio, position, range }) -> bool`

Type:

```rust
Audio2D {
    audio: Audio,
    position: Vector2,
    range: f32,
}
```

Methods:

- `res.Audio().two_d().play(bus_id, Audio2D { audio, position, range }) -> bool`
- `res.Audio().two_d().play_master(Audio2D { audio, position, range }) -> bool`
- `res.Audio().play_bus(bus_id, Audio2D { audio, position, range }) -> bool`
- `res.Audio().play_master(Audio2D { audio, position, range }) -> bool`

Rules:

- `Audio2D.position` is resolved against active `Camera2D`.
- Audio2D has no manual pan; pan is derived from camera-relative position.
- Audio outside `range` is skipped.
- Volume fades linearly from full at camera to silent at `range`.
- Pan uses camera rotation before clamping to `-1.0..1.0`.

## Audio3D

Audio3D uses `Vector3` position and resolves pan/volume against the active `Camera3D`.

Macros:

- `audio_play!(res, bus_id, Audio3D { audio, position, range }) -> bool`
- `audio_play!(res, Audio3D { audio, position, range }) -> bool`

Type:

```rust
Audio3D {
    audio: Audio,
    position: Vector3,
    range: f32,
}
```

Methods:

- `res.Audio().three_d().play(bus_id, Audio3D { audio, position, range }) -> bool`
- `res.Audio().three_d().play_master(Audio3D { audio, position, range }) -> bool`
- `res.Audio().play_bus(bus_id, Audio3D { audio, position, range }) -> bool`
- `res.Audio().play_master(Audio3D { audio, position, range }) -> bool`

Rules:

- `Audio3D.position` is resolved against active `Camera3D`.
- Audio3D has no manual pan; pan is derived from camera-relative position.
- Audio outside `range` is skipped.
- Volume fades linearly from full at camera to silent at `range`.
- Pan uses camera orientation before clamping to `-1.0..1.0`.

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
- `audio_play!(...)` accepts `Audio`, `Audio2D`, or `Audio3D`.
- `audio_stop!(res, bus_id, cfg)` is equivalent to `res.Audio().stop_audio(bus_id, cfg)`.
- `audio_stop!(res, cfg)` is equivalent to `res.Audio().stop_master_audio(cfg)`.
- Other audio macros map directly to same-named `res.Audio()` methods.

## Runtime Behavior

- Script call enqueues an audio command via `RuntimeResourceApi`.
- `perro_bark` handles commands on its own audio thread.
- Audio bytes/duration are cached by source path for reuse.
- `audio_load!` caches as unreserved (`reserved: false`).
- `audio_reserve!` caches as reserved (`reserved: true`), preventing automatic eviction.
- Unreserved cached audio is evicted after idle time with `ttl = max(2.0 * audio_length, 250ms)`, and idle timer starts when playback ends/stops.
- Playback uses one sink per source path; replaying same source replaces previous playback.
- Final loudness is multiplicative:
  - `final_volume = master_volume * bus_volume * audio.volume` when bus is provided
  - `final_volume = master_volume * audio.volume` when no bus is provided
- Final playback rate is multiplicative:
  - `final_speed = bus_speed * audio.speed` when bus is provided
  - `final_speed = audio.speed` when no bus is provided
- `speed` controls playback speed multiplier and also affects pitch.
- Effective playback segment:
  - starts at `from_start`
  - ends `from_end` seconds before the source end (when duration is known)
  - if `from_start + from_end` removes the full clip, playback is rejected

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
    speed: 1.0,
    from_start: 0.0,
    from_end: 0.0,
};

let _ = audio_play!(res, music, base);
let _ = res.Audio().play_bus(music, base);
let _ = audio_stop!(res, music, base);
let _ = audio_play!(res, base);
let _ = audio_play!(res, music, (base, AudioPan::new(-0.5, 0.0, 0.25)));

let hit = Audio2D {
    audio: Audio::new("res://hit.wav"),
    position: Vector2::new(128.0, 64.0),
    range: 512.0,
};
let _ = audio_play!(res, hit);

let step = Audio3D {
    audio: Audio::new("res://step.wav"),
    position: Vector3::new(0.0, 0.0, -5.0),
    range: 20.0,
};
let _ = audio_play!(res, music, step);

if let Some(length) = audio_length_seconds!(res, "res://groantube.mp3") {
    let half = Audio {
        source: "res://groantube.mp3",
        looped: false,
        volume: 1.0,
        speed: 1.0,
        from_start: 0.0,
        from_end: length * 0.5,
    };
    let _ = audio_play!(res, music, half);
}
```
