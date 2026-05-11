# Audio

Perro audio has three layers:

- script API: `ctx.res.Audio()` and `ctx.run.Audio()`
- runtime propagation: listener, occlusion, reflection, portals, zones
- playback backend: `perro_pawdio`

Use API docs for script calls:

- [Resource Audio Module](scripting/contexts/resource_modules/audio.md)
- [Runtime Audio Module](scripting/contexts/runtime_modules/audio.md)

## `perro_pawdio`

`perro_pawdio` is the audio backend crate.

Path:

- `perro_source/audio_stack/perro_pawdio`

Public pieces:

- `AudioController`: command sender and audio thread owner
- `BarkPlayer`: rodio output stream, spatial sinks, cache, buses
- `AudioPlaybackRequest`: playback command data
- `SpatialAudioParams`: live spatial playback update data
- `AudioPan`, `AudioEq`, `AudioCompression`: shared playback controls
- `Audio2D`, `Audio3D`, `AudioListener2D`, `AudioListener3D`: simple backend spatial types

Scripts do not call `BarkPlayer` directly.
Script calls go through runtime/resource APIs.
Those APIs enqueue commands into `AudioController`.
`AudioController` spawns the `perro_pawdio_audio` thread.
That thread owns `BarkPlayer` and handles load/play/stop/bus/spatial commands.

## Source Formats

Runtime audio source paths use normal project asset paths:

- `res://...wav`
- `res://...ogg`
- `res://...mp3`
- `res://...flac`
- `res://...aac`
- `res://...m4a`

Dynamic runtime loads read those files and let `rodio` decode them.

Static builds pack audio into `.pawdio` blobs.
`.pawdio` is not an authored source format.
It is a static pipeline container for embedded audio bytes.

Static pipeline behavior:

- scan audio files in `res/`
- preserve original `res://...` lookup path
- write embedded files under `embedded/audios/`
- emit `static/audios.rs` lookup code
- choose zlib payload only when smaller than raw bytes

`.pawdio` v1 layout:

- magic: `PAWDIO`
- version: `1`
- flags: `FLAG_ZLIB` or `0`
- raw length
- payload: raw source bytes or zlib-compressed source bytes

At runtime, `perro_pawdio` unwraps `.pawdio` back into original audio bytes.
Then `rodio` decodes those bytes like a normal source.

## Playback Model

All playback uses an `AudioPlaybackRequest`.

Core fields:

- source path
- optional bus id
- loop flag
- volume
- speed
- pan
- effect values
- trim start/end
- playback id for spatial updates

Plain `Audio` plays centered unless wrapped in `PannedAudio`.
`Audio2D` and `Audio3D` become spatial requests before backend playback.
Attached runtime audio follows node transforms before backend playback.

Current backend rule:

- one active sink per source path
- new play of same source stops previous playback for that source

## Buses

Audio buses group playback controls.

Bus state:

- volume
- speed
- paused flag

Final volume:

- with bus: `master_volume * bus_volume * audio.volume`
- without bus: `master_volume * audio.volume`

Final speed:

- with bus: `bus_speed * audio.effects.speed`
- without bus: `audio.effects.speed`

`effects.speed` changes playback rate and pitch.

## Cache

`perro_pawdio` caches source bytes by source path.

Cache commands:

- load: cache as unreserved
- reserve: cache as reserved
- drop: remove cached source

Reserved sources stay cached until explicit drop.
Unreserved sources can be evicted after use.

Unreserved eviction:

- when duration known: `max(audio_length * 2.0, 250ms)`
- when duration unknown: `1s`
- never evict while source has active playback
- cache soft limit: `128 MiB`

## Spatial Audio

Spatial audio starts in script API, then moves through runtime propagation.

Entrypoints:

- `ctx.res.Audio()` for point `Audio2D` and `Audio3D`
- `ctx.run.Audio()` for node-attached runtime audio

Listener source:

- active `Camera2D` for 2D
- active `Camera3D` for 3D

Runtime propagation inputs:

- source position and range
- listener transform
- audio material fields on physics nodes
- `AudioMask2D`
- `AudioPortal2D` and `AudioPortal3D`
- audio zones

Propagation output becomes `SpatialAudioParams`.
The runtime sends those params to `perro_pawdio`.
`perro_pawdio` applies pan and volume to the sink.
It also tracks low-pass, reflection, occlusion, EQ, and compression values on playback state.

Propagation runs only while active positional or attached spatial sounds exist.
If no active spatial sounds exist, no audio ray work runs that frame.

## Audio Materials

Physics bodies participate in audio propagation when `audio_interaction = true`.

Material fields:

- `audio_interaction`
- `absorption`
- `reflection`
- `transmission`
- `diffusion`
- `low_pass_strength`
- `thickness_multiplier`
- `occlusion_mask`

Use audio masks, zones, and portals for invisible or non-physical audio geometry.

## Portals

Audio portals are one-way links.
Add reverse links when sound should travel both directions.

Portal behavior:

- collision-shape children form portal input surfaces
- target portal gives exit transform
- hit point and ray direction move through target transform
- ray continues tracing after exit
- portal hops stop at cycle guard
- ray cannot immediately re-enter portal it just exited

## Project Config

```toml
[audio]
listener_max_distance = 500.0
propagation_tick_hz = 20
energy_cutoff = 0.02
debug_rays = false

[audio.propagation_2d]
max_bounces = 4
rays_per_tick = 64
max_ray_distance = 500.0

[audio.propagation_3d]
max_bounces = 4
rays_per_tick = 128
max_ray_distance = 500.0
```
