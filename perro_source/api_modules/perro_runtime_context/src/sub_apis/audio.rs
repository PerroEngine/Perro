use perro_ids::{AudioBusID, NodeID};

#[derive(Clone, Copy, Debug)]
pub struct RuntimeAudio<'a> {
    pub source: &'a str,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub from_start: f32,
    pub from_end: f32,
}

impl<'a> RuntimeAudio<'a> {
    pub const fn new(source: &'a str) -> Self {
        Self {
            source,
            looped: false,
            volume: 1.0,
            speed: 1.0,
            from_start: 0.0,
            from_end: 0.0,
        }
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
