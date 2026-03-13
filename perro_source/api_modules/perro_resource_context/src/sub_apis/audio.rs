pub trait AudioAPI {
    fn play_audio(&self, source: &str, bus_id: u32, looped: bool, volume: f32, pitch: f32)
    -> bool;
    fn stop_audio(&self, source: &str, bus_id: u32, looped: bool, volume: f32, pitch: f32)
    -> bool;
    fn stop_audio_source(&self, source: &str) -> bool;
    fn stop_all_audio(&self);
    fn set_master_volume(&self, volume: f32) -> bool;
    fn set_bus_volume(&self, bus_id: u32, volume: f32) -> bool;
    fn pause_bus(&self, bus_id: u32) -> bool;
    fn resume_bus(&self, bus_id: u32) -> bool;
    fn stop_bus(&self, bus_id: u32) -> bool;
}

pub struct Audio<'a> {
    pub source: &'a str,
    pub bus: u32,
    pub looped: bool,
    pub volume: f32,
    pub pitch: f32,
}

pub struct AudioModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> AudioModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn play(&self, audio: Audio<'_>) -> bool {
        self.api
            .play_audio(audio.source, audio.bus, audio.looped, audio.volume, audio.pitch)
    }

    #[inline]
    pub fn stop_audio(&self, audio: Audio<'_>) -> bool {
        self.api
            .stop_audio(audio.source, audio.bus, audio.looped, audio.volume, audio.pitch)
    }

    #[inline]
    pub fn stop_source<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.stop_audio_source(source.as_ref())
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
    pub fn set_bus_volume(&self, bus_id: u32, volume: f32) -> bool {
        self.api.set_bus_volume(bus_id, volume)
    }

    #[inline]
    pub fn pause_bus(&self, bus_id: u32) -> bool {
        self.api.pause_bus(bus_id)
    }

    #[inline]
    pub fn resume_bus(&self, bus_id: u32) -> bool {
        self.api.resume_bus(bus_id)
    }

    #[inline]
    pub fn stop_bus(&self, bus_id: u32) -> bool {
        self.api.stop_bus(bus_id)
    }
}

#[macro_export]
macro_rules! play_audio {
    ($res:expr, $audio:expr) => {
        $res.Audio().play($audio)
    };
}

#[macro_export]
macro_rules! stop_audio {
    ($res:expr, $audio:expr) => {
        $res.Audio().stop_audio($audio)
    };
}

#[macro_export]
macro_rules! stop_audio_source {
    ($res:expr, $source:expr) => {
        $res.Audio().stop_source($source)
    };
}

#[macro_export]
macro_rules! stop_all_audio {
    ($res:expr) => {
        $res.Audio().stop_all()
    };
}

#[macro_export]
macro_rules! set_master_volume {
    ($res:expr, $volume:expr) => {
        $res.Audio().set_master_volume($volume)
    };
}

#[macro_export]
macro_rules! set_bus_volume {
    ($res:expr, $bus_id:expr, $volume:expr) => {
        $res.Audio().set_bus_volume($bus_id, $volume)
    };
}

#[macro_export]
macro_rules! pause_bus {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().pause_bus($bus_id)
    };
}

#[macro_export]
macro_rules! resume_bus {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().resume_bus($bus_id)
    };
}

#[macro_export]
macro_rules! stop_bus {
    ($res:expr, $bus_id:expr) => {
        $res.Audio().stop_bus($bus_id)
    };
}

#[macro_export]
macro_rules! bus {
    ($name:literal) => {{
        const BUS_ID: u32 = $crate::sub_apis::bus_id($name);
        BUS_ID
    }};
}

pub const fn bus_id(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut hash: u32 = 0x811C9DC5;
    let mut i = 0usize;
    while i < bytes.len() {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        i += 1;
    }
    hash
}
