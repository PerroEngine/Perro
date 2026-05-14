use perro_ids::{AudioBusID, NodeID};
pub use perro_pawdio::{
    MidiChannel, MidiNoteHandle, MidiNoteOptions, MidiProgram, MidiSong, MidiSound, Note, program,
};
pub use perro_resource_api::sub_apis::{AudioDirection, SpatialAudioOptions};

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
pub struct RuntimeAudio<'a> {
    pub source: &'a str,
    pub looped: bool,
    pub volume: f32,
    pub effects: AudioEffects,
    pub from_start: f32,
    pub from_end: f32,
}

impl<'a> RuntimeAudio<'a> {
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

pub trait RuntimeAudioAPI {
    fn set_audio_debug_rays(&mut self, enabled: bool);
    fn audio_debug_rays_enabled(&mut self) -> bool;

    fn play_runtime_audio_attached(
        &mut self,
        bus_id: Option<AudioBusID>,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool;
    fn stop_runtime_audio_attached(&mut self, node: NodeID, source: &str) -> bool;
    fn play_midi_note_attached(
        &mut self,
        note: Note,
        node: NodeID,
        options: MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> bool;
    fn start_midi_note_attached(
        &mut self,
        note: Note,
        node: NodeID,
        options: MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> Option<MidiNoteHandle>;
    fn play_midi_file_attached(
        &mut self,
        song: MidiSong,
        node: NodeID,
        spatial: SpatialAudioOptions,
    ) -> bool;
    fn release_midi_note(&mut self, handle: MidiNoteHandle) -> bool;
    fn stop_midi_attached(&mut self, node: NodeID, target: AttachedMidiTarget<'_>) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub enum AttachedMidiTarget<'a> {
    Handle(MidiNoteHandle),
    Source(&'a str),
}

impl From<MidiNoteHandle> for AttachedMidiTarget<'_> {
    fn from(value: MidiNoteHandle) -> Self {
        Self::Handle(value)
    }
}

impl<'a> From<&'a str> for AttachedMidiTarget<'a> {
    fn from(value: &'a str) -> Self {
        Self::Source(value)
    }
}

pub struct RuntimeAudioModule<'rt, RT: RuntimeAudioAPI + ?Sized> {
    rt: &'rt mut RT,
}

impl<'rt, RT: RuntimeAudioAPI + ?Sized> RuntimeAudioModule<'rt, RT> {
    pub fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn set_debug_rays(&mut self, enabled: bool) {
        self.rt.set_audio_debug_rays(enabled);
    }

    #[inline]
    pub fn debug_rays_enabled(&mut self) -> bool {
        self.rt.audio_debug_rays_enabled()
    }

    #[inline]
    pub fn play_attached(
        &mut self,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool {
        self.rt
            .play_runtime_audio_attached(None, audio, node, options)
    }

    #[inline]
    pub fn play_attached_bus(
        &mut self,
        bus_id: AudioBusID,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool {
        self.rt
            .play_runtime_audio_attached(Some(bus_id), audio, node, options)
    }

    #[inline]
    pub fn stop_attached(&mut self, node: NodeID, source: &str) -> bool {
        self.rt.stop_runtime_audio_attached(node, source)
    }

    #[inline]
    pub fn midi(&mut self) -> RuntimeMidiModule<'_, RT> {
        RuntimeMidiModule { rt: self.rt }
    }
}

pub struct RuntimeMidiModule<'rt, RT: RuntimeAudioAPI + ?Sized> {
    rt: &'rt mut RT,
}

impl<'rt, RT: RuntimeAudioAPI + ?Sized> RuntimeMidiModule<'rt, RT> {
    #[inline]
    pub fn play_note_attached(
        &mut self,
        note: Note,
        node: NodeID,
        options: MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> bool {
        self.rt
            .play_midi_note_attached(note, node, options, spatial)
    }

    #[inline]
    pub fn start_note_attached(
        &mut self,
        note: Note,
        node: NodeID,
        options: MidiNoteOptions,
        spatial: SpatialAudioOptions,
    ) -> Option<MidiNoteHandle> {
        self.rt
            .start_midi_note_attached(note, node, options, spatial)
    }

    #[inline]
    pub fn play_file_attached(
        &mut self,
        song: MidiSong,
        node: NodeID,
        spatial: SpatialAudioOptions,
    ) -> bool {
        self.rt.play_midi_file_attached(song, node, spatial)
    }

    #[inline]
    pub fn release_note(&mut self, handle: MidiNoteHandle) -> bool {
        self.rt.release_midi_note(handle)
    }

    #[inline]
    pub fn stop_attached<T: Into<AttachedMidiTarget<'rt>>>(
        &mut self,
        node: NodeID,
        target: T,
    ) -> bool {
        self.rt.stop_midi_attached(node, target.into())
    }
}

#[macro_export]
macro_rules! audio_play_attached {
    ($rt:expr, $bus_id:expr, $audio:expr, $node:expr, $options:expr) => {
        $rt.Audio()
            .play_attached_bus($bus_id, $audio, $node, $options)
    };
    ($rt:expr, $audio:expr, $node:expr, $options:expr) => {
        $rt.Audio().play_attached($audio, $node, $options)
    };
}

#[macro_export]
macro_rules! midi_play_attached {
    ($rt:expr, $note:expr, $node:expr, $options:expr, $spatial:expr) => {
        $rt.Audio()
            .midi()
            .play_note_attached($note, $node, $options, $spatial)
    };
    ($rt:expr, $song:expr, $node:expr, $spatial:expr) => {
        $rt.Audio()
            .midi()
            .play_file_attached($song, $node, $spatial)
    };
}

#[macro_export]
macro_rules! midi_start_attached {
    ($rt:expr, $note:expr, $node:expr, $options:expr, $spatial:expr) => {
        $rt.Audio()
            .midi()
            .start_note_attached($note, $node, $options, $spatial)
    };
}

#[macro_export]
macro_rules! midi_release_attached {
    ($rt:expr, $handle:expr) => {
        $rt.Audio().midi().release_note($handle)
    };
}

#[macro_export]
macro_rules! midi_stop_attached {
    ($rt:expr, $node:expr, $target:expr) => {
        $rt.Audio().midi().stop_attached($node, $target)
    };
}
