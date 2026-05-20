# Audio

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

Use `Audio` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# Audio

Perro audio has three layers:

- script API: `ctx.res.Audio()` and `ctx.run.Audio()`
- runtime propagation: listener, occlusion, reflection, portals, effect zones
- playback backend: `perro_pawdio`

Use API docs for script calls:

- [Resource Audio Module](../scripting/contexts/resource_modules/audio.md)
- [Runtime Audio Module](../scripting/contexts/runtime_modules/audio.md)

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
- `res://...mid`
- `res://...midi`
- `res://...sf2`

Dynamic runtime loads read audio files and let `rodio` decode them.
MIDI files and soundfonts are read through the same asset path system.

Static builds pack audio into `.pawdio` blobs.
`.pawdio` is not an authored source format.
It is a static pipeline container for embedded audio bytes.

Static pipeline behavior:

- scan audio, MIDI, and soundfont files in `res/`
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
MIDI files and `.sf2` soundfonts are embedded as raw bytes and keep their original extension.

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

## MIDI

MIDI lives under `ctx.res.Audio().midi()`.
It supports:

- live one-shot notes
- held notes with explicit release
- `.mid` and `.midi` file playback
- built-in procedural instruments
- `.sf2` soundfont instruments
- bus volume, bus speed, pause, resume, and stop
- point 2D/3D propagation
- node-attached propagation through `ctx.run.Audio().midi()`

Main types:

- `Note`: MIDI key wrapper with constants from `Note::C0` through `Note::C8`
- `MidiChannel`: `0..15`,
- `MidiProgram`: GM patch `0..127`
- `MidiSound`: `BuiltIn` or `SoundFont(soundfont_id)`
- `MidiNoteOptions`: velocity, sustain, channel, program, sound, bus, volume, pan
- `MidiSong`: source path, sound, bus, volume, loop flag
- `MidiNoteHandle`: handle returned by held notes

Notes:

- `Note::from_midi(key)` accepts raw MIDI keys.
- `play_note` uses `MidiNoteOptions.sustain` for automatic note-off.
- `start_note` ignores sustain and keeps the note alive until release.
- `release_note(handle)` stops held notes.
- channel + program pick the instrument lane.
- channel 9 is the standard drum lane.
- bus speed changes MIDI playback rate.

## Built-In Vs Soundfont MIDI

`MidiSound` chooses the synthesizer.
`program` chooses the instrument slot inside that synthesizer.

`MidiSound::BuiltIn`:

- uses Perro's procedural synth
- needs no asset file
- uses `program` as a GM-style category hint
- maps program ranges to simple wave types
- does not use sampled instruments
- good for quick tones, prototyping, and light effects

Built-in program wave map:

- piano/chromatic/default: sine
- organ + synth lead: square
- guitar + bass + brass + reed + pipe: saw
- strings + ensemble: triangle
- synth pad + synth fx: sine
- percussive + sound fx: noise

`MidiSound::SoundFont(soundfont_id)`:

- loads an `.sf2` bank
- uses `program` to pick a patch inside that bank
- uses `channel` to hold current program state
- uses channel 9 for drums by MIDI convention
- sound quality depends on the `.sf2`

So the soundfont is not one instrument.
It is a bank of many instruments.
`program::Piano::AcousticGrand` means patch 0 in that bank.
`program::Brass::Trumpet` means patch 56 in that bank.
If the `.sf2` has weak or missing patches, output follows that file.

Built-in MIDI:

```rust
let music = audio_bus!("music");

let lead = MidiNoteOptions {
    velocity: 112,
    sustain: std::time::Duration::from_millis(180),
    program: program::SynthLead::Square,
    volume: 0.8,
    ..MidiNoteOptions::default()
};

let _ = midi_play!(ctx.res, music, Note::C4, lead);
let _ = ctx.res.Audio().midi().play_note(Note::E4, lead);

if let Some(handle) = midi_start!(ctx.res, Note::G4, lead) {
    let _ = midi_release!(ctx.res, handle);
}

let song = MidiSong::new("res://music/theme.mid").looped();
let _ = midi_play!(ctx.res, song);
```

Soundfont MIDI:

```rust
let font = "res://soundfonts/game.sf2";
let font_id = midi_load_soundfont!(ctx.res, font);

let piano = MidiNoteOptions {
    sound: MidiSound::SoundFont(font_id),
    program: program::Piano::AcousticGrand,
    sustain: std::time::Duration::from_millis(350),
    ..MidiNoteOptions::default()
};

let _ = midi_play!(ctx.res, Note::C4, piano);
let _ = midi_play!(ctx.res, Note::E4, piano);
let _ = midi_play!(ctx.res, Note::G4, piano);

let sf2_song = MidiSong::new("res://music/theme.mid")
    .with_sound(MidiSound::SoundFont(font_id))
    .looped();
let _ = ctx.res.Audio().midi().play_file(sf2_song);
```

Soundfont rules:

- source must be a project asset path, usually `res://soundfonts/name.sf2`
- `midi_load_soundfont!` loads the bank and returns `SoundFontID`
- same source returns same `SoundFontID`
- notes and files require a loaded soundfont id
- one live-note soundfont mixer is shared per soundfont, bus, and pan
- `MidiSound::BuiltIn` does not require `.sf2`

Positional MIDI:

```rust
let font_id = midi_load_soundfont!(ctx.res, "res://soundfonts/game.sf2");

let opts = MidiNoteOptions {
    sound: MidiSound::SoundFont(font_id),
    program: program::Brass::Trumpet,
    ..MidiNoteOptions::default()
};

let _ = midi_play_at!(
    ctx.res,
    Note::C5,
    Vector2::new(128.0, 64.0),
    512.0,
    opts
);

let _ = midi_play_at!(
    ctx.res,
    MidiSong::new("res://music/sting.mid"),
    Vector3::new(0.0, 2.0, -6.0),
    40.0
);
```

Attached MIDI:

```rust
let spatial = SpatialAudioOptions {
    range: 40.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};
let opts = MidiNoteOptions {
    program: program::Guitar::Clean,
    ..MidiNoteOptions::default()
};

let _ = ctx.run.Audio().midi().play_note_attached(Note::A3, node, opts, spatial);

let held = ctx.run.Audio().midi().start_note_attached(Note::E3, node, opts, spatial);
if let Some(handle) = held {
    let _ = ctx.run.Audio().midi().release_note(handle);
}

let song = MidiSong::new("res://music/loop.mid").looped();
let _ = ctx.run.Audio().midi().play_file_attached(song, node, spatial);
let _ = ctx.run.Audio().midi().stop_attached(node, "res://music/loop.mid");
```

## MIDI Program Table

Programs follow General MIDI patch numbers.
Use `MidiProgram::new(n)` for a raw value.
Use `program::Group::Name` for named values.

| Num | Helper                               | GM name                 |
| --- | ------------------------------------ | ----------------------- |
| 0   | `program::Piano::AcousticGrand`      | Acoustic Grand Piano    |
| 1   | `program::Piano::BrightAcoustic`     | Bright Acoustic Piano   |
| 2   | `program::Piano::ElectricGrand`      | Electric Grand Piano    |
| 3   | `program::Piano::HonkyTonk`          | Honky-tonk Piano        |
| 4   | `program::Piano::Electric1`          | Electric Piano 1        |
| 5   | `program::Piano::Electric2`          | Electric Piano 2        |
| 6   | `program::Piano::Harpsichord`        | Harpsichord             |
| 7   | `program::Piano::Clavinet`           | Clavinet                |
| 8   | `program::Chromatic::Celesta`        | Celesta                 |
| 9   | `program::Chromatic::Glockenspiel`   | Glockenspiel            |
| 10  | `program::Chromatic::MusicBox`       | Music Box               |
| 11  | `program::Chromatic::Vibraphone`     | Vibraphone              |
| 12  | `program::Chromatic::Marimba`        | Marimba                 |
| 13  | `program::Chromatic::Xylophone`      | Xylophone               |
| 14  | `program::Chromatic::TubularBells`   | Tubular Bells           |
| 15  | `program::Chromatic::Dulcimer`       | Dulcimer                |
| 16  | `program::Organ::Drawbar`            | Drawbar Organ           |
| 17  | `program::Organ::Percussive`         | Percussive Organ        |
| 18  | `program::Organ::Rock`               | Rock Organ              |
| 19  | `program::Organ::Church`             | Church Organ            |
| 20  | `program::Organ::Reed`               | Reed Organ              |
| 21  | `program::Organ::Accordion`          | Accordion               |
| 22  | `program::Organ::Harmonica`          | Harmonica               |
| 23  | `program::Organ::TangoAccordion`     | Tango Accordion         |
| 24  | `program::Guitar::Nylon`             | Acoustic Guitar (nylon) |
| 25  | `program::Guitar::Steel`             | Acoustic Guitar (steel) |
| 26  | `program::Guitar::Jazz`              | Electric Guitar (jazz)  |
| 27  | `program::Guitar::Clean`             | Electric Guitar (clean) |
| 28  | `program::Guitar::Muted`             | Electric Guitar (muted) |
| 29  | `program::Guitar::Overdriven`        | Overdriven Guitar       |
| 30  | `program::Guitar::Distortion`        | Distortion Guitar       |
| 31  | `program::Guitar::Harmonics`         | Guitar Harmonics        |
| 32  | `program::Bass::Acoustic`            | Acoustic Bass           |
| 33  | `program::Bass::Finger`              | Electric Bass (finger)  |
| 34  | `program::Bass::Pick`                | Electric Bass (pick)    |
| 35  | `program::Bass::Fretless`            | Fretless Bass           |
| 36  | `program::Bass::Slap1`               | Slap Bass 1             |
| 37  | `program::Bass::Slap2`               | Slap Bass 2             |
| 38  | `program::Bass::Synth1`              | Synth Bass 1            |
| 39  | `program::Bass::Synth2`              | Synth Bass 2            |
| 40  | `program::Strings::Violin`           | Violin                  |
| 41  | `program::Strings::Viola`            | Viola                   |
| 42  | `program::Strings::Cello`            | Cello                   |
| 43  | `program::Strings::Contrabass`       | Contrabass              |
| 44  | `program::Strings::Tremolo`          | Tremolo Strings         |
| 45  | `program::Strings::Pizzicato`        | Pizzicato Strings       |
| 46  | `program::Strings::Harp`             | Orchestral Harp         |
| 47  | `program::Strings::Timpani`          | Timpani                 |
| 48  | `program::Ensemble::String1`         | String Ensemble 1       |
| 49  | `program::Ensemble::String2`         | String Ensemble 2       |
| 50  | `program::Ensemble::SynthStrings1`   | Synth Strings 1         |
| 51  | `program::Ensemble::SynthStrings2`   | Synth Strings 2         |
| 52  | `program::Ensemble::ChoirAahs`       | Choir Aahs              |
| 53  | `program::Ensemble::VoiceOohs`       | Voice Oohs              |
| 54  | `program::Ensemble::SynthVoice`      | Synth Voice             |
| 55  | `program::Ensemble::OrchestraHit`    | Orchestra Hit           |
| 56  | `program::Brass::Trumpet`            | Trumpet                 |
| 57  | `program::Brass::Trombone`           | Trombone                |
| 58  | `program::Brass::Tuba`               | Tuba                    |
| 59  | `program::Brass::MutedTrumpet`       | Muted Trumpet           |
| 60  | `program::Brass::FrenchHorn`         | French Horn             |
| 61  | `program::Brass::BrassSection`       | Brass Section           |
| 62  | `program::Brass::SynthBrass1`        | Synth Brass 1           |
| 63  | `program::Brass::SynthBrass2`        | Synth Brass 2           |
| 64  | `program::Reed::SopranoSax`          | Soprano Sax             |
| 65  | `program::Reed::AltoSax`             | Alto Sax                |
| 66  | `program::Reed::TenorSax`            | Tenor Sax               |
| 67  | `program::Reed::BaritoneSax`         | Baritone Sax            |
| 68  | `program::Reed::Oboe`                | Oboe                    |
| 69  | `program::Reed::EnglishHorn`         | English Horn            |
| 70  | `program::Reed::Bassoon`             | Bassoon                 |
| 71  | `program::Reed::Clarinet`            | Clarinet                |
| 72  | `program::Pipe::Piccolo`             | Piccolo                 |
| 73  | `program::Pipe::Flute`               | Flute                   |
| 74  | `program::Pipe::Recorder`            | Recorder                |
| 75  | `program::Pipe::PanFlute`            | Pan Flute               |
| 76  | `program::Pipe::BlownBottle`         | Blown Bottle            |
| 77  | `program::Pipe::Shakuhachi`          | Shakuhachi              |
| 78  | `program::Pipe::Whistle`             | Whistle                 |
| 79  | `program::Pipe::Ocarina`             | Ocarina                 |
| 80  | `program::SynthLead::Square`         | Lead 1 (square)         |
| 81  | `program::SynthLead::Saw`            | Lead 2 (sawtooth)       |
| 82  | `program::SynthLead::Calliope`       | Lead 3 (calliope)       |
| 83  | `program::SynthLead::Chiff`          | Lead 4 (chiff)          |
| 84  | `program::SynthLead::Charang`        | Lead 5 (charang)        |
| 85  | `program::SynthLead::Voice`          | Lead 6 (voice)          |
| 86  | `program::SynthLead::Fifths`         | Lead 7 (fifths)         |
| 87  | `program::SynthLead::BassLead`       | Lead 8 (bass + lead)    |
| 88  | `program::SynthPad::NewAge`          | Pad 1 (new age)         |
| 89  | `program::SynthPad::Warm`            | Pad 2 (warm)            |
| 90  | `program::SynthPad::Polysynth`       | Pad 3 (polysynth)       |
| 91  | `program::SynthPad::Choir`           | Pad 4 (choir)           |
| 92  | `program::SynthPad::Bowed`           | Pad 5 (bowed)           |
| 93  | `program::SynthPad::Metallic`        | Pad 6 (metallic)        |
| 94  | `program::SynthPad::Halo`            | Pad 7 (halo)            |
| 95  | `program::SynthPad::Sweep`           | Pad 8 (sweep)           |
| 96  | `program::SynthFx::Rain`             | FX 1 (rain)             |
| 97  | `program::SynthFx::Soundtrack`       | FX 2 (soundtrack)       |
| 98  | `program::SynthFx::Crystal`          | FX 3 (crystal)          |
| 99  | `program::SynthFx::Atmosphere`       | FX 4 (atmosphere)       |
| 100 | `program::SynthFx::Brightness`       | FX 5 (brightness)       |
| 101 | `program::SynthFx::Goblins`          | FX 6 (goblins)          |
| 102 | `program::SynthFx::Echoes`           | FX 7 (echoes)           |
| 103 | `program::SynthFx::SciFi`            | FX 8 (sci-fi)           |
| 104 | `program::World::Sitar`              | Sitar                   |
| 105 | `program::World::Banjo`              | Banjo                   |
| 106 | `program::World::Shamisen`           | Shamisen                |
| 107 | `program::World::Koto`               | Koto                    |
| 108 | `program::World::Kalimba`            | Kalimba                 |
| 109 | `program::World::Bagpipe`            | Bagpipe                 |
| 110 | `program::World::Fiddle`             | Fiddle                  |
| 111 | `program::World::Shanai`             | Shanai                  |
| 112 | `program::Percussive::TinkleBell`    | Tinkle Bell             |
| 113 | `program::Percussive::Agogo`         | Agogo                   |
| 114 | `program::Percussive::SteelDrums`    | Steel Drums             |
| 115 | `program::Percussive::Woodblock`     | Woodblock               |
| 116 | `program::Percussive::TaikoDrum`     | Taiko Drum              |
| 117 | `program::Percussive::MelodicTom`    | Melodic Tom             |
| 118 | `program::Percussive::SynthDrum`     | Synth Drum              |
| 119 | `program::Percussive::ReverseCymbal` | Reverse Cymbal          |
| 120 | `program::SoundFx::GuitarFretNoise`  | Guitar Fret Noise       |
| 121 | `program::SoundFx::BreathNoise`      | Breath Noise            |
| 122 | `program::SoundFx::Seashore`         | Seashore                |
| 123 | `program::SoundFx::BirdTweet`        | Bird Tweet              |
| 124 | `program::SoundFx::TelephoneRing`    | Telephone Ring          |
| 125 | `program::SoundFx::Helicopter`       | Helicopter              |
| 126 | `program::SoundFx::Applause`         | Applause                |
| 127 | `program::SoundFx::Gunshot`          | Gunshot                 |

Drum kit helpers use normal GM drum-kit program values.
Use them with `MidiChannel::DRUMS` when target synth or soundfont supports drum kits.

| Num | Helper                         | Kit        |
| --- | ------------------------------ | ---------- |
| 0   | `program::DrumKit::Standard`   | Standard   |
| 8   | `program::DrumKit::Room`       | Room       |
| 16  | `program::DrumKit::Power`      | Power      |
| 24  | `program::DrumKit::Electronic` | Electronic |
| 25  | `program::DrumKit::Analog`     | Analog     |
| 32  | `program::DrumKit::Jazz`       | Jazz       |
| 40  | `program::DrumKit::Brush`      | Brush      |
| 48  | `program::DrumKit::Orchestra`  | Orchestra  |
| 56  | `program::DrumKit::Sfx`        | SFX        |

## Spatial Audio

Spatial audio starts in script API, then moves through runtime propagation.

Entrypoints:

- `ctx.res.Audio()` for point `Audio2D` and `Audio3D`
- `ctx.run.Audio()` for node-attached runtime audio

Use point audio for impacts, pickups, doors, switches, and other one-shot sounds at a fixed position.
Use attached audio for loops or held notes that follow a scene node.
Use `audio_play!(res, audio_2d_or_3d)` for master point playback.
Use `audio_play!(res, bus, audio_2d_or_3d)` for bus point playback.

Listener source:

- active `Camera2D` for 2D
- active `Camera3D` for 3D

Runtime propagation inputs:

- source position and range
- audio direction mode
- listener transform
- listener `audio_options` on active camera
- audio material fields on physics nodes
- `AudioMask2D`
- `AudioMask3D`
- `AudioPortal2D` and `AudioPortal3D`
- audio effect zones

Propagation output becomes `SpatialAudioParams`.
The runtime sends those params to `perro_pawdio`.
`perro_pawdio` applies pan and volume to the sink.
It also applies low-pass, EQ, compression, echo, reverb send, reflection, and occlusion in DSP.

Propagation runs only while active positional or attached spatial sounds exist.
If no active spatial sounds exist, no audio ray work runs that frame.

## Listener Options

Active `Camera2D` and `Camera3D` can set `audio_options`.
Listener options apply after audio zones and before explicit sound effects.
`audio_mask` ignores matching emitted `audio_layer`.
Default `audio_mask = []` ignores nothing, so listener effects apply to all emitted audio layers.

Scene example:

```text
[Camera3D]
    audio_options = {
        audio_mask = [1, 3],
        effects = [
            { reverb_send: 0.4, echo: 0.1, dampening: 0.2 }
        ]
    }
[/Camera3D]
```

Shorthand fields also work on cameras:

```text
[Camera2D]
    audio_mask = [2]
    reverb_send = 0.6
    echo = 0.2
    dampening = 0.3
[/Camera2D]
```

## Audio Direction

Spatial audio direction lives in `SpatialAudioOptions`.
Point audio direction lives on `Audio2D` and `Audio3D`.
Default-style setup uses `AudioDirection::Omni`.
Omni ignores direction and radiates the same in every direction.

Direction modes:

- `AudioDirection::Omni` - emits outward in all directions
- `AudioDirection::Directional(forward)`: strongest toward `forward`.
- `AudioDirection::InverseDirectional(forward)`: strongest opposite `forward`.
- `AudioDirection::Bidirectional(forward)`: strongest toward `forward` and `-forward`.

Point audio:

- `Audio2D::new(source, position, range)`
- `Audio3D::new(source, position, range)`

Attached audio:

- `SpatialAudioOptions` sets node-attached range, audio layer, propagation, and direction.
- `direction_2d` takes `AudioDirection<Vector2>`.
- `direction_3d` takes `AudioDirection<Vector3>`.
- struct fields set attached/node direction.
- direction vectors are normalized by the runtime.
- attached 2D directional audio uses node forward from node rotation.
- attached 3D directional audio uses node forward from node rotation.
- explicit direction is fallback for point use and non-attached use.

Point 2D example:

```rust
let hit = Audio2D {
    audio: Audio::new("res://audio/hit.wav"),
    position: Vector2::new(128.0, 64.0),
    range: 512.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction: None,
};

let _ = audio_play!(ctx.res, audio_bus!("sfx"), hit);
```

Point 3D directional example:

```rust
let horn = Audio3D::new(
    "res://audio/horn.wav",
    Vector3::new(0.0, 1.0, -4.0),
    80.0,
);

let horn = Audio3D {
    direction: Some(AudioDirection::Bidirectional(Vector3::new(0.0, 0.0, -1.0))),
    ..horn
};

let _ = audio_play!(ctx.res, audio_bus!("sfx"), horn);
```

Attached loop example:

```rust
let audio = RuntimeAudio {
    source: "res://audio/engine_loop.ogg",
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
    range: 80.0,
    audio_layer: BitMask::ALL,
    enable_propagation: true,
    direction_2d: AudioDirection::Omni,
    direction_3d: AudioDirection::Omni,
};

let _ = ctx.run.Audio().play_attached_bus(audio_bus!("ambience"), audio, vehicle_node, spatial);
```

## Audio Materials

Physics bodies and areas participate in audio propagation when `audio_interaction` is set.
Use `audio_interaction = none` to turn it off.

`AudioInteraction` fields:

- `audio_interaction`
- `absorption`
- `reflection`
- `transmission`
- `diffusion`
- `low_pass_strength`
- `thickness_multiplier`
- `audio_mask`
  - Uses `BitMask`; see `docs/scripting/bitmask.md`.

Use audio masks, effect zones, and portals for invisible or non-physical audio geometry.
Emitted sounds use `audio_layer`; audio geometry uses `audio_mask` to decide which layers to ignore.
Default `audio_mask = []` means audio geometry affects all emitted audio layers.

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
