use perro_ids::AudioBusID;

pub trait AudioAPI {
    fn load_audio_source(&self, source: &str) -> bool;
    fn reserve_audio_source(&self, source: &str) -> bool;
    fn drop_audio_source(&self, source: &str) -> bool;
    fn play_audio(
        &self,
        source: &str,
        bus_id: AudioBusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> bool;
    fn stop_audio(
        &self,
        source: &str,
        bus_id: AudioBusID,
        looped: bool,
        volume: f32,
        speed: f32,
        from_start: f32,
        from_end: f32,
    ) -> bool;
    fn stop_audio_source(&self, source: &str) -> bool;
    fn audio_length_seconds(&self, source: &str) -> Option<f32>;
    fn stop_all_audio(&self);
    fn set_master_volume(&self, volume: f32) -> bool;
    fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool;
    fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool;
    fn pause_bus(&self, bus_id: AudioBusID) -> bool;
    fn resume_bus(&self, bus_id: AudioBusID) -> bool;
    fn stop_bus(&self, bus_id: AudioBusID) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub struct Audio<'a> {
    pub source: &'a str,
    pub bus: AudioBusID,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub from_start: f32,
    pub from_end: f32,
}

pub struct AudioModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> AudioModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load_source<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.load_audio_source(source.as_ref())
    }

    #[inline]
    pub fn reserve_source<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.reserve_audio_source(source.as_ref())
    }

    #[inline]
    pub fn drop_source<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.drop_audio_source(source.as_ref())
    }

    #[inline]
    pub fn play(&self, audio: Audio<'_>) -> bool {
        self.api.play_audio(
            audio.source,
            audio.bus,
            audio.looped,
            audio.volume,
            audio.speed,
            audio.from_start,
            audio.from_end,
        )
    }

    #[inline]
    pub fn stop_audio(&self, audio: Audio<'_>) -> bool {
        self.api.stop_audio(
            audio.source,
            audio.bus,
            audio.looped,
            audio.volume,
            audio.speed,
            audio.from_start,
            audio.from_end,
        )
    }

    #[inline]
    pub fn stop_source<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.stop_audio_source(source.as_ref())
    }

    #[inline]
    pub fn source_length_seconds<S: AsRef<str>>(&self, source: S) -> Option<f32> {
        self.api.audio_length_seconds(source.as_ref())
    }

    #[inline]
    pub fn source_length_millis<S: AsRef<str>>(&self, source: S) -> Option<u64> {
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
    ($res:expr, $audio:expr) => {
        $res.Audio().play($audio)
    };
}

#[macro_export]
macro_rules! audio_stop {
    ($res:expr, $audio:expr) => {
        $res.Audio().stop_audio($audio)
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
macro_rules! audio_bus {
    ($name:literal) => {{
        const BUS_ID: $crate::sub_apis::AudioBusID = $crate::sub_apis::bus_id($name);
        BUS_ID
    }};
}

pub const fn bus_id(name: &str) -> AudioBusID {
    AudioBusID::from_string(name)
}
