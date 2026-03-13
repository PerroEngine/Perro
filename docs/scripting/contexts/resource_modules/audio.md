# Audio Module

Access:
- `res.Audio()`

Macros:
- `bus!("name") -> u32`
- `play_audio!(res, Audio { source, bus, looped, volume, pitch }) -> bool`
- `stop_audio!(res, Audio { source, bus, looped, volume, pitch }) -> bool`
- `stop_audio_source!(res, source) -> bool`
- `stop_all_audio!(res)`
- `set_master_volume!(res, volume) -> bool`
- `set_bus_volume!(res, bus_id, volume) -> bool`
- `pause_bus!(res, bus_id) -> bool`
- `resume_bus!(res, bus_id) -> bool`
- `stop_bus!(res, bus_id) -> bool`

Type:

```rust
Audio {
    source: &str, // res://...
    bus: u32,     // e.g. bus!("music") or 0
    looped: bool,
    volume: f32,  // 1.0 normal, 0.0 silent, >1 amplified
    pitch: f32,   // 1.0 normal
}
```

Module methods:
- `res.Audio().play(Audio { source, bus, looped, volume, pitch }) -> bool`
- `res.Audio().stop_audio(Audio { source, bus, looped, volume, pitch }) -> bool`
- `res.Audio().stop_source(source) -> bool`
- `res.Audio().stop_all()`
- `res.Audio().set_master_volume(volume) -> bool`
- `res.Audio().set_bus_volume(bus_id, volume) -> bool`
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
- `pitch` currently maps to playback speed multiplier.

Example:

```rust
let music = bus!("music");
let _ = set_master_volume!(res, 1.0);
let _ = set_bus_volume!(res, music, 0.7);

let cfg = Audio {
    source: "res://groantube.mp3",
    bus: music,
    looped: true,
    volume: 1.0,
    pitch: 1.0,
};

let _ = play_audio!(res, cfg);
let _ = res.Audio().play(cfg);
let _ = stop_audio!(res, cfg);
let _ = stop_audio_source!(res, "res://groantube.mp3");
```
