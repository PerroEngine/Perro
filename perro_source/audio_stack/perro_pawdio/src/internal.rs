use perro_ids::AudioBusID;
use rodio::SpatialSink;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::types::{AudioCompression, AudioEq, AudioPan, AudioPlaybackRequest, SpatialAudioParams};

pub(crate) struct Playback {
    pub(crate) id: u64,
    pub(crate) source: String,
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
    pub(crate) cache: HashMap<String, CachedAudioAsset>,
    pub(crate) cache_bytes: usize,
}
#[derive(Clone)]
pub(crate) struct OwnedAudioPlaybackRequest {
    pub(crate) id: u64,
    pub(crate) source: String,
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
        Self {
            source: value.source.to_string(),
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
        source: String,
        reserved: bool,
    },
    DropAsset {
        source: String,
    },
    Play {
        request: OwnedAudioPlaybackRequest,
    },
    Stop {
        source: String,
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
        source: String,
        reply: Sender<Option<f32>>,
    },
}

#[derive(Clone)]
pub(crate) struct CachedAudioAsset {
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) duration: Option<Duration>,
    pub(crate) duration_known: bool,
    pub(crate) reserved: bool,
    pub(crate) last_touched: Instant,
}
