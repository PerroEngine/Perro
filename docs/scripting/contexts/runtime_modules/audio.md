# Audio Module

Access:

- `ctx.run.Audio()`

Runtime audio is for node-bound sounds.
Point 2D/3D sounds live on `ctx.res.Audio()` because they are resource playback requests.

## Attached Audio

Use attached audio when a sound should follow a scene node without adding an audio emitter node.

Methods:

- `ctx.run.Audio().play_attached(audio, node_id, options) -> bool`
- `ctx.run.Audio().stop_attached(node_id, source) -> bool`

Types:

```rust
RuntimeAudio {
    source: &str,
    looped: bool,
    volume: f32,
    speed: f32,
    from_start: f32,
    from_end: f32,
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
- No `Audio2D` or `Audio3D` scene node is required.

Example:

```rust
let sound = RuntimeAudio {
    source: "res://audio/car_loop.wav",
    looped: true,
    volume: 1.0,
    speed: 1.0,
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
