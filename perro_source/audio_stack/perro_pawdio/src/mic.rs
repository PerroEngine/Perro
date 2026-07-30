#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, Mutex};
use std::time::Duration;

const PMIC_MAGIC: &[u8; 4] = b"PMIC";
const PMIC_VERSION: u16 = 1;
const PMIC_HEADER_LEN: usize = 16;
const PMIC_VERSION_COMPRESSED: u16 = 2;
const PMIC_COMPRESSED_HEADER_LEN: usize = 20;
const PMIC_CODEC_PCM: u8 = 0;
const PMIC_CODEC_ZLIB_PCM: u8 = 1;
const PMIC_CODEC_DELTA: u8 = 2;
const PMIC_CODEC_ZLIB_DELTA: u8 = 3;
/// Hard ceiling on the sample count a packet header may declare.
///
/// Header frames/channels are attacker controlled on any network path, and the
/// product feeds allocation sizes and decompression limits. `1 << 26` samples is
/// ~11.6 minutes of 48 kHz stereo, far past any mic capture, while keeping the
/// worst-case decode buffer bounded.
const PMIC_MAX_SAMPLES: usize = 1 << 26;

#[derive(Clone, Debug, PartialEq)]
pub struct MicClip {
    samples: Vec<i16>,
    sample_rate: u32,
    channels: u16,
}

impl MicClip {
    /// Creates a clip and panics when its format cannot be encoded losslessly.
    ///
    /// Prefer [`Self::try_new`] for data that is not already trusted.
    pub fn new(samples: Vec<i16>, sample_rate: u32, channels: u16) -> Self {
        Self::try_new(samples, sample_rate, channels).expect("invalid mic clip format")
    }

    /// Creates a clip after validating channel frames and encoder size limits.
    pub fn try_new(samples: Vec<i16>, sample_rate: u32, channels: u16) -> Result<Self, String> {
        if sample_rate == 0 {
            return Err("mic clip sample rate must be non-zero".to_string());
        }
        if channels == 0 {
            return Err("mic clip channel count must be non-zero".to_string());
        }
        let channel_count = channels as usize;
        if !samples.len().is_multiple_of(channel_count) {
            return Err(format!(
                "mic clip sample count {} is not divisible by {channels} channels",
                samples.len()
            ));
        }
        let frames = samples.len() / channel_count;
        u32::try_from(frames).map_err(|_| "mic clip frame count exceeds u32".to_string())?;
        let data_len = samples
            .len()
            .checked_mul(std::mem::size_of::<i16>())
            .ok_or_else(|| "mic clip byte length overflow".to_string())?;
        let data_len =
            u32::try_from(data_len).map_err(|_| "mic clip WAV data exceeds u32".to_string())?;
        data_len
            .checked_add(36)
            .ok_or_else(|| "mic clip WAV RIFF length exceeds u32".to_string())?;
        channels
            .checked_mul(std::mem::size_of::<i16>() as u16)
            .ok_or_else(|| "mic clip WAV block alignment exceeds u16".to_string())?;
        sample_rate
            .checked_mul(channels as u32)
            .and_then(|rate| rate.checked_mul(std::mem::size_of::<i16>() as u32))
            .ok_or_else(|| "mic clip WAV byte rate exceeds u32".to_string())?;

        Ok(Self {
            samples,
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
        let frames = self.samples.len() as f64 / self.channels.max(1) as f64;
        Duration::from_secs_f64(frames / self.sample_rate.max(1) as f64)
    }

    pub fn seconds(&self) -> f32 {
        self.duration().as_secs_f32()
    }

    pub fn pack(&self) -> Vec<u8> {
        let frames = (self.samples.len() / self.channels.max(1) as usize) as u32;
        let v1 = self.pack_v1(frames);
        let mut best = v1;
        let pcm = pcm_payload(&self.samples);

        try_best_pmic(
            &mut best,
            PMIC_CODEC_PCM,
            self.channels,
            self.sample_rate,
            frames,
            pcm.clone(),
        );

        if let Ok(compressed) = perro_io::compress_zlib_best(&pcm) {
            try_best_pmic(
                &mut best,
                PMIC_CODEC_ZLIB_PCM,
                self.channels,
                self.sample_rate,
                frames,
                compressed,
            );
        }

        let delta = delta_payload(&self.samples, self.channels);
        try_best_pmic(
            &mut best,
            PMIC_CODEC_DELTA,
            self.channels,
            self.sample_rate,
            frames,
            delta.clone(),
        );

        if let Ok(compressed) = perro_io::compress_zlib_best(&delta) {
            try_best_pmic(
                &mut best,
                PMIC_CODEC_ZLIB_DELTA,
                self.channels,
                self.sample_rate,
                frames,
                compressed,
            );
        }

        best
    }

    fn pack_v1(&self, frames: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(PMIC_HEADER_LEN + self.samples.len() * 2);
        out.extend_from_slice(PMIC_MAGIC);
        out.extend_from_slice(&PMIC_VERSION.to_le_bytes());
        out.extend_from_slice(&self.channels.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        for sample in &self.samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    pub fn unpack(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < PMIC_HEADER_LEN {
            return Err("mic clip too small".to_string());
        }
        if &bytes[..4] != PMIC_MAGIC {
            return Err("mic clip magic mismatch".to_string());
        }
        match u16::from_le_bytes([bytes[4], bytes[5]]) {
            PMIC_VERSION => Self::unpack_v1(bytes),
            PMIC_VERSION_COMPRESSED => Self::unpack_v2(bytes),
            version => Err(format!("unsupported mic clip version {version}")),
        }
    }

    fn unpack_v1(bytes: &[u8]) -> Result<Self, String> {
        let channels = u16::from_le_bytes([bytes[6], bytes[7]]);
        let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let frames = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let payload = &bytes[PMIC_HEADER_LEN..];
        let samples = decode_pcm_payload(payload, frames, channels)?;
        Self::try_new(samples, sample_rate, channels)
    }

    fn unpack_v2(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < PMIC_COMPRESSED_HEADER_LEN {
            return Err("mic clip v2 too small".to_string());
        }
        let channels = u16::from_le_bytes([bytes[6], bytes[7]]);
        let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let frames = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let codec = bytes[16];
        let expected_samples = checked_sample_len(frames, channels)?;
        let payload = &bytes[PMIC_COMPRESSED_HEADER_LEN..];
        let samples = match codec {
            PMIC_CODEC_PCM => decode_pcm_payload(payload, frames, channels)?,
            PMIC_CODEC_ZLIB_PCM => {
                let limit = checked_zlib_limit(expected_samples, 2)?;
                let decoded = perro_io::decompress_zlib_limited(payload, limit)
                    .map_err(|err| format!("mic clip zlib decode failed: {err}"))?;
                decode_pcm_payload(&decoded, frames, channels)?
            }
            PMIC_CODEC_DELTA => decode_delta_payload(payload, expected_samples, channels)?,
            PMIC_CODEC_ZLIB_DELTA => {
                let limit = checked_zlib_limit(expected_samples, 3)?;
                let decoded = perro_io::decompress_zlib_limited(payload, limit)
                    .map_err(|err| format!("mic clip zlib delta decode failed: {err}"))?;
                decode_delta_payload(&decoded, expected_samples, channels)?
            }
            other => return Err(format!("unsupported mic clip codec {other}")),
        };
        Self::try_new(samples, sample_rate, channels)
    }

    pub fn raw_bytes(&self) -> Vec<u8> {
        self.pack_v1((self.samples.len() / self.channels.max(1) as usize) as u32)
    }

    pub fn compressed_bytes(&self) -> Vec<u8> {
        self.pack()
    }

    pub fn byte_len(&self) -> usize {
        self.pack().len()
    }

    pub fn raw_byte_len(&self) -> usize {
        PMIC_HEADER_LEN + self.samples.len() * 2
    }

    pub fn compression_ratio(&self) -> f32 {
        let raw = self.raw_byte_len().max(1) as f32;
        self.byte_len() as f32 / raw
    }

    pub fn wav_bytes(&self) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let byte_rate = self.sample_rate * self.channels as u32 * 2;
        let block_align = self.channels * 2;
        let mut out = Vec::with_capacity(44 + self.samples.len() * 2);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&self.channels.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in &self.samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    pub fn samples_f32(&self) -> Vec<f32> {
        self.samples
            .iter()
            .map(|sample| *sample as f32 / i16::MAX as f32)
            .collect()
    }

    pub fn denoised(&self, settings: MicDenoiseSettings) -> Self {
        if !settings.enabled {
            return self.clone();
        }
        let mut state = MicDenoiseState::new(settings);
        let samples = self
            .samples
            .iter()
            .map(|sample| state.process_i16(*sample))
            .collect();
        Self::new(samples, self.sample_rate, self.channels)
    }
}

fn decode_pcm_payload(payload: &[u8], frames: usize, channels: u16) -> Result<Vec<i16>, String> {
    if !payload.len().is_multiple_of(2) {
        return Err("mic clip odd payload len".to_string());
    }
    let expected_samples = checked_sample_len(frames, channels)?;
    if payload.len() / 2 != expected_samples {
        return Err(format!(
            "mic clip len mismatch: expect {}, got {}",
            expected_samples,
            payload.len() / 2
        ));
    }
    let mut samples = Vec::with_capacity(expected_samples);
    for chunk in payload.chunks_exact(2) {
        samples.push(i16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(samples)
}

fn checked_sample_len(frames: usize, channels: u16) -> Result<usize, String> {
    let len = frames
        .checked_mul(channels as usize)
        .ok_or_else(|| "mic clip sample len overflow".to_string())?;
    if len > PMIC_MAX_SAMPLES {
        return Err(format!(
            "mic clip sample len {len} exceeds limit {PMIC_MAX_SAMPLES}"
        ));
    }
    Ok(len)
}

/// Decompression ceiling for a codec that expands to `bytes_per_sample` per sample.
///
/// `usize` is 32-bit on wasm32, so the multiply is checked rather than assumed.
fn checked_zlib_limit(expected_samples: usize, bytes_per_sample: usize) -> Result<usize, String> {
    expected_samples
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| "mic clip zlib limit overflow".to_string())
}

fn pcm_payload(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

fn try_best_pmic(
    best: &mut Vec<u8>,
    codec: u8,
    channels: u16,
    sample_rate: u32,
    frames: u32,
    payload: Vec<u8>,
) {
    let mut packed = Vec::with_capacity(PMIC_COMPRESSED_HEADER_LEN + payload.len());
    packed.extend_from_slice(PMIC_MAGIC);
    packed.extend_from_slice(&PMIC_VERSION_COMPRESSED.to_le_bytes());
    packed.extend_from_slice(&channels.to_le_bytes());
    packed.extend_from_slice(&sample_rate.to_le_bytes());
    packed.extend_from_slice(&frames.to_le_bytes());
    packed.push(codec);
    packed.extend_from_slice(&[0, 0, 0]);
    packed.extend_from_slice(&payload);
    if packed.len() < best.len() {
        *best = packed;
    }
}

fn delta_payload(samples: &[i16], channels: u16) -> Vec<u8> {
    let channels = channels.max(1) as usize;
    let mut prev = vec![0i16; channels];
    let mut out = Vec::with_capacity(samples.len());
    for (index, sample) in samples.iter().enumerate() {
        let channel = index % channels;
        let delta = sample.wrapping_sub(prev[channel]);
        prev[channel] = *sample;
        write_varint(zigzag_i16(delta), &mut out);
    }
    out
}

fn decode_delta_payload(
    payload: &[u8],
    expected_samples: usize,
    channels: u16,
) -> Result<Vec<i16>, String> {
    let channels = channels.max(1) as usize;
    // Each sample costs at least one varint byte, so the payload length is a hard
    // upper bound on decodable samples. Check before reserving: `expected_samples`
    // comes from the packet header, and an over-large `with_capacity` aborts the
    // process instead of returning an error.
    if expected_samples > payload.len() {
        return Err(format!(
            "mic clip delta len mismatch: expect {expected_samples}, payload holds at most {}",
            payload.len()
        ));
    }
    let mut prev = vec![0i16; channels];
    let mut samples = Vec::with_capacity(expected_samples);
    let mut cursor = 0usize;
    while cursor < payload.len() && samples.len() < expected_samples {
        let value = read_varint(payload, &mut cursor)?;
        let channel = samples.len() % channels;
        let delta = unzigzag_i16(value);
        let sample = prev[channel].wrapping_add(delta);
        prev[channel] = sample;
        samples.push(sample);
    }
    if samples.len() != expected_samples {
        return Err(format!(
            "mic clip delta len mismatch: expect {}, got {}",
            expected_samples,
            samples.len()
        ));
    }
    if cursor != payload.len() {
        return Err("mic clip delta trailing bytes".to_string());
    }
    Ok(samples)
}

fn write_varint(mut value: u16, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn read_varint(payload: &[u8], cursor: &mut usize) -> Result<u16, String> {
    let mut value = 0u32;
    let mut shift = 0u32;
    for _ in 0..3 {
        let Some(byte) = payload.get(*cursor).copied() else {
            return Err("mic clip delta truncated varint".to_string());
        };
        *cursor += 1;
        value |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return u16::try_from(value).map_err(|_| "mic clip delta varint overflow".to_string());
        }
        shift += 7;
    }
    Err("mic clip delta varint too long".to_string())
}

fn zigzag_i16(value: i16) -> u16 {
    ((value << 1) ^ (value >> 15)) as u16
}

fn unzigzag_i16(value: u16) -> i16 {
    ((value >> 1) as i16) ^ (-((value & 1) as i16))
}

#[derive(Clone, Debug)]
pub struct MicSettings {
    pub max_seconds: f32,
    pub denoise: MicDenoiseSettings,
    /// Backend device name from [`mic_devices`]. `None` or blank picks the OS default.
    pub device: Option<String>,
    pub channels: MicChannels,
}

impl Default for MicSettings {
    fn default() -> Self {
        Self {
            max_seconds: 30.0,
            denoise: MicDenoiseSettings::off(),
            device: None,
            channels: MicChannels::Auto,
        }
    }
}

impl MicSettings {
    #[inline]
    pub fn with_device(mut self, device: impl Into<String>) -> Self {
        self.device = Some(device.into());
        self
    }

    #[inline]
    pub fn with_default_device(mut self) -> Self {
        self.device = None;
        self
    }

    #[inline]
    pub fn with_max_seconds(mut self, seconds: f32) -> Self {
        self.max_seconds = seconds;
        self
    }

    #[inline]
    pub fn with_denoise(mut self, denoise: MicDenoiseSettings) -> Self {
        self.denoise = denoise;
        self
    }

    #[inline]
    pub fn with_channels(mut self, channels: MicChannels) -> Self {
        self.channels = channels;
        self
    }

    /// Device name to open, treating blank selections as "use the OS default".
    pub fn requested_device(&self) -> Option<&str> {
        self.device
            .as_deref()
            .map(str::trim)
            .filter(|device| !device.is_empty())
    }
}

/// Channel layout the capture path writes into [`MicClip`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MicChannels {
    /// Keep mono and stereo devices as-is, fold wider interfaces to mono.
    #[default]
    Auto,
    /// Always fold every device channel into one mono stream.
    Mono,
    /// Never fold; clips keep the device channel count.
    Device,
}

impl MicChannels {
    /// Clip channel count for a device that captures `channels`.
    pub fn output_channels(self, channels: u16) -> u16 {
        let channels = channels.max(1);
        match self {
            Self::Auto => {
                if channels > 2 {
                    1
                } else {
                    channels
                }
            }
            Self::Mono => 1,
            Self::Device => channels,
        }
    }
}

/// Input device reported by the capture backend.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MicDevice {
    /// Backend name. Selection key for [`MicSettings::device`].
    pub name: String,
    /// Menu label. Same as `name`, suffixed when two devices share a name.
    pub label: String,
    pub is_default: bool,
    /// Default capture rate, `0` when the backend does not report one.
    pub sample_rate: u32,
    /// Default channel count, `0` when the backend does not report one.
    pub channels: u16,
}

impl MicDevice {
    /// Default capture settings targeting this device.
    #[inline]
    pub fn settings(&self) -> MicSettings {
        MicSettings::default().with_device(&self.name)
    }
}

/// Pick the device a cached name points at, else the default entry.
///
/// Backends reorder and renumber devices between scans, so a saved selection is
/// matched by name and never by list position.
pub fn resolve_mic_device<'a>(
    devices: &'a [MicDevice],
    wanted: Option<&str>,
) -> Option<&'a MicDevice> {
    if let Some(wanted) = wanted {
        let names: Vec<&str> = devices.iter().map(|device| device.name.as_str()).collect();
        if let Some(index) = match_device_index(&names, wanted) {
            return devices.get(index);
        }
    }
    devices
        .iter()
        .find(|device| device.is_default)
        .or_else(|| devices.first())
}

/// Index of `wanted`: exact match first, then trimmed case-insensitive match.
fn match_device_index(names: &[&str], wanted: &str) -> Option<usize> {
    let wanted = wanted.trim();
    if wanted.is_empty() {
        return None;
    }
    names.iter().position(|name| *name == wanted).or_else(|| {
        names
            .iter()
            .position(|name| name.trim().eq_ignore_ascii_case(wanted))
    })
}

/// Scan the input devices the OS exposes right now.
///
/// Wireless and USB mics come and go, so every call re-queries the backend
/// instead of serving a cached list.
#[cfg(not(target_arch = "wasm32"))]
pub fn mic_devices() -> Result<Vec<MicDevice>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let found = match host.input_devices() {
        Ok(devices) => devices.collect::<Vec<_>>(),
        Err(err) => {
            // Some backends refuse a full scan but still hand out the default.
            let Some(device) = host.default_input_device() else {
                return Err(format!("mic device scan failed: {err}"));
            };
            vec![device]
        }
    };

    let mut names = Vec::with_capacity(found.len());
    let mut formats = Vec::with_capacity(found.len());
    for device in &found {
        let Ok(name) = device.name() else {
            continue;
        };
        let format = device
            .default_input_config()
            .map(|config| (config.sample_rate().0, config.channels()))
            .unwrap_or((0, 0));
        names.push(name);
        formats.push(format);
    }

    let labels = dedupe_labels(&names);
    let mut default_taken = false;
    let devices = names
        .into_iter()
        .zip(labels)
        .zip(formats)
        .map(|((name, label), (sample_rate, channels))| {
            // Duplicate names share one default flag; the first entry wins.
            let is_default = !default_taken && default_name.as_deref() == Some(name.as_str());
            default_taken |= is_default;
            MicDevice {
                name,
                label,
                is_default,
                sample_rate,
                channels,
            }
        })
        .collect();
    Ok(devices)
}

#[cfg(target_arch = "wasm32")]
pub fn mic_devices() -> Result<Vec<MicDevice>, String> {
    Ok(Vec::new())
}

/// Menu labels that stay distinct when a backend lists twin devices.
#[cfg(not(target_arch = "wasm32"))]
fn dedupe_labels(names: &[String]) -> Vec<String> {
    let mut labels = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let seen = names[..index].iter().filter(|prev| *prev == name).count();
        if seen == 0 {
            labels.push(name.clone());
        } else {
            labels.push(format!("{name} #{}", seen + 1));
        }
    }
    labels
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MicDenoiseSettings {
    pub enabled: bool,
    pub noise_floor: f32,
    pub reduction: f32,
    pub high_pass: bool,
}

impl MicDenoiseSettings {
    pub fn off() -> Self {
        Self {
            enabled: false,
            noise_floor: 0.02,
            reduction: 0.75,
            high_pass: true,
        }
    }

    pub fn voice() -> Self {
        Self {
            enabled: true,
            noise_floor: 0.02,
            reduction: 0.75,
            high_pass: true,
        }
    }
}

impl Default for MicDenoiseSettings {
    fn default() -> Self {
        Self::off()
    }
}

#[derive(Clone, Copy, Debug)]
struct MicDenoiseState {
    settings: MicDenoiseSettings,
    prev_input: f32,
    prev_output: f32,
    gain: f32,
}

impl MicDenoiseState {
    fn new(settings: MicDenoiseSettings) -> Self {
        Self {
            settings,
            prev_input: 0.0,
            prev_output: 0.0,
            gain: 1.0,
        }
    }

    fn process_i16(&mut self, sample: i16) -> i16 {
        let sample = sample as f32 / i16::MAX as f32;
        (self.process_f32(sample) * i16::MAX as f32) as i16
    }

    /// Pass samples through untouched while denoise is off.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply(&mut self, sample: i16) -> i16 {
        if self.settings.enabled {
            self.process_i16(sample)
        } else {
            sample
        }
    }

    fn process_f32(&mut self, sample: f32) -> f32 {
        if !self.settings.enabled {
            return sample.clamp(-1.0, 1.0);
        }

        let mut out = sample.clamp(-1.0, 1.0);
        if self.settings.high_pass {
            let high = out - self.prev_input + 0.995 * self.prev_output;
            self.prev_input = out;
            self.prev_output = high;
            out = high;
        }

        let floor = self.settings.noise_floor.clamp(0.0, 1.0);
        let reduction = self.settings.reduction.clamp(0.0, 1.0);
        let target_gain = if out.abs() < floor {
            1.0 - reduction
        } else {
            1.0
        };
        let smoothing = if target_gain < self.gain { 0.02 } else { 0.2 };
        self.gain += (target_gain - self.gain) * smoothing;
        (out * self.gain).clamp(-1.0, 1.0)
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct ActiveMic {
    _stream: cpal::Stream,
}

#[cfg(not(target_arch = "wasm32"))]
enum MicCommand {
    Start {
        settings: MicSettings,
        reply: std::sync::mpsc::Sender<Result<(), String>>,
    },
    Stop {
        reply: std::sync::mpsc::Sender<()>,
    },
}

/// Format of the stream currently or most recently open.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct MicStreamMeta {
    device: String,
    sample_rate: u32,
    channels: u16,
}

/// State shared between the caller, the mic worker, and the audio callback.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct MicShared {
    samples: Arc<Mutex<Vec<i16>>>,
    cursor: Arc<Mutex<usize>>,
    meta: Arc<Mutex<Option<MicStreamMeta>>>,
    /// Capture healthy. Cleared on stop and on device loss.
    listening: Arc<AtomicBool>,
    /// Stream opened and not yet stopped, even after a device error.
    armed: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MicShared {
    fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            cursor: Arc::new(Mutex::new(0)),
            meta: Arc::new(Mutex::new(None)),
            listening: Arc::new(AtomicBool::new(false)),
            armed: Arc::new(AtomicBool::new(false)),
            error: Arc::new(Mutex::new(None)),
        }
    }

    fn set_error(&self, err: Option<String>) {
        if let Ok(mut slot) = self.error.lock() {
            *slot = err;
        }
    }
}

pub struct MicRecorder {
    #[cfg(not(target_arch = "wasm32"))]
    tx: std::sync::mpsc::Sender<MicCommand>,
    #[cfg(not(target_arch = "wasm32"))]
    shared: MicShared,
}

impl MicRecorder {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {}
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let shared = MicShared::new();
            let worker = shared.clone();
            std::thread::Builder::new()
                .name("perro_pawdio_mic".to_string())
                .spawn(move || mic_worker(rx, worker))
                .ok();
            Self { tx, shared }
        }
    }

    /// Input devices visible right now.
    pub fn devices(&self) -> Result<Vec<MicDevice>, String> {
        mic_devices()
    }

    pub fn is_listening(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.shared.listening.load(Ordering::Relaxed)
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    /// Name of the device backing the current or last capture.
    pub fn device(&self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let meta = self.shared.meta.lock().ok()?;
            meta.as_ref().map(|meta| meta.device.clone())
        }
    }

    /// Last capture error, including a device lost mid-stream.
    pub fn last_error(&self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let error = self.shared.error.lock().ok()?;
            error.clone()
        }
    }

    pub fn start(&mut self, settings: MicSettings) -> Result<(), String> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = settings;
            Err("mic unsupported on wasm".to_string())
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.is_listening() {
                return Ok(());
            }
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            self.tx
                .send(MicCommand::Start {
                    settings,
                    reply: reply_tx,
                })
                .map_err(|_| "mic worker stopped".to_string())?;
            reply_rx
                .recv()
                .map_err(|_| "mic worker no reply".to_string())?
        }
    }

    pub fn stop(&mut self) -> Option<MicClip> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Armed, not listening = device died mid-capture. Still drain the buffer.
            if !self.shared.armed.load(Ordering::Relaxed) {
                return None;
            }
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            let _ = self.tx.send(MicCommand::Stop { reply: reply_tx });
            let _ = reply_rx.recv();
            self.clip_from_state()
        }
    }

    pub fn clip(&self) -> Option<MicClip> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.clip_from_state()
        }
    }

    pub fn stream_clip(&self) -> Option<MicClip> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let meta = self.shared.meta.lock().ok()?.clone()?;
            let samples = self.shared.samples.lock().ok()?;
            let mut cursor = self.shared.cursor.lock().ok()?;
            let start = (*cursor).min(samples.len());
            if start == samples.len() {
                return None;
            }
            let chunk = samples[start..].to_vec();
            *cursor = samples.len();
            MicClip::try_new(chunk, meta.sample_rate, meta.channels).ok()
        }
    }

    pub fn stream_bytes(&self) -> Option<Vec<u8>> {
        self.stream_clip().map(|clip| clip.pack())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn clip_from_state(&self) -> Option<MicClip> {
        let meta = self.shared.meta.lock().ok()?.clone()?;
        let samples = self.shared.samples.lock().ok()?.clone();
        MicClip::try_new(samples, meta.sample_rate, meta.channels).ok()
    }
}

impl Default for MicRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn mic_worker(rx: std::sync::mpsc::Receiver<MicCommand>, shared: MicShared) {
    // Holds the cpal stream alive; never read back.
    let mut _active: Option<ActiveMic> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            MicCommand::Start { settings, reply } => {
                // Drop any prior stream first: a dead device leaves one armed.
                _active = None;
                shared.listening.store(false, Ordering::Relaxed);
                shared.set_error(None);
                let res = match start_stream(&settings, &shared) {
                    Ok((stream, meta)) => {
                        _active = Some(ActiveMic { _stream: stream });
                        if let Ok(mut slot) = shared.meta.lock() {
                            *slot = Some(meta);
                        }
                        shared.armed.store(true, Ordering::Relaxed);
                        shared.listening.store(true, Ordering::Relaxed);
                        Ok(())
                    }
                    Err(err) => {
                        shared.armed.store(false, Ordering::Relaxed);
                        shared.set_error(Some(err.clone()));
                        Err(err)
                    }
                };
                let _ = reply.send(res);
            }
            MicCommand::Stop { reply } => {
                _active = None;
                shared.listening.store(false, Ordering::Relaxed);
                shared.armed.store(false, Ordering::Relaxed);
                let _ = reply.send(());
            }
        }
    }
}

/// Rate the negotiation aims for when a device offers a range.
#[cfg(not(target_arch = "wasm32"))]
const MIC_TARGET_RATE: u32 = 48_000;

#[cfg(not(target_arch = "wasm32"))]
fn start_stream(
    settings: &MicSettings,
    shared: &MicShared,
) -> Result<(cpal::Stream, MicStreamMeta), String> {
    use cpal::traits::{DeviceTrait, StreamTrait};

    if let Ok(mut samples) = shared.samples.lock() {
        samples.clear();
    }
    if let Ok(mut cursor) = shared.cursor.lock() {
        *cursor = 0;
    }

    let host = cpal::default_host();
    let (device, device_name) = open_input_device(&host, settings.requested_device())?;
    let config = negotiate_input_config(&device)?;
    let sample_rate = config.sample_rate().0;
    let src_channels = config.channels().max(1);
    let out_channels = settings.channels.output_channels(src_channels);
    let max_samples = ((settings.max_seconds.max(0.1) * sample_rate as f32) as usize)
        .saturating_mul(out_channels as usize);
    let sink = MicSink::new(
        shared,
        max_samples,
        settings.denoise,
        src_channels,
        out_channels,
    );

    let err_listening = Arc::clone(&shared.listening);
    let err_slot = Arc::clone(&shared.error);
    let err_fn = move |err: cpal::StreamError| {
        if let Ok(mut slot) = err_slot.lock() {
            *slot = Some(format!("mic input stream err: {err}"));
        }
        // A yanked USB/wireless mic ends capture; flip listening so the game sees it.
        if matches!(err, cpal::StreamError::DeviceNotAvailable) {
            err_listening.store(false, Ordering::Relaxed);
        }
    };

    let stream_config = config.config();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let mut sink = sink;
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _| sink.push(data.iter().map(|sample| f32_to_i16(*sample))),
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let mut sink = sink;
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _| sink.push(data.iter().copied()),
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut sink = sink;
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _| sink.push(data.iter().map(|sample| u16_to_i16(*sample))),
                err_fn,
                None,
            )
        }
        other => return Err(format!("unsupported mic sample format: {other:?}")),
    }
    .map_err(|err| format!("mic input stream failed on `{device_name}`: {err}"))?;
    stream
        .play()
        .map_err(|err| format!("mic input play failed on `{device_name}`: {err}"))?;
    Ok((
        stream,
        MicStreamMeta {
            device: device_name,
            sample_rate,
            channels: out_channels,
        },
    ))
}

/// Open the named device, or the OS default when no name is set.
#[cfg(not(target_arch = "wasm32"))]
fn open_input_device(
    host: &cpal::Host,
    wanted: Option<&str>,
) -> Result<(cpal::Device, String), String> {
    use cpal::traits::{DeviceTrait, HostTrait};

    let Some(wanted) = wanted else {
        let device = host
            .default_input_device()
            .ok_or_else(|| "no default mic input device".to_string())?;
        let name = device
            .name()
            .unwrap_or_else(|_| "default input".to_string());
        return Ok((device, name));
    };

    let devices: Vec<cpal::Device> = host
        .input_devices()
        .map_err(|err| format!("mic device scan failed: {err}"))?
        .collect();
    let names: Vec<String> = devices
        .iter()
        .map(|device| device.name().unwrap_or_default())
        .collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let index = match_device_index(&refs, wanted).ok_or_else(|| {
        format!("mic device `{wanted}` not connected; rescan devices and pick another")
    })?;
    let name = names
        .get(index)
        .cloned()
        .unwrap_or_else(|| wanted.to_string());
    let device = devices
        .into_iter()
        .nth(index)
        .ok_or_else(|| format!("mic device `{wanted}` vanished during open"))?;
    Ok((device, name))
}

/// Pick a capture config the conversion path can handle.
///
/// The device default covers nearly every mic; the scan is the fallback for
/// devices that report no default or expose an exotic sample format.
#[cfg(not(target_arch = "wasm32"))]
fn negotiate_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, String> {
    use cpal::traits::DeviceTrait;

    if let Ok(config) = device.default_input_config()
        && config.channels() > 0
        && sample_format_rank(config.sample_format()).is_some()
    {
        return Ok(config);
    }
    let ranges = device
        .supported_input_configs()
        .map_err(|err| format!("mic input cfg failed: {err}"))?;
    pick_input_config(ranges)
        .ok_or_else(|| "mic device exposes no f32, i16, or u16 input format".to_string())
}

/// Preference order of the formats the capture path converts. Lower is better.
#[cfg(not(target_arch = "wasm32"))]
fn sample_format_rank(format: cpal::SampleFormat) -> Option<u8> {
    match format {
        cpal::SampleFormat::I16 => Some(0),
        cpal::SampleFormat::F32 => Some(1),
        cpal::SampleFormat::U16 => Some(2),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pick_input_config<I>(ranges: I) -> Option<cpal::SupportedStreamConfig>
where
    I: IntoIterator<Item = cpal::SupportedStreamConfigRange>,
{
    let mut best: Option<((u8, u32, u16), cpal::SupportedStreamConfig)> = None;
    for range in ranges {
        let Some(rank) = sample_format_rank(range.sample_format()) else {
            continue;
        };
        let channels = range.channels();
        if channels == 0 {
            continue;
        }
        let rate = clamp_rate(
            MIC_TARGET_RATE,
            range.min_sample_rate().0,
            range.max_sample_rate().0,
        );
        let Some(config) = range.try_with_sample_rate(cpal::SampleRate(rate)) else {
            continue;
        };
        let score = (
            rank,
            rate.abs_diff(MIC_TARGET_RATE),
            channels.saturating_sub(1),
        );
        if best.as_ref().is_none_or(|(current, _)| score < *current) {
            best = Some((score, config));
        }
    }
    best.map(|(_, config)| config)
}

#[cfg(not(target_arch = "wasm32"))]
fn clamp_rate(target: u32, min: u32, max: u32) -> u32 {
    if max < min {
        return min;
    }
    target.clamp(min, max)
}

#[cfg(not(target_arch = "wasm32"))]
fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

#[cfg(not(target_arch = "wasm32"))]
fn u16_to_i16(sample: u16) -> i16 {
    (sample as i32 - i16::MAX as i32 - 1).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Average one interleaved frame down to a single sample.
#[cfg(not(target_arch = "wasm32"))]
fn average_i16(frame: &[i16]) -> i16 {
    if frame.is_empty() {
        return 0;
    }
    let sum: i32 = frame.iter().map(|sample| *sample as i32).sum();
    (sum / frame.len() as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Audio-callback side of capture: convert, fold, denoise, append, trim.
#[cfg(not(target_arch = "wasm32"))]
struct MicSink {
    out: Arc<Mutex<Vec<i16>>>,
    cursor: Arc<Mutex<usize>>,
    max_samples: usize,
    denoise: MicDenoiseState,
    src_channels: usize,
    /// Partial frame carried over when a callback splits one.
    frame: Vec<i16>,
    fold: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl MicSink {
    fn new(
        shared: &MicShared,
        max_samples: usize,
        denoise: MicDenoiseSettings,
        src_channels: u16,
        out_channels: u16,
    ) -> Self {
        let src_channels = src_channels.max(1) as usize;
        Self {
            out: Arc::clone(&shared.samples),
            cursor: Arc::clone(&shared.cursor),
            max_samples,
            denoise: MicDenoiseState::new(denoise),
            src_channels,
            frame: Vec::with_capacity(src_channels),
            fold: (out_channels.max(1) as usize) < src_channels,
        }
    }

    fn push<I: IntoIterator<Item = i16>>(&mut self, data: I) {
        let Self {
            out,
            cursor,
            max_samples,
            denoise,
            src_channels,
            frame,
            fold,
        } = self;
        let Ok(mut samples) = out.lock() else {
            return;
        };
        if *fold {
            for sample in data {
                frame.push(sample);
                if frame.len() >= *src_channels {
                    let mono = average_i16(frame);
                    frame.clear();
                    samples.push(denoise.apply(mono));
                }
            }
        } else {
            samples.extend(data.into_iter().map(|sample| denoise.apply(sample)));
        }
        trim_samples(&mut samples, cursor, *max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn trim_samples(samples: &mut Vec<i16>, stream_cursor: &Mutex<usize>, max_samples: usize) {
    if max_samples == 0 || samples.len() <= max_samples {
        return;
    }
    let drain = samples.len() - max_samples;
    samples.drain(..drain);
    if let Ok(mut cursor) = stream_cursor.lock() {
        *cursor = cursor.saturating_sub(drain);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MicChannels, MicClip, MicDenoiseSettings, MicDevice, MicSettings, PMIC_CODEC_DELTA,
        PMIC_CODEC_PCM, PMIC_CODEC_ZLIB_DELTA, PMIC_CODEC_ZLIB_PCM, PMIC_COMPRESSED_HEADER_LEN,
        PMIC_MAGIC, PMIC_MAX_SAMPLES, PMIC_VERSION, PMIC_VERSION_COMPRESSED, match_device_index,
        resolve_mic_device,
    };

    fn device(name: &str, is_default: bool) -> MicDevice {
        MicDevice {
            name: name.to_string(),
            label: name.to_string(),
            is_default,
            sample_rate: 48_000,
            channels: 1,
        }
    }

    /// Builds a v2 header with caller-chosen (possibly hostile) size fields.
    fn v2_packet(
        channels: u16,
        sample_rate: u32,
        frames: u32,
        codec: u8,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(PMIC_COMPRESSED_HEADER_LEN + payload.len());
        out.extend_from_slice(PMIC_MAGIC);
        out.extend_from_slice(&PMIC_VERSION_COMPRESSED.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&frames.to_le_bytes());
        out.push(codec);
        out.extend_from_slice(&[0, 0, 0]);
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn mic_clip_pack_roundtrip() {
        let clip = MicClip::new(vec![-1, 0, 1, 32000], 48_000, 2);
        let packed = clip.pack();
        let unpacked = MicClip::unpack(&packed).expect("unpack mic clip");
        assert_eq!(unpacked, clip);
    }

    #[test]
    fn mic_clip_raw_bytes_v1_roundtrip() {
        let clip = MicClip::new(vec![100, -100, 200, -200], 48_000, 2);
        let packed = clip.raw_bytes();
        assert_eq!(u16::from_le_bytes([packed[4], packed[5]]), PMIC_VERSION);
        let unpacked = MicClip::unpack(&packed).expect("unpack mic clip");
        assert_eq!(unpacked, clip);
    }

    #[test]
    fn mic_clip_pack_uses_smaller_v2_when_possible() {
        let clip = MicClip::new(vec![0; 480], 48_000, 1);
        let packed = clip.pack();
        assert_eq!(
            u16::from_le_bytes([packed[4], packed[5]]),
            PMIC_VERSION_COMPRESSED
        );
        assert!(packed.len() < clip.raw_byte_len());
        let unpacked = MicClip::unpack(&packed).expect("unpack mic clip");
        assert_eq!(unpacked, clip);
    }

    #[test]
    fn mic_clip_wav_has_riff_header() {
        let clip = MicClip::new(vec![0, 1], 44_100, 1);
        let wav = clip.wav_bytes();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[test]
    fn mic_clip_denoise_reduces_quiet_samples() {
        let clip = MicClip::new(vec![200, 20_000], 48_000, 1);
        let denoised = clip.denoised(MicDenoiseSettings {
            enabled: true,
            noise_floor: 0.02,
            reduction: 0.9,
            high_pass: false,
        });
        assert!(denoised.samples[0].abs() < clip.samples[0].abs());
        assert!(denoised.samples[1].abs() > 10_000);
    }

    #[test]
    fn mic_clip_rejects_invalid_format_invariants() {
        assert!(MicClip::try_new(vec![0], 0, 1).is_err());
        assert!(MicClip::try_new(vec![0], 48_000, 0).is_err());
        assert!(MicClip::try_new(vec![0, 1, 2], 48_000, 2).is_err());
        assert!(MicClip::try_new(vec![], u32::MAX, u16::MAX).is_err());
    }

    #[test]
    fn mic_clip_unpack_rejects_zero_format_fields() {
        let clip = MicClip::new(vec![0, 1], 48_000, 1);
        let mut packed = clip.raw_bytes();
        packed[6..8].copy_from_slice(&0u16.to_le_bytes());
        assert!(MicClip::unpack(&packed).is_err());

        let mut packed = clip.raw_bytes();
        packed[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(MicClip::unpack(&packed).is_err());
    }

    /// Header sizes are attacker controlled. A tiny packet declaring a huge sample
    /// count must return an error, never reach an allocation that aborts the process.
    #[test]
    fn mic_clip_unpack_rejects_oversized_header_sizes() {
        for codec in [
            PMIC_CODEC_PCM,
            PMIC_CODEC_ZLIB_PCM,
            PMIC_CODEC_DELTA,
            PMIC_CODEC_ZLIB_DELTA,
        ] {
            let packet = v2_packet(u16::MAX, 48_000, u32::MAX, codec, &[]);
            assert_eq!(packet.len(), PMIC_COMPRESSED_HEADER_LEN);
            assert!(
                MicClip::unpack(&packet).is_err(),
                "codec {codec} accepted oversized header"
            );
        }
    }

    #[test]
    fn mic_clip_unpack_v1_rejects_oversized_frame_count() {
        let clip = MicClip::new(vec![0, 1], 48_000, 1);
        let mut packed = clip.raw_bytes();
        packed[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(MicClip::unpack(&packed).is_err());
    }

    /// Under the sample ceiling, but still more samples than the payload can encode.
    #[test]
    fn mic_clip_unpack_rejects_delta_frames_beyond_payload() {
        let packet = v2_packet(1, 48_000, 4096, PMIC_CODEC_DELTA, &[0; 8]);
        assert!(MicClip::unpack(&packet).is_err());
    }

    #[test]
    fn mic_clip_unpack_rejects_unknown_codec() {
        let packet = v2_packet(1, 48_000, 0, 9, &[]);
        assert!(MicClip::unpack(&packet).is_err());
    }

    #[test]
    fn mic_clip_unpack_accepts_valid_uncompressed_codecs() {
        let samples = vec![-32_768, -1, 0, 1, 32_767, 900];
        let clip = MicClip::new(samples.clone(), 48_000, 2);
        let frames = (samples.len() / 2) as u32;

        let pcm = v2_packet(
            2,
            48_000,
            frames,
            PMIC_CODEC_PCM,
            &super::pcm_payload(&samples),
        );
        assert_eq!(MicClip::unpack(&pcm).expect("unpack pcm codec"), clip);

        let delta = v2_packet(
            2,
            48_000,
            frames,
            PMIC_CODEC_DELTA,
            &super::delta_payload(&samples, 2),
        );
        assert_eq!(MicClip::unpack(&delta).expect("unpack delta codec"), clip);
    }

    /// The ceiling must stay well clear of any real capture length.
    #[test]
    fn mic_clip_sample_ceiling_covers_long_captures() {
        let ten_min_48k_stereo = 48_000 * 2 * 60 * 10;
        assert!(PMIC_MAX_SAMPLES > ten_min_48k_stereo);
    }

    #[test]
    fn mic_settings_default_targets_os_default_device() {
        let settings = MicSettings::default();
        assert_eq!(settings.device, None);
        assert_eq!(settings.requested_device(), None);
        assert_eq!(settings.channels, MicChannels::Auto);
    }

    #[test]
    fn mic_settings_blank_device_falls_back_to_os_default() {
        assert_eq!(
            MicSettings::default().with_device("").requested_device(),
            None
        );
        assert_eq!(
            MicSettings::default().with_device("   ").requested_device(),
            None
        );
        assert_eq!(
            MicSettings::default()
                .with_device("  USB Mic  ")
                .requested_device(),
            Some("USB Mic")
        );
    }

    #[test]
    fn mic_settings_keep_other_fields_when_device_set() {
        let settings = MicSettings::default()
            .with_max_seconds(8.0)
            .with_denoise(MicDenoiseSettings::voice())
            .with_channels(MicChannels::Mono)
            .with_device("Yeti Nano");
        assert_eq!(settings.requested_device(), Some("Yeti Nano"));
        assert_eq!(settings.max_seconds, 8.0);
        assert!(settings.denoise.enabled);
        assert_eq!(settings.channels, MicChannels::Mono);
        assert_eq!(
            settings.with_default_device().requested_device(),
            None,
            "clearing the device returns to the OS default"
        );
    }

    #[test]
    fn mic_device_settings_target_that_device() {
        let entry = device("Headset (Wireless)", false);
        assert_eq!(
            entry.settings().requested_device(),
            Some("Headset (Wireless)")
        );
    }

    #[test]
    fn mic_channels_pick_clip_layout() {
        for channels in [1u16, 2] {
            assert_eq!(MicChannels::Auto.output_channels(channels), channels);
        }
        assert_eq!(MicChannels::Auto.output_channels(4), 1);
        assert_eq!(MicChannels::Auto.output_channels(8), 1);
        assert_eq!(MicChannels::Auto.output_channels(0), 1);
        assert_eq!(MicChannels::Mono.output_channels(8), 1);
        assert_eq!(MicChannels::Device.output_channels(8), 8);
        assert_eq!(MicChannels::Device.output_channels(0), 1);
    }

    /// Scans reorder between calls, so a cached name must still hit its device.
    #[test]
    fn mic_device_match_keys_off_name_not_position() {
        let first = ["Built-in Mic", "USB Mic", "Virtual Cable"];
        let second = ["Virtual Cable", "Built-in Mic", "USB Mic"];
        assert_eq!(match_device_index(&first, "USB Mic"), Some(1));
        assert_eq!(match_device_index(&second, "USB Mic"), Some(2));
    }

    #[test]
    fn mic_device_match_accepts_case_and_padding_drift() {
        let names = ["USB Mic"];
        assert_eq!(match_device_index(&names, "usb mic"), Some(0));
        assert_eq!(match_device_index(&names, " USB Mic "), Some(0));
        assert_eq!(match_device_index(&names, "Other Mic"), None);
        assert_eq!(match_device_index(&names, "  "), None);
    }

    /// Exact match wins even when a case-insensitive twin sits earlier.
    #[test]
    fn mic_device_match_prefers_exact_name() {
        let names = ["usb mic", "USB Mic"];
        assert_eq!(match_device_index(&names, "USB Mic"), Some(1));
    }

    #[test]
    fn resolve_mic_device_uses_cached_name() {
        let devices = [
            device("Built-in Mic", true),
            device("USB Mic", false),
            device("Virtual Cable", false),
        ];
        let picked = resolve_mic_device(&devices, Some("USB Mic")).expect("cached device");
        assert_eq!(picked.name, "USB Mic");
    }

    #[test]
    fn resolve_mic_device_falls_back_when_cached_device_is_gone() {
        let devices = [device("Built-in Mic", true), device("Virtual Cable", false)];
        let picked = resolve_mic_device(&devices, Some("USB Mic")).expect("default device");
        assert_eq!(picked.name, "Built-in Mic");

        let no_default = [device("Virtual Cable", false)];
        let picked = resolve_mic_device(&no_default, Some("USB Mic")).expect("first device");
        assert_eq!(picked.name, "Virtual Cable");

        assert!(resolve_mic_device(&[], Some("USB Mic")).is_none());
        assert!(resolve_mic_device(&[], None).is_none());
    }
}

/// Capture-path tests. Native only: the sink and cpal helpers are gated off wasm.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod capture_tests {
    use super::{
        MicChannels, MicDenoiseSettings, MicShared, MicSink, average_i16, clamp_rate,
        dedupe_labels, f32_to_i16, pick_input_config, u16_to_i16,
    };

    /// Sink writing into a fresh buffer, standing in for the audio callback.
    fn sink(src_channels: u16, max_samples: usize, mode: MicChannels) -> (MicSink, MicShared) {
        let shared = MicShared::new();
        let sink = MicSink::new(
            &shared,
            max_samples,
            MicDenoiseSettings::off(),
            src_channels,
            mode.output_channels(src_channels),
        );
        (sink, shared)
    }

    fn captured(shared: &MicShared) -> Vec<i16> {
        shared.samples.lock().expect("lock mic samples").clone()
    }

    fn config_range(
        channels: u16,
        min_rate: u32,
        max_rate: u32,
        format: cpal::SampleFormat,
    ) -> cpal::SupportedStreamConfigRange {
        cpal::SupportedStreamConfigRange::new(
            channels,
            cpal::SampleRate(min_rate),
            cpal::SampleRate(max_rate),
            cpal::SupportedBufferSize::Unknown,
            format,
        )
    }

    #[test]
    fn f32_input_converts_and_clamps() {
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), i16::MAX);
        assert_eq!(f32_to_i16(-1.0), -i16::MAX);
        assert_eq!(f32_to_i16(4.0), i16::MAX);
        assert_eq!(f32_to_i16(-4.0), -i16::MAX);
        assert_eq!(f32_to_i16(0.5), 16_383);
    }

    #[test]
    fn u16_input_recenters_to_signed() {
        assert_eq!(u16_to_i16(0), i16::MIN);
        assert_eq!(u16_to_i16(32_768), 0);
        assert_eq!(u16_to_i16(u16::MAX), i16::MAX);
    }

    #[test]
    fn sink_writes_f32_stream_as_i16() {
        let (mut sink, shared) = sink(1, 0, MicChannels::Auto);
        sink.push([0.0f32, 1.0, -1.0, 0.5].iter().map(|s| f32_to_i16(*s)));
        assert_eq!(captured(&shared), vec![0, i16::MAX, -i16::MAX, 16_383]);
    }

    #[test]
    fn sink_writes_u16_stream_as_i16() {
        let (mut sink, shared) = sink(1, 0, MicChannels::Auto);
        sink.push([0u16, 32_768, u16::MAX].iter().map(|s| u16_to_i16(*s)));
        assert_eq!(captured(&shared), vec![i16::MIN, 0, i16::MAX]);
    }

    #[test]
    fn sink_keeps_stereo_devices_interleaved() {
        let (mut sink, shared) = sink(2, 0, MicChannels::Auto);
        sink.push([100i16, -100, 200, -200]);
        assert_eq!(captured(&shared), vec![100, -100, 200, -200]);
    }

    #[test]
    fn sink_folds_wide_interfaces_to_mono() {
        let (mut sink, shared) = sink(4, 0, MicChannels::Auto);
        sink.push([100i16, 200, 300, 400, 0, 0, 0, 40]);
        assert_eq!(captured(&shared), vec![250, 10]);
    }

    #[test]
    fn sink_folds_stereo_when_mono_is_forced() {
        let (mut sink, shared) = sink(2, 0, MicChannels::Mono);
        sink.push([100i16, 300]);
        assert_eq!(captured(&shared), vec![200]);
    }

    #[test]
    fn sink_keeps_wide_layout_in_device_mode() {
        let (mut sink, shared) = sink(4, 0, MicChannels::Device);
        sink.push([1i16, 2, 3, 4]);
        assert_eq!(captured(&shared), vec![1, 2, 3, 4]);
    }

    /// A callback may cut a frame in half; the tail must join the next one.
    #[test]
    fn sink_folds_frames_split_across_callbacks() {
        let (mut sink, shared) = sink(4, 0, MicChannels::Auto);
        sink.push([100i16, 200]);
        assert!(captured(&shared).is_empty());
        sink.push([300i16, 400]);
        assert_eq!(captured(&shared), vec![250]);
    }

    #[test]
    fn sink_trims_to_max_samples_and_rewinds_cursor() {
        let (mut sink, shared) = sink(1, 4, MicChannels::Auto);
        sink.push([1i16, 2, 3, 4]);
        if let Ok(mut cursor) = shared.cursor.lock() {
            *cursor = 4;
        }
        sink.push([5i16, 6]);
        assert_eq!(captured(&shared), vec![3, 4, 5, 6]);
        assert_eq!(*shared.cursor.lock().expect("lock cursor"), 2);
    }

    #[test]
    fn sink_applies_denoise_after_the_fold() {
        let shared = MicShared::new();
        let mut sink = MicSink::new(&shared, 0, MicDenoiseSettings::voice(), 4, 1);
        sink.push([120i16; 8]);
        let samples = captured(&shared);
        assert_eq!(samples.len(), 2, "4ch frames fold to one sample each");
        assert!(
            samples.iter().all(|sample| sample.abs() < 120),
            "quiet folded frames get gated: {samples:?}"
        );
    }

    #[test]
    fn frame_average_clamps_and_handles_empty() {
        assert_eq!(average_i16(&[]), 0);
        assert_eq!(average_i16(&[100, 200]), 150);
        assert_eq!(average_i16(&[i16::MIN, i16::MIN]), i16::MIN);
        assert_eq!(average_i16(&[i16::MAX, i16::MAX]), i16::MAX);
    }

    #[test]
    fn labels_disambiguate_twin_devices() {
        let names = [
            "USB Mic".to_string(),
            "Built-in".to_string(),
            "USB Mic".to_string(),
            "USB Mic".to_string(),
        ];
        assert_eq!(
            dedupe_labels(&names),
            vec!["USB Mic", "Built-in", "USB Mic #2", "USB Mic #3"]
        );
    }

    #[test]
    fn config_pick_skips_formats_the_capture_path_cannot_convert() {
        let ranges = [
            config_range(2, 44_100, 44_100, cpal::SampleFormat::I32),
            config_range(2, 8_000, 96_000, cpal::SampleFormat::F32),
            config_range(1, 8_000, 96_000, cpal::SampleFormat::I16),
        ];
        let picked = pick_input_config(ranges).expect("convertible config");
        assert_eq!(picked.sample_format(), cpal::SampleFormat::I16);
        assert_eq!(picked.sample_rate().0, 48_000);
        assert_eq!(picked.channels(), 1);
    }

    /// Devices capped below the target rate still open at their own rate.
    #[test]
    fn config_pick_clamps_rate_into_device_range() {
        let ranges = [config_range(1, 16_000, 16_000, cpal::SampleFormat::F32)];
        let picked = pick_input_config(ranges).expect("convertible config");
        assert_eq!(picked.sample_rate().0, 16_000);
    }

    #[test]
    fn config_pick_rejects_devices_without_a_usable_format() {
        let ranges = [
            config_range(2, 44_100, 44_100, cpal::SampleFormat::I32),
            config_range(2, 44_100, 44_100, cpal::SampleFormat::F64),
            config_range(0, 8_000, 96_000, cpal::SampleFormat::I16),
        ];
        assert!(pick_input_config(ranges).is_none());
    }

    #[test]
    fn rate_clamp_survives_backwards_ranges() {
        assert_eq!(clamp_rate(48_000, 8_000, 96_000), 48_000);
        assert_eq!(clamp_rate(48_000, 8_000, 16_000), 16_000);
        assert_eq!(clamp_rate(48_000, 96_000, 192_000), 96_000);
        assert_eq!(clamp_rate(48_000, 44_100, 8_000), 44_100);
    }

    /// Needs a real host with audio devices.
    #[test]
    #[ignore = "requires audio hardware"]
    fn device_scan_lists_hardware() {
        let devices = super::super::mic_devices().expect("scan input devices");
        assert!(!devices.is_empty());
        assert!(devices.iter().filter(|device| device.is_default).count() <= 1);
    }

    /// Needs a real host; a missing device must error rather than panic.
    #[test]
    #[ignore = "requires audio hardware"]
    fn start_on_missing_device_errors() {
        let mut recorder = super::super::MicRecorder::new();
        let err = recorder
            .start(super::super::MicSettings::default().with_device("perro-no-such-device"))
            .expect_err("unknown device must fail");
        assert!(err.contains("perro-no-such-device"), "{err}");
        assert!(!recorder.is_listening());
        assert!(recorder.stop().is_none());
    }
}
