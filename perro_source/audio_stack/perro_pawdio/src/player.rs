use perro_ids::AudioBusID;
use rodio::buffer::SamplesBuffer;
use rodio::source::UniformSourceIterator;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, SpatialSink};
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::codec::decode_static_pawdio;
use crate::dsp::{DspControl, DspParams, DspSource};
#[cfg(feature = "profile")]
use crate::internal::SourceLoadKind;
use crate::internal::{
    AudioState, BuiltInMidiMixerPlayback, BusState, CachedAudioAsset, CachedMidiFile, CachedPcm,
    CachedSoundFont, MidiMixerKey, MidiNoteReleaseTarget, MidiPlayback, Playback,
    SoundFontMidiMixerKey, SoundFontMidiMixerPlayback, SourceLoadStats, midi_mixer_is_idle,
    take_midi_note_target, track_held_midi_note,
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
    // Device rate the mixer runs at. Cached PCM is resampled into the cache at
    // this rate once, so repeated plays feed the mixer straight through
    // instead of running rodio's per-sample rate converter every play.
    output_sample_rate: u32,
    state: Mutex<AudioState>,
    static_audio_lookup: Option<fn(u64) -> &'static [u8]>,
}

impl BarkPlayer {
    // Soft cap on compressed-bytes + decoded-PCM the cache pins. Unreserved
    // idle entries evict above this; 64MiB comfortably holds a typical game's
    // active sfx set (a 12s stereo 48k clip decodes to ~4.6MiB).
    const CACHE_SOFT_LIMIT_BYTES: usize = 64 * 1024 * 1024;
    // Clips at or under this length keep their decoded PCM cached so repeated
    // plays skip the decoder; longer clips stream-decode per play.
    const PCM_CACHE_MAX_SECONDS: usize = 12;
    const CACHE_EVICT_SWEEP_INTERVAL: Duration = Duration::from_millis(100);
    const UNRESERVED_TTL_FACTOR: f32 = 2.0;
    const UNRESERVED_TTL_FALLBACK: Duration = Duration::from_secs(1);
    const UNRESERVED_TTL_MIN: Duration = Duration::from_millis(250);

    pub fn new(static_audio_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        let (stream, handle, output_sample_rate) = open_default_output()?;
        Ok(Self {
            _stream: stream,
            handle,
            output_sample_rate,
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
                pending_audio_loads: HashMap::new(),
                soundfonts: HashMap::new(),
                midi_files: HashMap::new(),
                cache_bytes: 0,
                soundfont_bytes: 0,
                next_cache_epoch: 1,
                last_evict_sweep: Instant::now(),
                volumes_dirty: false,
                speeds_dirty: false,
            }),
        })
    }

    fn decode_duration_from_cached_bytes(bytes: Arc<[u8]>) -> Option<Duration> {
        // The bytes are already resident: a `Cursor` is the reader, wrapping it
        // in a `BufReader` just copies every block a second time.
        let decoder = Decoder::new(Cursor::new(bytes)).ok()?;
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

// Mirrors `OutputStream::try_default` (default device, then any other output
// device) but keeps the config it opened with, so the mixer's sample rate is
// known instead of guessed. rodio can still fall back to another config
// internally; a wrong guess only costs the resample it would have done anyway.
fn open_default_output() -> Result<(OutputStream, OutputStreamHandle, u32), String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    fn open(device: &cpal::Device) -> Option<(OutputStream, OutputStreamHandle, u32)> {
        let config = device.default_output_config().ok()?;
        let rate = config.sample_rate().0.max(1);
        let (stream, handle) = OutputStream::try_from_device_config(device, config).ok()?;
        Some((stream, handle, rate))
    }

    let host = cpal::default_host();
    if let Some(device) = host.default_output_device()
        && let Some(opened) = open(&device)
    {
        return Ok(opened);
    }
    host.output_devices()
        .map_err(|err| format!("audio output init failed: {err}"))?
        .find_map(|device| open(&device))
        .ok_or_else(|| "audio output init failed: no usable output device".to_string())
}

mod cache;
mod midi_player;
mod pcm_source;
mod playback;
use pcm_source::{append_cached_with_trims, append_with_trims};
