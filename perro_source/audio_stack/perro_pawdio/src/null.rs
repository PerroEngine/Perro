use crate::midi::{MidiFileRequest, MidiNoteHandle, MidiNoteRequest};
use crate::{AudioPan, AudioPlaybackRequest, SpatialAudioParams};
use perro_ids::{AudioBusID, SoundFontID};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioEnqueueError {
    Disconnected,
}

impl fmt::Display for AudioEnqueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("audio disabled")
    }
}

impl std::error::Error for AudioEnqueueError {}

pub type AudioEnqueueResult<T = ()> = Result<T, AudioEnqueueError>;

#[derive(Clone)]
pub struct AudioSourceHandle(Arc<str>);

impl AudioSourceHandle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn shared_str(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

#[derive(Clone, Copy, Default)]
pub struct AudioLengthProber;

impl AudioLengthProber {
    pub fn source_length_seconds(&self, _source: &str) -> Option<f32> {
        None
    }
}

#[derive(Default)]
pub struct AudioController;

impl AudioController {
    pub fn new(_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        Err("audio disabled".to_string())
    }

    pub fn source_handle(&self, source: &str) -> AudioSourceHandle {
        AudioSourceHandle(Arc::from(source))
    }
    pub fn play_source(&self, _request: AudioPlaybackRequest<'_>) -> bool {
        false
    }
    pub fn play_clip(
        &self,
        _source: &str,
        _clip: MicClip,
        _bus: Option<AudioBusID>,
        _volume: f32,
        _pan: AudioPan,
    ) -> bool {
        false
    }
    pub fn play_stream_clip(
        &self,
        _source: &str,
        _clip: MicClip,
        _bus: Option<AudioBusID>,
        _volume: f32,
        _pan: AudioPan,
    ) -> bool {
        false
    }
    pub fn play_source_handle(
        &self,
        _handle: &AudioSourceHandle,
        _request: AudioPlaybackRequest<'_>,
    ) -> bool {
        false
    }
    pub fn play_spatial_source(&self, _request: AudioPlaybackRequest<'_>) -> Option<u64> {
        None
    }
    pub fn play_spatial_source_handle(
        &self,
        _handle: &AudioSourceHandle,
        _request: AudioPlaybackRequest<'_>,
    ) -> Option<u64> {
        None
    }
    pub fn update_spatial(&self, _id: u64, _params: SpatialAudioParams) -> bool {
        false
    }
    pub fn stop_playback(&self, _id: u64) -> bool {
        false
    }
    pub fn load_source(&self, _source: &str) -> bool {
        false
    }
    pub fn load_source_bytes(&self, _source: &str, _bytes: Arc<[u8]>) -> bool {
        false
    }
    pub fn is_source_loaded(&self, _source: &str) -> bool {
        false
    }
    pub fn reserve_source(&self, _source: &str) -> bool {
        false
    }
    pub fn reserve_source_bytes(&self, _source: &str, _bytes: Arc<[u8]>) -> bool {
        false
    }
    pub fn drop_source(&self, _source: &str) -> bool {
        false
    }
    pub fn length_prober(&self) -> AudioLengthProber {
        AudioLengthProber
    }
    pub fn source_length_seconds(&self, _source: &str) -> Option<f32> {
        None
    }
    pub fn stop_source(&self, _source: &str) -> bool {
        false
    }
    pub fn stop_match(&self, _request: AudioPlaybackRequest<'_>) -> bool {
        false
    }
    pub fn stop_all(&self) -> bool {
        false
    }
    pub fn set_master_volume(&self, _volume: f32) -> bool {
        false
    }
    pub fn set_bus_volume(&self, _bus: AudioBusID, _volume: f32) -> bool {
        false
    }
    pub fn set_bus_speed(&self, _bus: AudioBusID, _speed: f32) -> bool {
        false
    }
    pub fn pause_bus(&self, _bus: AudioBusID) -> bool {
        false
    }
    pub fn resume_bus(&self, _bus: AudioBusID) -> bool {
        false
    }
    pub fn stop_bus(&self, _bus: AudioBusID) -> bool {
        false
    }
    pub fn load_soundfont(&self, source: &str) -> SoundFontID {
        SoundFontID::from_string(source)
    }
    pub fn load_soundfont_with_id(&self, id: SoundFontID, _source: &str) -> SoundFontID {
        id
    }
    pub fn load_soundfont_bytes_with_id(
        &self,
        id: SoundFontID,
        _source: &str,
        _bytes: Arc<[u8]>,
    ) -> SoundFontID {
        id
    }
    pub fn is_soundfont_loaded(&self, _id: SoundFontID) -> bool {
        false
    }
    pub fn load_midi_file(&self, _source: &str) -> bool {
        false
    }
    pub fn play_midi_note(&self, _request: MidiNoteRequest) -> bool {
        false
    }
    pub fn start_midi_note(&self, _request: MidiNoteRequest) -> Option<MidiNoteHandle> {
        None
    }
    pub fn play_spatial_midi_note(&self, _request: MidiNoteRequest) -> Option<u64> {
        None
    }
    pub fn play_midi_note_spatial(&self, _request: MidiNoteRequest) -> bool {
        false
    }
    pub fn play_midi_file(&self, _request: MidiFileRequest<'_>) -> bool {
        false
    }
    pub fn play_spatial_midi_file(&self, _request: MidiFileRequest<'_>) -> Option<u64> {
        None
    }
    pub fn release_midi_note(&self, _handle: MidiNoteHandle) -> bool {
        false
    }
}

pub struct BarkPlayer;

#[derive(Clone, Debug, PartialEq)]
pub struct MicClip {
    samples: Arc<[i16]>,
    sample_rate: u32,
    channels: u16,
}

impl MicClip {
    pub fn new(samples: Vec<i16>, sample_rate: u32, channels: u16) -> Self {
        Self::try_new(samples, sample_rate, channels).expect("invalid mic clip format")
    }

    pub fn try_new(samples: Vec<i16>, sample_rate: u32, channels: u16) -> Result<Self, String> {
        if sample_rate == 0 || channels == 0 || !samples.len().is_multiple_of(channels as usize) {
            return Err("invalid mic clip format".to_string());
        }
        Ok(Self {
            samples: Arc::from(samples),
            sample_rate,
            channels,
        })
    }

    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub const fn channels(&self) -> u16 {
        self.channels
    }
    pub fn duration(&self) -> Duration {
        Duration::from_secs_f64(
            self.samples.len() as f64
                / self.channels.max(1) as f64
                / self.sample_rate.max(1) as f64,
        )
    }
    pub fn seconds(&self) -> f32 {
        self.duration().as_secs_f32()
    }
    pub fn pack(&self) -> Vec<u8> {
        self.raw_bytes()
    }
    pub fn unpack(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 16 || &bytes[..4] != b"PMIC" {
            return Err("mic clip header invalid".to_string());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != 1 {
            return Err(format!("unsupported mic clip version {version}"));
        }
        let channels = u16::from_le_bytes([bytes[6], bytes[7]]);
        let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let frames = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let sample_count = frames
            .checked_mul(channels as usize)
            .ok_or_else(|| "mic clip size overflow".to_string())?;
        let payload = &bytes[16..];
        if payload.len()
            != sample_count
                .checked_mul(2)
                .ok_or_else(|| "mic clip size overflow".to_string())?
        {
            return Err("mic clip payload size mismatch".to_string());
        }
        let samples = payload
            .chunks_exact(2)
            .map(|v| i16::from_le_bytes([v[0], v[1]]))
            .collect();
        Self::try_new(samples, sample_rate, channels)
    }
    pub fn raw_bytes(&self) -> Vec<u8> {
        let frames = (self.samples.len() / self.channels as usize) as u32;
        let mut out = Vec::with_capacity(16 + self.samples.len() * 2);
        out.extend_from_slice(b"PMIC");
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.channels.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        out.extend(self.samples.iter().flat_map(|v| v.to_le_bytes()));
        out
    }
    pub fn compressed_bytes(&self) -> Vec<u8> {
        self.pack()
    }
    pub fn byte_len(&self) -> usize {
        self.raw_byte_len()
    }
    pub fn raw_byte_len(&self) -> usize {
        16 + self.samples.len() * 2
    }
    pub fn compression_ratio(&self) -> f32 {
        1.0
    }
    pub fn wav_bytes(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let byte_rate = self.sample_rate * self.channels as u32 * 2;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.channels.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&(self.channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend(self.samples.iter().flat_map(|v| v.to_le_bytes()));
        out
    }
    pub fn samples_f32(&self) -> Vec<f32> {
        self.samples
            .iter()
            .map(|v| *v as f32 / i16::MAX as f32)
            .collect()
    }
    pub fn denoised(&self, _settings: MicDenoiseSettings) -> Self {
        self.clone()
    }
}

#[derive(Clone, Debug)]
pub struct MicSettings {
    pub max_seconds: f32,
    pub denoise: MicDenoiseSettings,
    pub device: Option<String>,
    pub channels: MicChannels,
    pub gain: f32,
    pub auto_gain: bool,
}

impl Default for MicSettings {
    fn default() -> Self {
        Self {
            max_seconds: 30.0,
            denoise: MicDenoiseSettings::off(),
            device: None,
            channels: MicChannels::Auto,
            gain: 1.0,
            auto_gain: false,
        }
    }
}

impl MicSettings {
    pub const GAIN_RANGE: std::ops::RangeInclusive<f32> = 0.0..=8.0;
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }
    pub fn with_default_device(mut self) -> Self {
        self.device = None;
        self
    }
    pub fn with_max_seconds(mut self, seconds: f32) -> Self {
        self.max_seconds = seconds;
        self
    }
    pub fn with_denoise(mut self, denoise: MicDenoiseSettings) -> Self {
        self.denoise = denoise;
        self
    }
    pub fn with_channels(mut self, channels: MicChannels) -> Self {
        self.channels = channels;
        self
    }
    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = gain.clamp(0.0, 8.0);
        self
    }
    pub fn with_auto_gain(mut self, auto_gain: bool) -> Self {
        self.auto_gain = auto_gain;
        self
    }
    pub fn clamped_gain(&self) -> f32 {
        if self.gain.is_finite() {
            self.gain.clamp(0.0, 8.0)
        } else {
            1.0
        }
    }
    pub fn requested_device(&self) -> Option<&str> {
        self.device
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MicChannels {
    #[default]
    Auto,
    Mono,
    Device,
}

impl MicChannels {
    pub fn output_channels(self, channels: u16) -> u16 {
        match self {
            Self::Auto if channels > 2 => 1,
            Self::Auto | Self::Device => channels.max(1),
            Self::Mono => 1,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MicDevice {
    pub name: String,
    pub label: String,
    pub is_default: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

impl MicDevice {
    /// Default capture settings targeting this device.
    #[inline]
    pub fn settings(&self) -> MicSettings {
        MicSettings::default().with_device(&self.name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MicDenoiseSettings {
    pub enabled: bool,
    pub noise_floor: f32,
    pub reduction: f32,
    pub high_pass: bool,
    pub high_pass_hz: f32,
}

impl MicDenoiseSettings {
    pub fn off() -> Self {
        Self::default()
    }
    pub fn voice() -> Self {
        Self {
            enabled: true,
            noise_floor: 0.015,
            reduction: 0.8,
            high_pass: true,
            high_pass_hz: 80.0,
        }
    }
    pub fn strong() -> Self {
        Self {
            enabled: true,
            noise_floor: 0.03,
            reduction: 1.0,
            high_pass: true,
            high_pass_hz: 100.0,
        }
    }
    pub fn with_noise_floor(mut self, value: f32) -> Self {
        self.noise_floor = value;
        self
    }
    pub fn with_reduction(mut self, value: f32) -> Self {
        self.reduction = value;
        self
    }
    pub fn with_high_pass_hz(mut self, value: f32) -> Self {
        self.high_pass = true;
        self.high_pass_hz = value;
        self
    }
}

pub fn mic_devices() -> Result<Vec<MicDevice>, String> {
    Ok(Vec::new())
}

pub fn resolve_mic_device<'a>(
    devices: &'a [MicDevice],
    wanted: Option<&str>,
) -> Option<&'a MicDevice> {
    wanted
        .and_then(|name| devices.iter().find(|item| item.name == name))
        .or_else(|| devices.first())
}

#[derive(Default)]
pub struct MicRecorder;

impl MicRecorder {
    pub fn new() -> Self {
        Self
    }
    pub fn devices(&self) -> Result<Vec<MicDevice>, String> {
        mic_devices()
    }
    pub fn is_listening(&self) -> bool {
        false
    }
    pub fn device(&self) -> Option<String> {
        None
    }
    pub fn last_error(&self) -> Option<String> {
        None
    }
    pub fn level(&self) -> f32 {
        0.0
    }
    pub fn diagnostic(&self) -> Option<String> {
        None
    }
    pub fn start(&mut self, _settings: MicSettings) -> Result<(), String> {
        Err("audio disabled".to_string())
    }
    pub fn stop(&mut self) -> Option<MicClip> {
        None
    }
    pub fn clip(&self) -> Option<MicClip> {
        None
    }
    pub fn stream_clip(&self) -> Option<MicClip> {
        None
    }
    pub fn stream_bytes(&self) -> Option<Vec<u8>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mic_v1_round_trip_stays_available_without_backend() {
        let clip = MicClip::new(vec![1, -2, 3, -4], 48_000, 2);
        assert_eq!(MicClip::unpack(&clip.pack()).expect("unpack mic clip"), clip);
        assert_eq!(&clip.wav_bytes()[..4], b"RIFF");
    }
}
