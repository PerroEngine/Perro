#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::dsp::DspControl;
use crossbeam_channel::Sender;
use perro_ids::AudioBusID;
use rodio::SpatialSink;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::mic::MicClip;
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
    pub(crate) dsp: std::sync::Arc<DspControl>,
    pub(crate) from_start: f32,
    pub(crate) from_end: f32,
    pub(crate) sink: SpatialSink,
}

pub(crate) struct MidiPlayback {
    pub(crate) id: u64,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) pan: AudioPan,
    pub(crate) dsp: std::sync::Arc<DspControl>,
    pub(crate) source: Option<Arc<str>>,
    pub(crate) control: crossbeam_channel::Sender<MidiControl>,
    pub(crate) sink: SpatialSink,
}

// Mixer-key pan components snap to this grid. Raw float pans made every
// position of a moving spatial emitter mint its own mixer (sink + control
// channel + mixer source) that nothing ever freed; 0.25 is far finer than the
// audible pan step, and the sink still gets the exact pan on reuse.
pub(crate) const MIXER_PAN_QUANT_STEP: f32 = 0.25;

// Idle mixers are pruned this long after their last note is expected to have
// finished, so a pan sweep leaves at most a few short-lived mixers behind.
pub(crate) const MIXER_IDLE_TTL: Duration = Duration::from_secs(5);

#[inline]
pub(crate) fn quantize_mixer_pan(value: f32) -> i16 {
    // Callers clamp to -1..1 first, so the quantized value always fits i16.
    (value / MIXER_PAN_QUANT_STEP).round() as i16
}

#[inline]
pub(crate) fn dequantize_mixer_pan(value: i16) -> f32 {
    value as f32 * MIXER_PAN_QUANT_STEP
}

// A mixer may be dropped once every note it was handed is past its sustain
// window plus the idle grace, and no tracked (held) note still names it.
pub(crate) fn midi_mixer_is_idle(busy_until: Instant, now: Instant, tracked_notes: bool) -> bool {
    !tracked_notes && now.saturating_duration_since(busy_until) >= MIXER_IDLE_TTL
}

// Only held notes are tracked for later release/stop. One-shots retire on
// their own, so recording them just grows the map for the process lifetime.
pub(crate) fn track_held_midi_note<K>(map: &mut HashMap<u64, K>, id: u64, key: K, held: bool) {
    if held {
        map.insert(id, key);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MidiMixerKey {
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) pan_x: i16,
    pub(crate) pan_y: i16,
    pub(crate) pan_z: i16,
}

impl MidiMixerKey {
    pub(crate) fn new(bus_id: Option<AudioBusID>, pan: AudioPan) -> Self {
        let pan = pan.clamped();
        Self {
            bus_id,
            pan_x: quantize_mixer_pan(pan.x),
            pan_y: quantize_mixer_pan(pan.y),
            pan_z: quantize_mixer_pan(pan.z),
        }
    }

    pub(crate) fn pan(self) -> AudioPan {
        AudioPan {
            x: dequantize_mixer_pan(self.pan_x),
            y: dequantize_mixer_pan(self.pan_y),
            z: dequantize_mixer_pan(self.pan_z),
        }
    }
}

pub(crate) struct BuiltInMidiMixerPlayback {
    pub(crate) key: MidiMixerKey,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) dsp: std::sync::Arc<DspControl>,
    pub(crate) control: Sender<MidiMixerControl>,
    pub(crate) sink: SpatialSink,
    // Latest instant any note handed to this mixer can still be sounding.
    pub(crate) busy_until: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SoundFontMidiMixerKey {
    pub(crate) id: perro_ids::SoundFontID,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) pan_x: i16,
    pub(crate) pan_y: i16,
    pub(crate) pan_z: i16,
}

impl SoundFontMidiMixerKey {
    pub(crate) fn new(
        id: perro_ids::SoundFontID,
        bus_id: Option<AudioBusID>,
        pan: AudioPan,
    ) -> Self {
        let pan = pan.clamped();
        Self {
            id,
            bus_id,
            pan_x: quantize_mixer_pan(pan.x),
            pan_y: quantize_mixer_pan(pan.y),
            pan_z: quantize_mixer_pan(pan.z),
        }
    }

    pub(crate) fn pan(self) -> AudioPan {
        AudioPan {
            x: dequantize_mixer_pan(self.pan_x),
            y: dequantize_mixer_pan(self.pan_y),
            z: dequantize_mixer_pan(self.pan_z),
        }
    }
}

pub(crate) struct SoundFontMidiMixerPlayback {
    pub(crate) key: SoundFontMidiMixerKey,
    pub(crate) bus_id: Option<AudioBusID>,
    pub(crate) base_volume: f32,
    pub(crate) dsp: std::sync::Arc<DspControl>,
    pub(crate) control: Sender<SoundFontMixerControl>,
    pub(crate) sink: SpatialSink,
    // See `BuiltInMidiMixerPlayback::busy_until`.
    pub(crate) busy_until: Instant,
}

// Which mixer map a released/stopped note id came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MidiNoteReleaseTarget {
    BuiltIn(MidiMixerKey),
    SoundFont(SoundFontMidiMixerKey),
}

// Forget a tracked note id and report which mixer owns it. Both maps are
// consulted: a held soundfont note lives in `soundfont_midi_notes`, and
// leaving it there both leaked the entry and dropped its release.
pub(crate) fn take_midi_note_target(
    state: &mut AudioState,
    id: u64,
) -> Option<MidiNoteReleaseTarget> {
    if let Some(key) = state.built_in_midi_notes.remove(&id) {
        return Some(MidiNoteReleaseTarget::BuiltIn(key));
    }
    state
        .soundfont_midi_notes
        .remove(&id)
        .map(MidiNoteReleaseTarget::SoundFont)
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
    // Only in-flight misses; drop invalidates the token without retaining tombstones.
    pub(crate) pending_audio_loads: HashMap<Arc<str>, (u64, usize)>,
    pub(crate) soundfonts: HashMap<perro_ids::SoundFontID, CachedSoundFont>,
    pub(crate) midi_files: HashMap<u64, CachedMidiFile>,
    pub(crate) cache_bytes: usize,
    // Resident parsed-soundfont footprint, tracked separately from
    // `cache_bytes`: fonts have no unload API and no reload path (the bytes
    // variant never touches disk), so they are not evictable and must not
    // shrink the clip cache's effective budget.
    pub(crate) soundfont_bytes: usize,
    pub(crate) next_cache_epoch: u64,
    pub(crate) last_evict_sweep: Instant,
    // Volume/speed refresh walks are batched: setters mark dirty, the worker
    // flushes once its command queue drains, so a burst of volume commands
    // walks the playback lists once instead of per command.
    pub(crate) volumes_dirty: bool,
    pub(crate) speeds_dirty: bool,
}

pub(crate) struct CachedSoundFont {
    pub(crate) source: Arc<str>,
    pub(crate) font: std::sync::Arc<rustysynth::SoundFont>,
    // Real resident-footprint estimate for the parsed font: the i16 wave
    // table rustysynth keeps in memory (its dominant allocation), floored at
    // the source-file size to cover preset/instrument/region structs.
    pub(crate) footprint_bytes: usize,
}

impl CachedSoundFont {
    pub(crate) fn estimate_footprint(font: &rustysynth::SoundFont, source_bytes: usize) -> usize {
        std::mem::size_of_val(font.get_wave_data()).max(source_bytes)
    }
}

pub(crate) struct CachedMidiFile {
    pub(crate) source: Arc<str>,
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) built_in: Option<Arc<BuiltInMidiFileData>>,
    pub(crate) last_touched: Instant,
}

impl CachedMidiFile {
    // Bytes this entry pins in the cache budget. Only the raw file bytes are
    // charged (matching what load added); the parsed built-in data is small
    // and proportional to them.
    pub(crate) fn cache_len(&self) -> usize {
        self.bytes.len()
    }
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

    pub(crate) fn as_request(&self) -> AudioPlaybackRequest<'_> {
        AudioPlaybackRequest {
            id: self.id,
            source: &self.source,
            bus_id: self.bus_id,
            looped: self.looped,
            volume: self.volume,
            speed: self.speed,
            pan: self.pan,
            low_pass: self.low_pass,
            reverb_send: self.reverb_send,
            echo: self.echo,
            reflection: self.reflection,
            occlusion: self.occlusion,
            eq: self.eq,
            compression: self.compression,
            from_start: self.from_start,
            from_end: self.from_end,
        }
    }
}

#[derive(Clone)]
pub(crate) enum OwnedMidiSound {
    BuiltIn,
    SoundFont(perro_ids::SoundFontID),
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
    pub(crate) fn from_sound(sound: crate::midi::MidiSound) -> Self {
        match sound {
            crate::midi::MidiSound::BuiltIn => Self::BuiltIn,
            crate::midi::MidiSound::SoundFont(id) => Self::SoundFont(id),
        }
    }

    pub(crate) fn as_sound(&self) -> crate::midi::MidiSound {
        match self {
            Self::BuiltIn => crate::midi::MidiSound::BuiltIn,
            Self::SoundFont(id) => crate::midi::MidiSound::SoundFont(*id),
        }
    }
}

impl OwnedMidiNoteOptions {
    pub(crate) fn from_options(value: MidiNoteOptions) -> Self {
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

    pub(crate) fn as_options(&self) -> MidiNoteOptions {
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
    pub(crate) fn from_request(value: MidiNoteRequest) -> Self {
        Self {
            id: value.id,
            note: value.note,
            options: OwnedMidiNoteOptions::from_options(value.options),
            held: value.held,
        }
    }

    pub(crate) fn as_request(&self) -> MidiNoteRequest {
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
    Flush {
        reply: Sender<()>,
    },
    Load {
        source: Arc<str>,
        reserved: bool,
    },
    LoadBytes {
        source: Arc<str>,
        bytes: Arc<[u8]>,
        reserved: bool,
    },
    DropAsset {
        source: Arc<str>,
    },
    Play {
        request: OwnedAudioPlaybackRequest,
    },
    PlayClip {
        source: Arc<str>,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: AudioPan,
    },
    PlayStreamClip {
        source: Arc<str>,
        clip: MicClip,
        bus_id: Option<AudioBusID>,
        volume: f32,
        pan: AudioPan,
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
        id: perro_ids::SoundFontID,
        source: Arc<str>,
    },
    LoadSoundFontBytes {
        id: perro_ids::SoundFontID,
        source: Arc<str>,
        bytes: Arc<[u8]>,
    },
    LoadMidiFile {
        source: Arc<str>,
    },
    MidiNote {
        request: OwnedMidiNoteRequest,
    },
    MidiNoteSpatial {
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

// Fully decoded interleaved f32 samples for a short clip. Cached so repeated
// plays (footsteps, gunshots) skip the compressed-bytes decode entirely.
#[derive(Clone)]
pub(crate) struct CachedPcm {
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) samples: Arc<[f32]>,
}

impl CachedPcm {
    pub(crate) fn byte_len(&self) -> usize {
        self.samples.len() * std::mem::size_of::<f32>()
    }

    pub(crate) fn duration(&self) -> Duration {
        let frames = self.samples.len() as u64 / self.channels.max(1) as u64;
        Duration::from_secs_f64(frames as f64 / self.sample_rate.max(1) as f64)
    }
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
    // Decoded PCM for short clips; None until the first play decodes it.
    pub(crate) pcm: Option<Arc<CachedPcm>>,
    // Clip exceeds the PCM cache cap: never retry the full decode.
    pub(crate) pcm_oversized: bool,
}

impl CachedAudioAsset {
    // Bytes this entry pins in the cache budget (compressed + decoded PCM).
    pub(crate) fn cache_len(&self) -> usize {
        self.bytes.len() + self.pcm.as_ref().map_or(0, |pcm| pcm.byte_len())
    }
}

#[cfg(test)]
impl AudioState {
    // Sink-free state for map/bookkeeping tests: building a real `BarkPlayer`
    // needs an output device, which test machines do not have.
    pub(crate) fn empty_for_test() -> Self {
        Self {
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
            soundfont_bytes: 0,
            next_cache_epoch: 1,
            pending_audio_loads: HashMap::new(),
            last_evict_sweep: Instant::now(),
            volumes_dirty: false,
            speeds_dirty: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pan(x: f32) -> AudioPan {
        AudioPan::new(x, 0.0, 0.0)
    }

    fn soundfont_id() -> perro_ids::SoundFontID {
        perro_ids::SoundFontID::from_string("res://piano.sf2")
    }

    #[test]
    fn nearby_pans_share_one_mixer_key() {
        // A drifting emitter used to mint a mixer per float pan.
        let base = MidiMixerKey::new(None, pan(0.5));
        assert_eq!(base, MidiMixerKey::new(None, pan(0.51)));
        assert_eq!(base, MidiMixerKey::new(None, pan(0.44)));
        assert_ne!(base, MidiMixerKey::new(None, pan(0.9)));
        assert!((base.pan().x - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn soundfont_mixer_key_quantizes_pan_too() {
        let id = soundfont_id();
        let base = SoundFontMidiMixerKey::new(id, None, pan(-0.5));
        assert_eq!(base, SoundFontMidiMixerKey::new(id, None, pan(-0.49)));
        assert_ne!(base, SoundFontMidiMixerKey::new(id, None, pan(0.0)));
        assert!((base.pan().x + 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn full_pan_sweep_stays_bounded() {
        let mut keys = std::collections::HashSet::new();
        for step in 0..=2000 {
            let x = -1.0 + step as f32 / 1000.0;
            keys.insert(MidiMixerKey::new(None, pan(x)));
        }
        // -1..1 at a 0.25 grid: 9 buckets, not 2001 mixers.
        assert_eq!(keys.len(), 9);
    }

    #[test]
    fn idle_mixer_prunes_only_after_ttl_and_without_tracked_notes() {
        let busy_until = Instant::now();
        let now = busy_until + MIXER_IDLE_TTL + Duration::from_millis(1);
        assert!(midi_mixer_is_idle(busy_until, now, false));
        // A held note still names this mixer: keep it alive.
        assert!(!midi_mixer_is_idle(busy_until, now, true));
        // Still inside the sustain window plus grace.
        assert!(!midi_mixer_is_idle(busy_until, busy_until, false));
        assert!(!midi_mixer_is_idle(
            now + Duration::from_secs(30),
            now,
            false
        ));
    }

    #[test]
    fn one_shot_notes_are_not_tracked() {
        let mut map: HashMap<u64, MidiMixerKey> = HashMap::new();
        let key = MidiMixerKey::new(None, pan(0.0));
        track_held_midi_note(&mut map, 1, key, false);
        assert!(map.is_empty());
        track_held_midi_note(&mut map, 2, key, true);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn release_takes_soundfont_note_entry() {
        let mut state = AudioState::empty_for_test();
        let built_in = MidiMixerKey::new(None, pan(0.0));
        let soundfont = SoundFontMidiMixerKey::new(soundfont_id(), None, pan(0.0));
        state.built_in_midi_notes.insert(1, built_in);
        state.soundfont_midi_notes.insert(2, soundfont);

        assert_eq!(
            take_midi_note_target(&mut state, 2),
            Some(MidiNoteReleaseTarget::SoundFont(soundfont))
        );
        assert!(state.soundfont_midi_notes.is_empty());
        assert_eq!(
            take_midi_note_target(&mut state, 1),
            Some(MidiNoteReleaseTarget::BuiltIn(built_in))
        );
        assert!(state.built_in_midi_notes.is_empty());
        assert_eq!(take_midi_note_target(&mut state, 3), None);
    }
}
