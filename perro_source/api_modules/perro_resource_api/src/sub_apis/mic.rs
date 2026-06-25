use crate::ResPathSource;
use perro_ids::AudioBusID;
pub use perro_pawdio::{MicClip, MicDenoiseSettings, MicSettings};

pub trait MicAPI {
    fn mic_start(&self, settings: MicSettings) -> Result<(), String>;
    fn mic_stop(&self) -> Option<MicClip>;
    fn mic_clip(&self) -> Option<MicClip>;
    fn mic_stream_clip(&self) -> Option<MicClip>;
    fn mic_stream_bytes(&self) -> Option<Vec<u8>>;
    fn mic_is_listening(&self) -> bool;
    fn mic_play(&self, bus_id: Option<AudioBusID>, clip: &MicClip, volume: f32) -> bool;
    fn mic_save_wav(&self, source: &str, clip: &MicClip) -> Result<(), String>;
}

pub struct MicModule<'res, R: MicAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: MicAPI + ?Sized> MicModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn start_listening(&self) -> Result<(), String> {
        self.api.mic_start(MicSettings::default())
    }

    #[inline]
    pub fn start_stream(&self) -> Result<(), String> {
        self.start_listening()
    }

    #[inline]
    pub fn start_with(&self, settings: MicSettings) -> Result<(), String> {
        self.api.mic_start(settings)
    }

    #[inline]
    pub fn record(&self) -> Result<(), String> {
        self.start_listening()
    }

    #[inline]
    pub fn stop_listening(&self) -> Option<MicClip> {
        self.api.mic_stop()
    }

    #[inline]
    pub fn stop_stream(&self) -> Option<MicClip> {
        self.stop_listening()
    }

    #[inline]
    pub fn stop(&self) -> Option<MicClip> {
        self.stop_listening()
    }

    #[inline]
    pub fn clip(&self) -> Option<MicClip> {
        self.api.mic_clip()
    }

    #[inline]
    pub fn stream_clip(&self) -> Option<MicClip> {
        self.api.mic_stream_clip()
    }

    #[inline]
    pub fn stream_bytes(&self) -> Option<Vec<u8>> {
        self.api.mic_stream_bytes()
    }

    #[inline]
    pub fn get_clip(&self) -> Option<MicClip> {
        self.stream_clip()
    }

    #[inline]
    pub fn get_bytes(&self) -> Option<Vec<u8>> {
        self.stream_bytes()
    }

    #[inline]
    pub fn is_listening(&self) -> bool {
        self.api.mic_is_listening()
    }

    #[inline]
    pub fn play_master(&self, clip: &MicClip) -> bool {
        self.api.mic_play(None, clip, 1.0)
    }

    #[inline]
    pub fn play_master_volume(&self, clip: &MicClip, volume: f32) -> bool {
        self.api.mic_play(None, clip, volume)
    }

    #[inline]
    pub fn play_bus(&self, bus_id: AudioBusID, clip: &MicClip) -> bool {
        self.api.mic_play(Some(bus_id), clip, 1.0)
    }

    #[inline]
    pub fn play_bus_volume(&self, bus_id: AudioBusID, clip: &MicClip, volume: f32) -> bool {
        self.api.mic_play(Some(bus_id), clip, volume)
    }

    #[inline]
    pub fn save_wav<S: ResPathSource>(&self, source: S, clip: &MicClip) -> Result<(), String> {
        self.api.mic_save_wav(source.as_res_path_str(), clip)
    }

    #[inline]
    pub fn pack(&self, clip: &MicClip) -> Vec<u8> {
        clip.pack()
    }

    #[inline]
    pub fn unpack(&self, bytes: &[u8]) -> Result<MicClip, String> {
        MicClip::unpack(bytes)
    }
}

#[macro_export]
macro_rules! mic_start {
    ($res:expr) => {
        $res.Mic().start_listening()
    };
    ($res:expr, $settings:expr) => {
        $res.Mic().start_with($settings)
    };
}

#[macro_export]
macro_rules! mic_start_listening {
    ($res:expr) => {
        $res.Mic().start_listening()
    };
}

#[macro_export]
macro_rules! mic_start_stream {
    ($res:expr) => {
        $res.Mic().start_stream()
    };
}

#[macro_export]
macro_rules! mic_start_with {
    ($res:expr, $settings:expr) => {
        $res.Mic().start_with($settings)
    };
}

#[macro_export]
macro_rules! mic_record {
    ($res:expr) => {
        $res.Mic().record()
    };
}

#[macro_export]
macro_rules! mic_stop {
    ($res:expr) => {
        $res.Mic().stop_listening()
    };
}

#[macro_export]
macro_rules! mic_stop_listening {
    ($res:expr) => {
        $res.Mic().stop_listening()
    };
}

#[macro_export]
macro_rules! mic_stop_stream {
    ($res:expr) => {
        $res.Mic().stop_stream()
    };
}

#[macro_export]
macro_rules! mic_clip {
    ($res:expr) => {
        $res.Mic().clip()
    };
}

#[macro_export]
macro_rules! mic_stream_clip {
    ($res:expr) => {
        $res.Mic().stream_clip()
    };
}

#[macro_export]
macro_rules! mic_stream_bytes {
    ($res:expr) => {
        $res.Mic().stream_bytes()
    };
}

#[macro_export]
macro_rules! mic_get_clip {
    ($res:expr) => {
        $res.Mic().get_clip()
    };
}

#[macro_export]
macro_rules! mic_get_bytes {
    ($res:expr) => {
        $res.Mic().get_bytes()
    };
}

#[macro_export]
macro_rules! mic_frame {
    ($res:expr) => {
        $res.Mic().stream_clip()
    };
}

#[macro_export]
macro_rules! mic_frame_bytes {
    ($res:expr) => {
        $res.Mic().stream_bytes()
    };
}

#[macro_export]
macro_rules! mic_is_listening {
    ($res:expr) => {
        $res.Mic().is_listening()
    };
}

#[macro_export]
macro_rules! mic_play {
    ($res:expr, $clip:expr) => {
        $res.Mic().play_master($clip)
    };
    ($res:expr, $bus_id:expr, $clip:expr) => {
        $res.Mic().play_bus($bus_id, $clip)
    };
    ($res:expr, $bus_id:expr, $clip:expr, $volume:expr) => {
        $res.Mic().play_bus_volume($bus_id, $clip, $volume)
    };
}

#[macro_export]
macro_rules! mic_play_master {
    ($res:expr, $clip:expr) => {
        $res.Mic().play_master($clip)
    };
}

#[macro_export]
macro_rules! mic_play_master_volume {
    ($res:expr, $clip:expr, $volume:expr) => {
        $res.Mic().play_master_volume($clip, $volume)
    };
}

#[macro_export]
macro_rules! mic_play_bus {
    ($res:expr, $bus_id:expr, $clip:expr) => {
        $res.Mic().play_bus($bus_id, $clip)
    };
}

#[macro_export]
macro_rules! mic_play_bus_volume {
    ($res:expr, $bus_id:expr, $clip:expr, $volume:expr) => {
        $res.Mic().play_bus_volume($bus_id, $clip, $volume)
    };
}

#[macro_export]
macro_rules! mic_save_wav {
    ($res:expr, $source:expr, $clip:expr) => {
        $res.Mic().save_wav($source, $clip)
    };
}

#[macro_export]
macro_rules! mic_pack {
    ($res:expr, $clip:expr) => {
        $res.Mic().pack($clip)
    };
}

#[macro_export]
macro_rules! mic_unpack {
    ($res:expr, $bytes:expr) => {
        $res.Mic().unpack($bytes)
    };
}
