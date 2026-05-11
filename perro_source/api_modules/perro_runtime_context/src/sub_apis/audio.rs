use perro_ids::{AudioBusID, NodeID};

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

#[derive(Clone, Copy, Debug)]
pub struct SpatialAudioOptions {
    pub range: f32,
    pub bus_id: Option<AudioBusID>,
    pub occlusion_mask: u32,
    pub enable_propagation: bool,
}

impl SpatialAudioOptions {
    pub const fn new(range: f32) -> Self {
        Self {
            range,
            bus_id: None,
            occlusion_mask: u32::MAX,
            enable_propagation: true,
        }
    }
}

impl Default for SpatialAudioOptions {
    fn default() -> Self {
        Self::new(50.0)
    }
}

pub trait RuntimeAudioAPI {
    fn play_runtime_audio_attached(
        &mut self,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool;
    fn stop_runtime_audio_attached(&mut self, node: NodeID, source: &str) -> bool;
}

pub struct RuntimeAudioModule<'rt, RT: RuntimeAudioAPI + ?Sized> {
    rt: &'rt mut RT,
}

impl<'rt, RT: RuntimeAudioAPI + ?Sized> RuntimeAudioModule<'rt, RT> {
    pub fn new(rt: &'rt mut RT) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn play_attached(
        &mut self,
        audio: RuntimeAudio<'_>,
        node: NodeID,
        options: SpatialAudioOptions,
    ) -> bool {
        self.rt.play_runtime_audio_attached(audio, node, options)
    }

    #[inline]
    pub fn stop_attached(&mut self, node: NodeID, source: &str) -> bool {
        self.rt.stop_runtime_audio_attached(node, source)
    }
}
