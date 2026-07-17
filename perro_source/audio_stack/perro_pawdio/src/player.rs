use perro_ids::AudioBusID;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, SpatialSink};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::codec::decode_static_pawdio;
use crate::dsp::{DspControl, DspParams, DspSource};
#[cfg(feature = "profile")]
use crate::internal::SourceLoadKind;
use crate::internal::{
    AudioState, BuiltInMidiMixerPlayback, BusState, CachedAudioAsset, CachedMidiFile, CachedPcm,
    CachedSoundFont, MidiMixerKey, MidiPlayback, Playback, SoundFontMidiMixerKey,
    SoundFontMidiMixerPlayback, SourceLoadStats,
};
use crate::mic::MicClip;
use crate::midi::{
    BuiltInMidiMixerSource, BuiltInMidiSource, MidiControl, MidiFileRequest, MidiMixerControl,
    MidiMixerNote, MidiNoteRequest, MidiSound, RustyFileSource, RustyNoteMixerSource,
    SoundFontMixerControl, SoundFontMixerNote, parse_built_in_midi_file,
};
use crate::types::{AudioPan, AudioPlaybackRequest, SpatialAudioParams};

type LoadedAudioAsset = (Arc<[u8]>, Arc<str>, u64, u64, bool, SourceLoadStats);

struct MidiSinkActivation {
    id: u64,
    source: Option<Arc<str>>,
    bus_id: Option<AudioBusID>,
    volume: f32,
    pan: AudioPan,
    control: crossbeam_channel::Sender<MidiControl>,
    dsp: Arc<DspControl>,
    sink: SpatialSink,
}

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    state: Mutex<AudioState>,
    static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
}

impl BarkPlayer {
    const CACHE_SOFT_LIMIT_BYTES: usize = 128 * 1024 * 1024;
    // Clips at or under this length keep their decoded PCM cached so repeated
    // plays skip the decoder; longer clips stream-decode per play.
    const PCM_CACHE_MAX_SECONDS: usize = 12;
    const CACHE_EVICT_SWEEP_INTERVAL: Duration = Duration::from_millis(100);
    const UNRESERVED_TTL_FACTOR: f32 = 2.0;
    const UNRESERVED_TTL_FALLBACK: Duration = Duration::from_secs(1);
    const UNRESERVED_TTL_MIN: Duration = Duration::from_millis(250);

    pub fn new(static_audio_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|err| format!("audio output init failed: {err}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            static_audio_lookup,
            state: Mutex::new(AudioState {
                master_volume: 1.0,
                buses: HashMap::new(),
                playbacks: Vec::new(),
                midi_playbacks: Vec::new(),
                built_in_midi_mixers: Vec::new(),
                built_in_midi_mixer_index: HashMap::new(),
                built_in_midi_notes: HashMap::new(),
                soundfont_midi_mixers: Vec::new(),
                soundfont_midi_mixer_index: HashMap::new(),
                soundfont_midi_notes: HashMap::new(),
                cache: HashMap::new(),
                soundfonts: HashMap::new(),
                midi_files: HashMap::new(),
                cache_bytes: 0,
                next_cache_epoch: 1,
                last_evict_sweep: Instant::now(),
            }),
        })
    }

    fn decode_duration_from_cached_bytes(bytes: Arc<[u8]>) -> Option<Duration> {
        let cursor = Cursor::new(bytes);
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader).ok()?;
        if let Some(duration) = decoder.total_duration() {
            return Some(duration);
        }
        let channels = decoder.channels() as f64;
        let sample_rate = decoder.sample_rate() as f64;
        if channels <= 0.0 || sample_rate <= 0.0 {
            return None;
        }
        let sample_count = decoder.count() as f64;
        if sample_count <= 0.0 {
            return None;
        }
        let seconds = sample_count / (channels * sample_rate);
        Some(Duration::from_secs_f64(seconds))
    }
}

mod cache;
mod midi_player;
mod pcm_source;
mod playback;
use pcm_source::{CachedPcmSource, append_with_trims};
