# Audio Module

Access:

- `res.Audio()`

Macros:

- `bus!("name") -> BusID`
- `play_audio!(res, Audio { source, bus, looped, volume, speed, from_start, from_end }) -> bool`
- `stop_audio!(res, Audio { source, bus, looped, volume, speed, from_start, from_end }) -> bool`
- `stop_audio_source!(res, source) -> bool`
- `audio_length_seconds!(res, source) -> Option<f32>`
- `audio_length_millis!(res, source) -> Option<u64>`
- `stop_all_audio!(res)`
- `set_master_volume!(res, volume) -> bool`
- `set_bus_volume!(res, bus_id, volume) -> bool`
- `set_bus_speed!(res, bus_id, speed) -> bool`
- `pause_bus!(res, bus_id) -> bool`
- `resume_bus!(res, bus_id) -> bool`
- `stop_bus!(res, bus_id) -> bool`

Type:

```rust
Audio {
    source: &str, // res://...
    bus: BusID,   // e.g. bus!("music")
    looped: bool,
    volume: f32,  // 1.0 normal, 0.0 silent, >1 amplified
    speed: f32,   // 1.0 normal playback speed (also changes pitch)
    from_start: f32, // seconds trimmed from the start (>= 0.0)
    from_end: f32,   // seconds trimmed from the end (>= 0.0)
}
```

Module methods:

- `res.Audio().play(Audio { source, bus, looped, volume, speed, from_start, from_end }) -> bool`
- `res.Audio().stop_audio(Audio { source, bus, looped, volume, speed, from_start, from_end }) -> bool`
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

Macro/method parity:

- `play_audio!(res, cfg)` is equivalent to `res.Audio().play(cfg)`.
- `stop_audio!(res, cfg)` is equivalent to `res.Audio().stop_audio(cfg)`.
- Other audio macros map directly to same-named `res.Audio()` methods.

How it maps to `perro_bark`:

- Script call enqueues an audio command via `RuntimeResourceApi`.
- `perro_bark` handles commands on its own audio thread.
- Playback uses one sink per source path; replaying same source replaces previous playback.
- Final loudness is multiplicative:
  - `final_volume = master_volume * bus_volume * Audio.volume`
- Final playback rate is multiplicative:
  - `final_speed = bus_speed * Audio.speed`
- `speed` controls playback speed multiplier and also affects pitch.
- Effective playback segment:
  - starts at `Audio.from_start`
  - ends `Audio.from_end` seconds before the source end (when duration is known)
  - if `from_start + from_end` removes the full clip, playback is rejected

Example:

```rust
let music = bus!("music");
let _ = set_master_volume!(res, 1.0);
let _ = set_bus_volume!(res, music, 0.7);
let _ = set_bus_speed!(res, music, 1.0);

let cfg = Audio {
    source: "res://groantube.mp3",
    bus: music,
    looped: true,
    volume: 1.0,
    speed: 1.0,
    from_start: 0.0,
    from_end: 0.0,
};

let _ = play_audio!(res, cfg);
let _ = res.Audio().play(cfg);
let _ = stop_audio!(res, cfg);
let _ = stop_audio_source!(res, "res://groantube.mp3");

// play first half of the clip using queried duration
if let Some(length) = audio_length_seconds!(res, "res://groantube.mp3") {
    let half = Audio {
        source: "res://groantube.mp3",
        bus: music,
        looped: false,
        volume: 1.0,
        speed: 1.0,
        from_start: 0.0,
        from_end: length * 0.5,
    };
    let _ = play_audio!(res, half);
}
```
