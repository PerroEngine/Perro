use crate::ResPathSource;
use perro_pawdio::resolve_mic_device;
pub use perro_pawdio::{MicChannels, MicClip, MicDenoiseSettings, MicDevice, MicSettings};

pub trait MicAPI {
    fn mic_devices(&self) -> Result<Vec<MicDevice>, String>;
    fn mic_start(&self, settings: MicSettings) -> Result<(), String>;
    fn mic_stop(&self) -> Option<MicClip>;
    fn mic_clip(&self) -> Option<MicClip>;
    fn mic_stream_clip(&self) -> Option<MicClip>;
    fn mic_stream_bytes(&self) -> Option<Vec<u8>>;
    fn mic_is_listening(&self) -> bool;
    fn mic_device(&self) -> Option<String>;
    fn mic_last_error(&self) -> Option<String>;
    fn mic_level(&self) -> f32;
    fn mic_diagnostic(&self) -> Option<String>;
    fn mic_save_wav(&self, source: &str, clip: &MicClip) -> Result<(), String>;
}

pub struct MicModule<'res, R: MicAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: MicAPI + ?Sized> MicModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    /// Input devices connected right now. Rescans on every call.
    #[inline]
    pub fn devices(&self) -> Result<Vec<MicDevice>, String> {
        self.api.mic_devices()
    }

    #[inline]
    pub fn scan(&self) -> Result<Vec<MicDevice>, String> {
        self.devices()
    }

    /// Device the OS records as the input default.
    #[inline]
    pub fn default_device(&self) -> Option<MicDevice> {
        self.devices()
            .ok()?
            .into_iter()
            .find(|device| device.is_default)
    }

    /// Look up a cached device name in a fresh scan. `None` once it is unplugged.
    #[inline]
    pub fn find_device(&self, name: &str) -> Option<MicDevice> {
        let devices = self.devices().ok()?;
        let found = resolve_mic_device(&devices, Some(name))?;
        (found.name == name || found.name.eq_ignore_ascii_case(name.trim())).then(|| found.clone())
    }

    /// Cached device name if still present, else whatever the OS defaults to.
    #[inline]
    pub fn resolve_device(&self, name: Option<&str>) -> Option<MicDevice> {
        let devices = self.devices().ok()?;
        resolve_mic_device(&devices, name).cloned()
    }

    #[inline]
    pub fn has_device(&self, name: &str) -> bool {
        self.find_device(name).is_some()
    }

    #[inline]
    pub fn start_listening(&self) -> Result<(), String> {
        self.api.mic_start(MicSettings::default())
    }

    /// Capture from a named device. Errs when that device is gone.
    #[inline]
    pub fn start_on<S: Into<String>>(&self, device: S) -> Result<(), String> {
        self.api
            .mic_start(MicSettings::default().with_device(device))
    }

    #[inline]
    pub fn start_on_with<S: Into<String>>(
        &self,
        device: S,
        settings: MicSettings,
    ) -> Result<(), String> {
        self.api.mic_start(settings.with_device(device))
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

    /// Device name backing the current or last capture.
    #[inline]
    pub fn device(&self) -> Option<String> {
        self.api.mic_device()
    }

    /// Last capture error, including a device lost mid-stream.
    #[inline]
    pub fn last_error(&self) -> Option<String> {
        self.api.mic_last_error()
    }

    /// Live input peak, 0..=1. Drives level meters and sensitivity sliders in
    /// audio settings menus.
    #[inline]
    pub fn level(&self) -> f32 {
        self.api.mic_level()
    }

    /// Non-fatal capture health hint. Set while the stream delivers pure
    /// silence — the classic symptom of an OS microphone-permission block.
    #[inline]
    pub fn diagnostic(&self) -> Option<String> {
        self.api.mic_diagnostic()
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
macro_rules! mic_devices {
    ($res:expr) => {
        $res.Mic().devices()
    };
}

#[macro_export]
macro_rules! mic_scan {
    ($res:expr) => {
        $res.Mic().scan()
    };
}

#[macro_export]
macro_rules! mic_default_device {
    ($res:expr) => {
        $res.Mic().default_device()
    };
}

#[macro_export]
macro_rules! mic_find_device {
    ($res:expr, $name:expr) => {
        $res.Mic().find_device($name)
    };
}

#[macro_export]
macro_rules! mic_resolve_device {
    ($res:expr, $name:expr) => {
        $res.Mic().resolve_device($name)
    };
}

#[macro_export]
macro_rules! mic_has_device {
    ($res:expr, $name:expr) => {
        $res.Mic().has_device($name)
    };
}

#[macro_export]
macro_rules! mic_start_on {
    ($res:expr, $device:expr) => {
        $res.Mic().start_on($device)
    };
    ($res:expr, $device:expr, $settings:expr) => {
        $res.Mic().start_on_with($device, $settings)
    };
}

#[macro_export]
macro_rules! mic_device {
    ($res:expr) => {
        $res.Mic().device()
    };
}

#[macro_export]
macro_rules! mic_last_error {
    ($res:expr) => {
        $res.Mic().last_error()
    };
}

#[macro_export]
macro_rules! mic_level {
    ($res:expr) => {
        $res.Mic().level()
    };
}

#[macro_export]
macro_rules! mic_diagnostic {
    ($res:expr) => {
        $res.Mic().diagnostic()
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mic backend with a fixed device list; no hardware involved.
    struct DummyMicApi {
        devices: Vec<MicDevice>,
        started: Mutex<Vec<MicSettings>>,
    }

    impl DummyMicApi {
        fn new(names: &[(&str, bool)]) -> Self {
            Self {
                devices: names
                    .iter()
                    .map(|(name, is_default)| MicDevice {
                        name: (*name).to_string(),
                        label: (*name).to_string(),
                        is_default: *is_default,
                        sample_rate: 48_000,
                        channels: 1,
                    })
                    .collect(),
                started: Mutex::new(Vec::new()),
            }
        }

        fn last_start(&self) -> Option<MicSettings> {
            self.started.lock().ok()?.last().cloned()
        }
    }

    impl MicAPI for DummyMicApi {
        fn mic_devices(&self) -> Result<Vec<MicDevice>, String> {
            Ok(self.devices.clone())
        }

        fn mic_start(&self, settings: MicSettings) -> Result<(), String> {
            if let Some(wanted) = settings.requested_device()
                && !self.devices.iter().any(|device| device.name == wanted)
            {
                return Err(format!("mic device `{wanted}` not connected"));
            }
            if let Ok(mut started) = self.started.lock() {
                started.push(settings);
            }
            Ok(())
        }

        fn mic_stop(&self) -> Option<MicClip> {
            None
        }

        fn mic_clip(&self) -> Option<MicClip> {
            None
        }

        fn mic_stream_clip(&self) -> Option<MicClip> {
            None
        }

        fn mic_stream_bytes(&self) -> Option<Vec<u8>> {
            None
        }

        fn mic_is_listening(&self) -> bool {
            false
        }

        fn mic_device(&self) -> Option<String> {
            self.last_start()?
                .requested_device()
                .map(ToString::to_string)
        }

        fn mic_last_error(&self) -> Option<String> {
            None
        }

        fn mic_level(&self) -> f32 {
            0.0
        }

        fn mic_diagnostic(&self) -> Option<String> {
            None
        }

        fn mic_save_wav(&self, _source: &str, _clip: &MicClip) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn devices_expose_names_labels_and_default_flag() {
        let api = DummyMicApi::new(&[("Built-in Mic", true), ("USB Mic", false)]);
        let module = MicModule::new(&api);
        let devices = module.devices().expect("scan devices");
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].label, "Built-in Mic");
        assert_eq!(
            module.default_device().map(|device| device.name),
            Some("Built-in Mic".to_string())
        );
    }

    #[test]
    fn start_listening_uses_os_default_device() {
        let api = DummyMicApi::new(&[("Built-in Mic", true)]);
        let module = MicModule::new(&api);
        module.start_listening().expect("start default");
        let settings = api.last_start().expect("recorded settings");
        assert_eq!(settings.requested_device(), None);
    }

    #[test]
    fn start_on_plumbs_device_into_settings() {
        let api = DummyMicApi::new(&[("Built-in Mic", true), ("USB Mic", false)]);
        let module = MicModule::new(&api);
        module.start_on("USB Mic").expect("start on usb mic");
        let settings = api.last_start().expect("recorded settings");
        assert_eq!(settings.requested_device(), Some("USB Mic"));
        assert_eq!(settings.max_seconds, MicSettings::default().max_seconds);
    }

    #[test]
    fn start_on_with_keeps_caller_settings() {
        let api = DummyMicApi::new(&[("USB Mic", false)]);
        let module = MicModule::new(&api);
        let settings = MicSettings::default()
            .with_max_seconds(4.0)
            .with_denoise(MicDenoiseSettings::voice())
            .with_channels(MicChannels::Mono);
        module
            .start_on_with("USB Mic", settings)
            .expect("start on usb mic");
        let settings = api.last_start().expect("recorded settings");
        assert_eq!(settings.requested_device(), Some("USB Mic"));
        assert_eq!(settings.max_seconds, 4.0);
        assert!(settings.denoise.enabled);
        assert_eq!(settings.channels, MicChannels::Mono);
    }

    /// A cached name whose device is unplugged must fail loudly, not silently swap.
    #[test]
    fn start_on_missing_device_reports_error() {
        let api = DummyMicApi::new(&[("Built-in Mic", true)]);
        let module = MicModule::new(&api);
        let err = module.start_on("USB Mic").expect_err("missing device errs");
        assert!(err.contains("USB Mic"), "{err}");
        assert!(api.last_start().is_none());
    }

    #[test]
    fn cached_name_lookup_survives_device_reorder() {
        let api = DummyMicApi::new(&[("Virtual Cable", false), ("USB Mic", false)]);
        let module = MicModule::new(&api);
        assert!(module.has_device("USB Mic"));
        assert_eq!(
            module.find_device("USB Mic").map(|device| device.name),
            Some("USB Mic".to_string())
        );
        assert!(module.find_device("Gone Mic").is_none());
    }

    #[test]
    fn resolve_device_falls_back_to_default() {
        let api = DummyMicApi::new(&[("Built-in Mic", true), ("USB Mic", false)]);
        let module = MicModule::new(&api);
        assert_eq!(
            module
                .resolve_device(Some("Gone Mic"))
                .map(|device| device.name),
            Some("Built-in Mic".to_string())
        );
        assert_eq!(
            module
                .resolve_device(Some("USB Mic"))
                .map(|device| device.name),
            Some("USB Mic".to_string())
        );
    }

    /// The menu flow: scan, pick, cache the name, start on the cached name.
    #[test]
    fn menu_selection_round_trips_through_settings() {
        let api = DummyMicApi::new(&[("Built-in Mic", true), ("USB Mic", false)]);
        let module = MicModule::new(&api);
        let devices = module.devices().expect("scan devices");
        let cached = devices[1].name.clone();
        let picked = module
            .resolve_device(Some(&cached))
            .expect("cached device present");
        module
            .start_with(picked.settings())
            .expect("start on cached device");
        assert_eq!(module.device(), Some("USB Mic".to_string()));
    }
}
