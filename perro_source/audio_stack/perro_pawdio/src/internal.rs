use crossbeam_channel::Sender;
use perro_ids::AudioBusID;
use rodio::SpatialSink;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::midi::{
    BuiltInMidiFileData, MidiControl, MidiFileRequest, MidiMixerControl, MidiNoteOptions,
    MidiNoteRequest, MidiSong, Note, SoundFontMixerControl,
};
use crate::types::{AudioCompression, AudioEq, AudioPan, AudioPlaybackRequest, SpatialAudioParams};

pub(crate) struct Playback {
    pub(crate) id: u64,
    pub(crate) source: Arc<str>,
    pub(crate) source_hash: u64,
    pub(crate) asset_epoch: u64,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) looped: bool,
    pub(crate) base_volume: f32,
    pub(crate) speed: f32,
    pub(crate) pan: AudioPan,
    pub(crate) low_pass: f32,
    pub(crate) reverb_send: f32,
    pub(crate) echo: f32,
    pub(crate) reflection: f32,
    pub(crate) occlusion: f32,
    pub(crate) eq: AudioEq,
    pub(crate) compression: AudioCompression,
    pub(crate) from_start: f32,
    pub(crate) from_end: f32,
    pub(crate) sink: SpatialSink,
}

pub(crate) struct MidiPlayback {
    pub(crate) id: u64,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) pan: AudioPan,
    pub(crate) low_pass: f32,
    pub(crate) reverb_send: f32,
    pub(crate) echo: f32,
    pub(crate) reflection: f32,
    pub(crate) occlusion: f32,
    pub(crate) eq: AudioEq,
    pub(crate) compression: AudioCompression,
    pub(crate) source: Option<Arc<str>>,
    pub(crate) control: crossbeam_channel::Sender<MidiControl>,
    pub(crate) sink: SpatialSink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MidiMixerKey {
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) pan_x: u32,
    pub(crate) pan_y: u32,
    pub(crate) pan_z: u32,
}

impl MidiMixerKey {
    pub(crate) fn new(bus_id: Option<AudioBusID>, pan: AudioPan) -> Self {
        let pan = pan.clamped();
        Self {
            bus_id,
            pan_x: pan.x.to_bits(),
            pan_y: pan.y.to_bits(),
            pan_z: pan.z.to_bits(),
        }
    }

    pub(crate) fn pan(self) -> AudioPan {
        AudioPan {
            x: f32::from_bits(self.pan_x),
            y: f32::from_bits(self.pan_y),
            z: f32::from_bits(self.pan_z),
        }
    }
}

pub(crate) struct BuiltInMidiMixerPlayback {
    pub(crate) key: MidiMixerKey,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) control: Sender<MidiMixerControl>,
    pub(crate) sink: SpatialSink,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SoundFontMidiMixerKey {
    pub(crate) source_hash: u64,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) pan_x: u32,
    pub(crate) pan_y: u32,
    pub(crate) pan_z: u32,
}

impl SoundFontMidiMixerKey {
    pub(crate) fn new(source_hash: u64, bus_id: Option<AudioBusID>, pan: AudioPan) -> Self {
        let pan = pan.clamped();
        Self {
            source_hash,
            bus_id,
            pan_x: pan.x.to_bits(),
            pan_y: pan.y.to_bits(),
            pan_z: pan.z.to_bits(),
        }
    }

    pub(crate) fn pan(self) -> AudioPan {
        AudioPan {
            x: f32::from_bits(self.pan_x),
            y: f32::from_bits(self.pan_y),
            z: f32::from_bits(self.pan_z),
        }
    }
}

pub(crate) struct SoundFontMidiMixerPlayback {
    pub(crate) key: SoundFontMidiMixerKey,
    pub(crate) source: Arc<str>,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) control: Sender<SoundFontMixerControl>,
    pub(crate) sink: SpatialSink,
}

#[derive(Clone, Copy)]
pub(crate) struct BusState {
    pub(crate) volume: f32,
    pub(crate) speed: f32,
    pub(crate) paused: bool,
}

pub(crate) struct AudioState {
    pub(crate) master_volume: f32,
    pub(crate) buses: HashMap<AudioBusID, BusState>,
    pub(crate) playbacks: Vec<Playback>,
    pub(crate) midi_playbacks: Vec<MidiPlayback>,
    pub(crate) built_in_midi_mixers: Vec<BuiltInMidiMixerPlayback>,
    pub(crate) built_in_midi_mixer_index: HashMap<MidiMixerKey, usize>,
    pub(crate) built_in_midi_notes: HashMap<u64, MidiMixerKey>,
    pub(crate) soundfont_midi_mixers: Vec<SoundFontMidiMixerPlayback>,
    pub(crate) soundfont_midi_mixer_index: HashMap<SoundFontMidiMixerKey, usize>,
    pub(crate) soundfont_midi_notes: HashMap<u64, SoundFontMidiMixerKey>,
    pub(crate) cache: HashMap<u64, CachedAudioAsset>,
    pub(crate) soundfonts: HashMap<u64, CachedSoundFont>,
    pub(crate) midi_files: HashMap<u64, CachedMidiFile>,
    pub(crate) cache_bytes: usize,
    pub(crate) next_cache_epoch: u64,
    pub(crate) last_evict_sweep: Instant,
}

pub(crate) struct CachedSoundFont {
    pub(crate) source: Arc<str>,
    pub(crate) font: std::sync::Arc<rustysynth::SoundFont>,
}

pub(crate) struct CachedMidiFile {
    pub(crate) source: Arc<str>,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) built_in: Option<Arc<BuiltInMidiFileData>>,
}
#[derive(Clone)]
pub(crate) struct OwnedAudioPlaybackRequest {
    pub(crate) id: u64,
    pub(crate) source: Arc<str>,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) looped: bool,
    pub(crate) volume: f32,
    pub(crate) speed: f32,
    pub(crate) pan: AudioPan,
    pub(crate) low_pass: f32,
    pub(crate) reverb_send: f32,
    pub(crate) echo: f32,
    pub(crate) reflection: f32,
    pub(crate) occlusion: f32,
    pub(crate) eq: AudioEq,
    pub(crate) compression: AudioCompression,
    pub(crate) from_start: f32,
    pub(crate) from_end: f32,
}

impl From<AudioPlaybackRequest<'_>> for OwnedAudioPlaybackRequest {
    fn from(value: AudioPlaybackRequest<'_>) -> Self {
        Self::from_request_with_source(value, Arc::from(value.source))
    }
}

impl OwnedAudioPlaybackRequest {
    pub(crate) fn from_request_with_source(
        value: AudioPlaybackRequest<'_>,
        source: Arc<str>,
    ) -> Self {
        Self {
            source,
            id: value.id,
            bus_id: value.bus_id,
            looped: value.looped,
            volume: value.volume,
            speed: value.speed,
            pan: value.pan,
            low_pass: value.low_pass,
            reverb_send: value.reverb_send,
            echo: value.echo,
            reflection: value.reflection,
            occlusion: value.occlusion,
            eq: value.eq,
            compression: value.compression,
            from_start: value.from_start,
            from_end: value.from_end,
        }
    }
}

#[derive(Clone)]
pub(crate) enum OwnedMidiSound {
    BuiltIn,
    SoundFont(Arc<str>),
}

#[derive(Clone)]
pub(crate) struct OwnedMidiNoteOptions {
    pub(crate) velocity: u8,
    pub(crate) sustain: Duration,
    pub(crate) channel: crate::midi::MidiChannel,
    pub(crate) program: crate::midi::MidiProgram,
    pub(crate) sound: OwnedMidiSound,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) volume: f32,
    pub(crate) pan: AudioPan,
}

#[derive(Clone)]
pub(crate) struct OwnedMidiNoteRequest {
    pub(crate) id: u64,
    pub(crate) note: Note,
    pub(crate) options: OwnedMidiNoteOptions,
    pub(crate) held: bool,
}

#[derive(Clone)]
pub(crate) struct OwnedMidiFileRequest {
    pub(crate) id: u64,
    pub(crate) source: Arc<str>,
    pub(crate) sound: OwnedMidiSound,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) volume: f32,
    pub(crate) looped: bool,
    pub(crate) pan: AudioPan,
}

impl OwnedMidiSound {
    pub(crate) fn from_sound(sound: crate::midi::MidiSound<'_>) -> Self {
        match sound {
            crate::midi::MidiSound::BuiltIn => Self::BuiltIn,
            crate::midi::MidiSound::SoundFont(source) => Self::SoundFont(Arc::from(source)),
        }
    }

    pub(crate) fn as_sound(&self) -> crate::midi::MidiSound<'_> {
        match self {
            Self::BuiltIn => crate::midi::MidiSound::BuiltIn,
            Self::SoundFont(source) => crate::midi::MidiSound::SoundFont(source.as_ref()),
        }
    }
}

impl OwnedMidiNoteOptions {
    pub(crate) fn from_options(value: MidiNoteOptions<'_>) -> Self {
        Self {
            velocity: value.velocity,
            sustain: value.sustain,
            channel: value.channel,
            program: value.program,
            sound: OwnedMidiSound::from_sound(value.sound),
            bus_id: value.bus_id,
            volume: value.volume,
            pan: value.pan,
        }
    }

    pub(crate) fn as_options(&self) -> MidiNoteOptions<'_> {
        MidiNoteOptions {
            velocity: self.velocity,
            sustain: self.sustain,
            channel: self.channel,
            program: self.program,
            sound: self.sound.as_sound(),
            bus_id: self.bus_id,
            volume: self.volume,
            pan: self.pan,
        }
    }
}

impl OwnedMidiNoteRequest {
    pub(crate) fn from_request(value: MidiNoteRequest<'_>) -> Self {
        Self {
            id: value.id,
            note: value.note,
            options: OwnedMidiNoteOptions::from_options(value.options),
            held: value.held,
        }
    }

    pub(crate) fn as_request(&self) -> MidiNoteRequest<'_> {
        MidiNoteRequest {
            id: self.id,
            note: self.note,
            options: self.options.as_options(),
            held: self.held,
        }
    }
}

impl OwnedMidiFileRequest {
    pub(crate) fn from_request(value: MidiFileRequest<'_>) -> Self {
        Self {
            id: value.id,
            source: Arc::from(value.song.source),
            sound: OwnedMidiSound::from_sound(value.song.sound),
            bus_id: value.song.bus_id,
            volume: value.song.volume,
            looped: value.song.looped,
            pan: value.pan,
        }
    }

    pub(crate) fn as_request(&self) -> MidiFileRequest<'_> {
        MidiFileRequest {
            id: self.id,
            song: MidiSong {
                source: self.source.as_ref(),
                sound: self.sound.as_sound(),
                bus_id: self.bus_id,
                volume: self.volume,
                looped: self.looped,
            },
            pan: self.pan,
        }
    }
}

#[cfg(feature = "profile")]
#[derive(Clone, Copy)]
pub(crate) enum SourceLoadKind {
    Cache,
    Static,
    Disk,
}

#[cfg(feature = "profile")]
#[derive(Clone, Copy)]
pub(crate) struct SourceLoadStats {
    pub(crate) kind: SourceLoadKind,
    pub(crate) static_lookup: Duration,
    pub(crate) pawdio_decompress: Duration,
    pub(crate) disk_read: Duration,
}

#[cfg(not(feature = "profile"))]
#[derive(Clone, Copy)]
pub(crate) struct SourceLoadStats;

impl SourceLoadStats {
    pub(crate) const fn cache_hit() -> Self {
        #[cfg(feature = "profile")]
        {
            Self {
                kind: SourceLoadKind::Cache,
                static_lookup: Duration::ZERO,
                pawdio_decompress: Duration::ZERO,
                disk_read: Duration::ZERO,
            }
        }
        #[cfg(not(feature = "profile"))]
        {
            Self
        }
    }
}

pub(crate) enum AudioCommand {
    Load {
        source: Arc<str>,
        reserved: bool,
    },
    DropAsset {
        source: Arc<str>,
    },
    Play {
        request: OwnedAudioPlaybackRequest,
    },
    Stop {
        source: Arc<str>,
    },
    StopMatch {
        request: OwnedAudioPlaybackRequest,
    },
    StopPlayback {
        id: u64,
    },
    UpdateSpatial {
        id: u64,
        params: SpatialAudioParams,
    },
    StopAll,
    SetMasterVolume {
        volume: f32,
    },
    SetBusVolume {
        bus_id: AudioBusID,
        volume: f32,
    },
    SetBusSpeed {
        bus_id: AudioBusID,
        speed: f32,
    },
    PauseBus {
        bus_id: AudioBusID,
    },
    ResumeBus {
        bus_id: AudioBusID,
    },
    StopBus {
        bus_id: AudioBusID,
    },
    SourceLength {
        source: Arc<str>,
        reply: Sender<Option<f32>>,
    },
    LoadSoundFont {
        source: Arc<str>,
    },
    LoadMidiFile {
        source: Arc<str>,
    },
    MidiNote {
        request: OwnedMidiNoteRequest,
    },
    MidiNotes {
        requests: Vec<OwnedMidiNoteRequest>,
    },
    MidiFile {
        request: OwnedMidiFileRequest,
    },
    MidiRelease {
        id: u64,
    },
}

#[derive(Clone)]
pub(crate) struct CachedAudioAsset {
    pub(crate) source: Arc<str>,
    pub(crate) source_hash: u64,
    pub(crate) asset_epoch: u64,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) duration: Option<Duration>,
    pub(crate) duration_known: bool,
    pub(crate) reserved: bool,
    pub(crate) active_uses: usize,
    pub(crate) last_touched: Instant,
}
