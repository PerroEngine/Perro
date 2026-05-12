use crate::ResPathSource;
use perro_ids::{AudioBusID, SoundFontID};
pub use perro_pawdio::{
    MidiChannel, MidiNoteHandle, MidiNoteOptions, MidiProgram, MidiSong, MidiSound, Note, program,
};
use perro_structs::{Vector2, Vector3};

pub trait AudioAPI {
    fn load_audio_source(&self, source: &str) -> bool;
    fn reserve_audio_source(&self, source: &str) -> bool;
    fn drop_audio_source(&self, source: &str) -> bool;
    fn play_audio(&self, bus_id: Option<AudioBusID>, audio: Audio<'_>, pan: AudioPan) -> bool;
    fn play_audio_2d(&self, bus_id: Option<AudioBusID>, audio: Audio2D<'_>) -> bool;
    fn play_audio_3d(&self, bus_id: Option<AudioBusID>, audio: Audio3D<'_>) -> bool;
    fn stop_audio(&self, bus_id: Option<AudioBusID>, audio: Audio<'_>, pan: AudioPan) -> bool;
    fn stop_audio_source(&self, source: &str) -> bool;
    fn audio_length_seconds(&self, source: &str) -> Option<f32>;
    fn stop_all_audio(&self);
    fn set_master_volume(&self, volume: f32) -> bool;
    fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool;
    fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool;
    fn pause_bus(&self, bus_id: AudioBusID) -> bool;
    fn resume_bus(&self, bus_id: AudioBusID) -> bool;
    fn stop_bus(&self, bus_id: AudioBusID) -> bool;
    fn load_midi_soundfont_hashed(&self, source_hash: u64, source: Option<&str>) -> SoundFontID;
    fn load_midi_soundfont(&self, source: &str) -> SoundFontID {
        self.load_midi_soundfont_hashed(perro_ids::string_to_u64(source), Some(source))
    }
    fn play_midi_note(&self, note: Note, options: MidiNoteOptions) -> bool;
    fn start_midi_note(&self, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle>;
    fn release_midi_note(&self, handle: MidiNoteHandle) -> bool;
    fn play_midi_file(&self, song: MidiSong) -> bool;
    fn play_midi_note_at(
        &self,
        note: Note,
        position: MidiSpatialPosition,
        range: f32,
        options: MidiNoteOptions,
    ) -> bool;
    fn start_midi_note_at(
        &self,
        note: Note,
        position: MidiSpatialPosition,
        range: f32,
        options: MidiNoteOptions,
    ) -> Option<MidiNoteHandle>;
    fn play_midi_file_at(&self, song: MidiSong, position: MidiSpatialPosition, range: f32) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub enum MidiSpatialPosition {
    TwoD(Vector2),
    ThreeD(Vector3),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AudioDirection<T> {
    #[default]
    Omni,
    Directional(T),
    InverseDirectional(T),
    Bidirectional(T),
}

#[derive(Clone, Copy, Debug)]
pub struct SpatialAudioOptions {
    pub range: f32,
    pub occlusion_mask: u32,
    pub enable_propagation: bool,
    pub direction_2d: AudioDirection<Vector2>,
    pub direction_3d: AudioDirection<Vector3>,
}

pub trait MidiSpatialPos {
    fn into_midi_spatial_position(self) -> MidiSpatialPosition;
}

impl MidiSpatialPos for Vector2 {
    fn into_midi_spatial_position(self) -> MidiSpatialPosition {
        MidiSpatialPosition::TwoD(self)
    }
}

impl MidiSpatialPos for Vector3 {
    fn into_midi_spatial_position(self) -> MidiSpatialPosition {
        MidiSpatialPosition::ThreeD(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioPan {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AudioPan {
    pub const CENTER: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl Default for AudioPan {
    fn default() -> Self {
        Self::CENTER
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioEq {
    pub low_gain: f32,
    pub mid_gain: f32,
    pub high_gain: f32,
}

impl AudioEq {
    pub const fn new() -> Self {
        Self {
            low_gain: 1.0,
            mid_gain: 1.0,
            high_gain: 1.0,
        }
    }
}

impl Default for AudioEq {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioCompression {
    pub threshold: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
}

impl AudioCompression {
    pub const fn new() -> Self {
        Self {
            threshold: 1.0,
            ratio: 1.0,
            attack: 0.01,
            release: 0.1,
        }
    }
}

impl Default for AudioCompression {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioEffects {
    pub speed: f32,
    pub low_pass: f32,
    pub reverb_send: f32,
    pub echo: f32,
    pub reflection: f32,
    pub occlusion: f32,
    pub eq: AudioEq,
    pub compression: AudioCompression,
}

impl AudioEffects {
    pub const fn new() -> Self {
        Self {
            speed: 1.0,
            low_pass: 0.0,
            reverb_send: 0.0,
            echo: 0.0,
            reflection: 0.0,
            occlusion: 0.0,
            eq: AudioEq::new(),
            compression: AudioCompression::new(),
        }
    }

    pub const fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

impl Default for AudioEffects {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Audio<'a> {
    pub source: &'a str,
    pub looped: bool,
    pub volume: f32,
    pub effects: AudioEffects,
    pub from_start: f32,
    pub from_end: f32,
}

impl<'a> Audio<'a> {
    pub const fn new(source: &'a str) -> Self {
        Self {
            source,
            looped: false,
            volume: 1.0,
            effects: AudioEffects::new(),
            from_start: 0.0,
            from_end: 0.0,
        }
    }

    pub const fn with_effects(mut self, effects: AudioEffects) -> Self {
        self.effects = effects;
        self
    }

    pub const fn with_speed(mut self, speed: f32) -> Self {
        self.effects.speed = speed;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PannedAudio<'a> {
    pub audio: Audio<'a>,
    pub pan: AudioPan,
}

impl<'a> PannedAudio<'a> {
    pub const fn new(audio: Audio<'a>, pan: AudioPan) -> Self {
        Self { audio, pan }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Audio2D<'a> {
    pub audio: Audio<'a>,
    pub position: Vector2,
    pub range: f32,
    pub occlusion_mask: u32,
    pub enable_propagation: bool,
    pub direction: Option<AudioDirection<Vector2>>,
}

impl<'a> Audio2D<'a> {
    pub const fn new(source: &'a str, position: Vector2, range: f32) -> Self {
        Self {
            audio: Audio::new(source),
            position,
            range,
            occlusion_mask: u32::MAX,
            enable_propagation: true,
            direction: None,
        }
    }

    pub const fn from_audio(audio: Audio<'a>, position: Vector2, range: f32) -> Self {
        Self {
            audio,
            position,
            range,
            occlusion_mask: u32::MAX,
            enable_propagation: true,
            direction: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Audio3D<'a> {
    pub audio: Audio<'a>,
    pub position: Vector3,
    pub range: f32,
    pub occlusion_mask: u32,
    pub enable_propagation: bool,
    pub direction: Option<AudioDirection<Vector3>>,
}

impl<'a> Audio3D<'a> {
    pub const fn new(source: &'a str, position: Vector3, range: f32) -> Self {
        Self {
            audio: Audio::new(source),
            position,
            range,
            occlusion_mask: u32::MAX,
            enable_propagation: true,
            direction: None,
        }
    }

    pub const fn from_audio(audio: Audio<'a>, position: Vector3, range: f32) -> Self {
        Self {
            audio,
            position,
            range,
            occlusion_mask: u32::MAX,
            enable_propagation: true,
            direction: None,
        }
    }
}

pub trait AudioPlayConfig<R: AudioAPI + ?Sized> {
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool;
}

impl<'a, R: AudioAPI + ?Sized> AudioPlayConfig<R> for Audio<'a> {
    #[inline]
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool {
        api.play_audio(bus_id, self, AudioPan::CENTER)
    }
}

impl<'a, R: AudioAPI + ?Sized> AudioPlayConfig<R> for PannedAudio<'a> {
    #[inline]
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool {
        api.play_audio(bus_id, self.audio, self.pan)
    }
}

impl<'a, R: AudioAPI + ?Sized> AudioPlayConfig<R> for (Audio<'a>, AudioPan) {
    #[inline]
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool {
        api.play_audio(bus_id, self.0, self.1)
    }
}

impl<'a, R: AudioAPI + ?Sized> AudioPlayConfig<R> for Audio2D<'a> {
    #[inline]
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool {
        api.play_audio_2d(bus_id, self)
    }
}

impl<'a, R: AudioAPI + ?Sized> AudioPlayConfig<R> for Audio3D<'a> {
    #[inline]
    fn play_with(self, api: &R, bus_id: Option<AudioBusID>) -> bool {
        api.play_audio_3d(bus_id, self)
    }
}

pub struct AudioModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> AudioModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load_source<S: ResPathSource>(&self, source: S) -> bool {
        self.api.load_audio_source(source.as_res_path_str())
    }

    #[inline]
    pub fn reserve_source<S: ResPathSource>(&self, source: S) -> bool {
        self.api.reserve_audio_source(source.as_res_path_str())
    }

    #[inline]
    pub fn drop_source<S: ResPathSource>(&self, source: S) -> bool {
        self.api.drop_audio_source(source.as_res_path_str())
    }

    #[inline]
    pub fn play(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool {
        self.api.play_audio(Some(bus_id), audio, AudioPan::CENTER)
    }

    #[inline]
    pub fn play_bus<C>(&self, bus_id: AudioBusID, audio: C) -> bool
    where
        C: AudioPlayConfig<R>,
    {
        audio.play_with(self.api, Some(bus_id))
    }

    #[inline]
    pub fn play_master<C>(&self, audio: C) -> bool
    where
        C: AudioPlayConfig<R>,
    {
        audio.play_with(self.api, None)
    }

    #[inline]
    pub fn play_master_audio(&self, audio: Audio<'_>) -> bool {
        self.api.play_audio(None, audio, AudioPan::CENTER)
    }

    #[inline]
    pub fn play_panned(&self, bus_id: AudioBusID, audio: Audio<'_>, pan: AudioPan) -> bool {
        self.api.play_audio(Some(bus_id), audio, pan)
    }

    #[inline]
    pub fn play_master_panned(&self, audio: Audio<'_>, pan: AudioPan) -> bool {
        self.api.play_audio(None, audio, pan)
    }

    #[inline]
    pub fn two_d(&self) -> Audio2DModule<'res, R> {
        Audio2DModule { api: self.api }
    }

    #[inline]
    pub fn three_d(&self) -> Audio3DModule<'res, R> {
        Audio3DModule { api: self.api }
    }

    #[inline]
    pub fn midi(&self) -> MidiModule<'res, R> {
        MidiModule { api: self.api }
    }

    #[inline]
    pub fn stop_audio(&self, bus_id: AudioBusID, audio: Audio<'_>) -> bool {
        self.api.stop_audio(Some(bus_id), audio, AudioPan::CENTER)
    }

    #[inline]
    pub fn stop_master_audio(&self, audio: Audio<'_>) -> bool {
        self.api.stop_audio(None, audio, AudioPan::CENTER)
    }

    #[inline]
    pub fn stop_source<S: ResPathSource>(&self, source: S) -> bool {
        self.api.stop_audio_source(source.as_res_path_str())
    }

    #[inline]
    pub fn source_length_seconds<S: ResPathSource>(&self, source: S) -> Option<f32> {
        self.api.audio_length_seconds(source.as_res_path_str())
    }

    #[inline]
    pub fn source_length_millis<S: ResPathSource>(&self, source: S) -> Option<u64> {
        self.source_length_seconds(source)
            .map(|seconds| (seconds * 1000.0).max(0.0) as u64)
    }

    #[inline]
    pub fn stop_all(&self) {
        self.api.stop_all_audio();
    }

    #[inline]
    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.api.set_master_volume(volume)
    }

    #[inline]
    pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool {
        self.api.set_bus_volume(bus_id, volume)
    }

    #[inline]
    pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool {
        self.api.set_bus_speed(bus_id, speed)
    }

    #[inline]
    pub fn pause_bus(&self, bus_id: AudioBusID) -> bool {
        self.api.pause_bus(bus_id)
    }

    #[inline]
    pub fn resume_bus(&self, bus_id: AudioBusID) -> bool {
        self.api.resume_bus(bus_id)
    }

    #[inline]
    pub fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        self.api.stop_bus(bus_id)
    }
}

pub struct Audio2DModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> Audio2DModule<'res, R> {
    #[inline]
    pub fn play(&self, bus_id: AudioBusID, audio: Audio2D<'_>) -> bool {
        self.api.play_audio_2d(Some(bus_id), audio)
    }

    #[inline]
    pub fn play_master(&self, audio: Audio2D<'_>) -> bool {
        self.api.play_audio_2d(None, audio)
    }
}

pub struct Audio3DModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

pub struct MidiModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> MidiModule<'res, R> {
    #[inline]
    pub fn load_soundfont<S: ResPathSource>(&self, source: S) -> SoundFontID {
        self.api.load_midi_soundfont(source.as_res_path_str())
    }

    #[inline]
    pub fn load_soundfont_hashed(&self, source_hash: u64) -> SoundFontID {
        self.api.load_midi_soundfont_hashed(source_hash, None)
    }

    #[inline]
    pub fn load_soundfont_hashed_with_source<S: ResPathSource>(
        &self,
        source_hash: u64,
        source: S,
    ) -> SoundFontID {
        self.api
            .load_midi_soundfont_hashed(source_hash, Some(source.as_res_path_str()))
    }

    #[inline]
    pub fn play_note(&self, note: Note, options: MidiNoteOptions) -> bool {
        self.api.play_midi_note(note, options)
    }

    #[inline]
    pub fn play_note_bus(
        &self,
        bus_id: AudioBusID,
        note: Note,
        mut options: MidiNoteOptions,
    ) -> bool {
        options.bus_id = Some(bus_id);
        self.api.play_midi_note(note, options)
    }

    #[inline]
    pub fn start_note(&self, note: Note, options: MidiNoteOptions) -> Option<MidiNoteHandle> {
        self.api.start_midi_note(note, options)
    }

    #[inline]
    pub fn start_note_bus(
        &self,
        bus_id: AudioBusID,
        note: Note,
        mut options: MidiNoteOptions,
    ) -> Option<MidiNoteHandle> {
        options.bus_id = Some(bus_id);
        self.api.start_midi_note(note, options)
    }

    #[inline]
    pub fn release_note(&self, handle: MidiNoteHandle) -> bool {
        self.api.release_midi_note(handle)
    }

    #[inline]
    pub fn play_file(&self, song: MidiSong) -> bool {
        self.api.play_midi_file(song)
    }

    #[inline]
    pub fn play_note_at<P: MidiSpatialPos>(
        &self,
        note: Note,
        position: P,
        range: f32,
        options: MidiNoteOptions,
    ) -> bool {
        self.api
            .play_midi_note_at(note, position.into_midi_spatial_position(), range, options)
    }

    #[inline]
    pub fn start_note_at<P: MidiSpatialPos>(
        &self,
        note: Note,
        position: P,
        range: f32,
        options: MidiNoteOptions,
    ) -> Option<MidiNoteHandle> {
        self.api
            .start_midi_note_at(note, position.into_midi_spatial_position(), range, options)
    }

    #[inline]
    pub fn play_file_at<P: MidiSpatialPos>(&self, song: MidiSong, position: P, range: f32) -> bool {
        self.api
            .play_midi_file_at(song, position.into_midi_spatial_position(), range)
    }
}

impl<'res, R: AudioAPI + ?Sized> Audio3DModule<'res, R> {
    #[inline]
    pub fn play(&self, bus_id: AudioBusID, audio: Audio3D<'_>) -> bool {
        self.api.play_audio_3d(Some(bus_id), audio)
    }

    #[inline]
    pub fn play_master(&self, audio: Audio3D<'_>) -> bool {
        self.api.play_audio_3d(None, audio)
    }
}

#[macro_export]
macro_rules! audio_load {
    ($res:expr, $source:expr) => {
        $res.Audio().load_source($source)
    };
}

#[macro_export]
macro_rules! audio_reserve {
    ($res:expr, $source:expr) => {
        $res.Audio().reserve_source($source)
    };
}

#[macro_export]
macro_rules! audio_drop {
    ($res:expr, $source:expr) => {
        $res.Audio().drop_source($source)
    };
}

#[macro_export]
macro_rules! audio_play {
    ($res:expr, $bus_id:expr, $audio:expr) => {
        $res.Audio().play_bus($bus_id, $audio)
    };
    ($res:expr, $audio:expr) => {
        $res.Audio().play_master($audio)
    };
}

#[macro_export]
macro_rules! audio_stop {
    ($res:expr, $bus_id:expr, $audio:expr) => {
        $res.Audio().stop_audio($bus_id, $audio)
    };
    ($res:expr, $audio:expr) => {
        $res.Audio().stop_master_audio($audio)
    };
}

#[macro_export]
macro_rules! audio_stop_source {
    ($res:expr, $source:expr) => {
        $res.Audio().stop_source($source)
    };
}

#[macro_export]
macro_rules! audio_length_seconds {
    ($res:expr, $source:expr) => {
        $res.Audio().source_length_seconds($source)
    };
}

#[macro_export]
macro_rules! audio_length_millis {
    ($res:expr, $source:expr) => {
        $res.Audio().source_length_millis($source)
    };
}

#[macro_export]
macro_rules! audio_stop_all {
    ($res:expr) => {
        $res.Audio().stop_all()
    };
}

#[macro_export]
macro_rules! audio_set_master_volume {
    ($res:expr, $volume:expr) => {
        $res.Audio().set_master_volume($volume)
    };
}

#[macro_export]
macro_rules! audio_bus_set_volume {
    ($res:expr, $bus_id:expr, $volume:expr) => {
        $res.Audio().set_bus_volume($bus_id, $volume)
    };
}

#[macro_export]
macro_rules! audio_bus_set_speed {
    ($res:expr, $bus_id:expr, $speed:expr) => {
        $res.Audio().set_bus_speed($bus_id, $speed)
    };
}

#[macro_export]
macro_rules! audio_bus_pause {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().pause_bus($bus_id)
    };
}

#[macro_export]
macro_rules! audio_bus_resume {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().resume_bus($bus_id)
    };
}

#[macro_export]
macro_rules! audio_bus_stop {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().stop_bus($bus_id)
    };
}

#[macro_export]
/// R is the return type of the underlying API method call this macro expands to.
macro_rules! audio_bus {
    ($name:literal) => {{
        const BUS_ID: $crate::sub_apis::AudioBusID = $crate::sub_apis::bus_id($name);
        BUS_ID
    }};
}

#[macro_export]
macro_rules! midi_load_soundfont {
    ($res:expr, $source:literal) => {{
        const __HASH: u64 = $crate::__perro_string_to_u64($source);
        $res.Audio()
            .midi()
            .load_soundfont_hashed_with_source(__HASH, $source)
    }};
    ($res:expr, $source:expr) => {
        $res.Audio().midi().load_soundfont($source)
    };
}

#[macro_export]
macro_rules! midi_play {
    ($res:expr, $bus_id:expr, $note:expr, $options:expr) => {
        $res.Audio().midi().play_note_bus($bus_id, $note, $options)
    };
    ($res:expr, $note:expr, $options:expr) => {
        $res.Audio().midi().play_note($note, $options)
    };
    ($res:expr, $song:expr) => {
        $res.Audio().midi().play_file($song)
    };
}

#[macro_export]
macro_rules! midi_start {
    ($res:expr, $bus_id:expr, $note:expr, $options:expr) => {
        $res.Audio().midi().start_note_bus($bus_id, $note, $options)
    };
    ($res:expr, $note:expr, $options:expr) => {
        $res.Audio().midi().start_note($note, $options)
    };
}

#[macro_export]
macro_rules! midi_release {
    ($res:expr, $handle:expr) => {
        $res.Audio().midi().release_note($handle)
    };
}

#[macro_export]
macro_rules! midi_play_at {
    ($res:expr, $note:expr, $pos:expr, $range:expr, $options:expr) => {
        $res.Audio()
            .midi()
            .play_note_at($note, $pos, $range, $options)
    };
    ($res:expr, $song:expr, $pos:expr, $range:expr) => {
        $res.Audio().midi().play_file_at($song, $pos, $range)
    };
}

#[macro_export]
macro_rules! midi_start_at {
    ($res:expr, $note:expr, $pos:expr, $range:expr, $options:expr) => {
        $res.Audio()
            .midi()
            .start_note_at($note, $pos, $range, $options)
    };
}

pub const fn bus_id(name: &str) -> AudioBusID {
    AudioBusID::from_string(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAudioApi;

    impl AudioAPI for DummyAudioApi {
        fn load_audio_source(&self, _source: &str) -> bool {
            true
        }

        fn reserve_audio_source(&self, _source: &str) -> bool {
            true
        }

        fn drop_audio_source(&self, _source: &str) -> bool {
            true
        }

        fn play_audio(
            &self,
            _bus_id: Option<AudioBusID>,
            _audio: Audio<'_>,
            _pan: AudioPan,
        ) -> bool {
            true
        }

        fn play_audio_2d(&self, _bus_id: Option<AudioBusID>, _audio: Audio2D<'_>) -> bool {
            true
        }

        fn play_audio_3d(&self, _bus_id: Option<AudioBusID>, _audio: Audio3D<'_>) -> bool {
            true
        }

        fn stop_audio(
            &self,
            _bus_id: Option<AudioBusID>,
            _audio: Audio<'_>,
            _pan: AudioPan,
        ) -> bool {
            true
        }

        fn stop_audio_source(&self, _source: &str) -> bool {
            true
        }

        fn audio_length_seconds(&self, _source: &str) -> Option<f32> {
            Some(1.0)
        }

        fn stop_all_audio(&self) {}

        fn set_master_volume(&self, _volume: f32) -> bool {
            true
        }

        fn set_bus_volume(&self, _bus_id: AudioBusID, _volume: f32) -> bool {
            true
        }

        fn set_bus_speed(&self, _bus_id: AudioBusID, _speed: f32) -> bool {
            true
        }

        fn pause_bus(&self, _bus_id: AudioBusID) -> bool {
            true
        }

        fn resume_bus(&self, _bus_id: AudioBusID) -> bool {
            true
        }

        fn stop_bus(&self, _bus_id: AudioBusID) -> bool {
            true
        }

        fn load_midi_soundfont_hashed(
            &self,
            source_hash: u64,
            _source: Option<&str>,
        ) -> SoundFontID {
            SoundFontID::from_u64(source_hash)
        }

        fn play_midi_note(&self, _note: Note, _options: MidiNoteOptions) -> bool {
            true
        }

        fn start_midi_note(
            &self,
            _note: Note,
            _options: MidiNoteOptions,
        ) -> Option<MidiNoteHandle> {
            Some(MidiNoteHandle(1))
        }

        fn release_midi_note(&self, _handle: MidiNoteHandle) -> bool {
            true
        }

        fn play_midi_file(&self, _song: MidiSong) -> bool {
            true
        }

        fn play_midi_note_at(
            &self,
            _note: Note,
            _position: MidiSpatialPosition,
            _range: f32,
            _options: MidiNoteOptions,
        ) -> bool {
            true
        }

        fn start_midi_note_at(
            &self,
            _note: Note,
            _position: MidiSpatialPosition,
            _range: f32,
            _options: MidiNoteOptions,
        ) -> Option<MidiNoteHandle> {
            Some(MidiNoteHandle(2))
        }

        fn play_midi_file_at(
            &self,
            _song: MidiSong,
            _position: MidiSpatialPosition,
            _range: f32,
        ) -> bool {
            true
        }
    }

    struct DummyResource<'a>(&'a DummyAudioApi);

    #[allow(non_snake_case)]
    impl DummyResource<'_> {
        fn Audio(&self) -> AudioModule<'_, DummyAudioApi> {
            AudioModule::new(self.0)
        }
    }

    #[test]
    fn audio_play_macro_dispatches_by_audio_type() {
        let api = DummyAudioApi;
        let res = DummyResource(&api);
        let bus = bus_id("sfx");

        assert!(crate::audio_play!(res, Audio::new("res://base.wav")));
        assert!(crate::audio_play!(
            res,
            bus,
            Audio2D::new("res://hit.wav", Vector2::new(1.0, 2.0), 10.0)
        ));
        assert!(crate::audio_play!(
            res,
            Audio3D::new("res://step.wav", Vector3::new(1.0, 2.0, 3.0), 10.0)
        ));
        assert!(crate::audio_play!(
            res,
            bus,
            Audio3D::new("res://step.wav", Vector3::new(1.0, 2.0, 3.0), 10.0)
        ));
    }

    #[test]
    fn midi_macros_dispatch() {
        let api = DummyAudioApi;
        let res = DummyResource(&api);
        assert!(crate::midi_play!(res, Note::C4, MidiNoteOptions::default()));
        assert!(crate::midi_play!(res, MidiSong::new("res://song.mid")));
        assert!(crate::midi_play_at!(
            res,
            Note::C4,
            Vector2::new(1.0, 2.0),
            10.0,
            MidiNoteOptions::default()
        ));
        assert!(crate::midi_start!(res, Note::C4, MidiNoteOptions::default()).is_some());
    }
}
