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
                let decoded = perro_io::decompress_zlib_limited(payload, expected_samples * 2)
                    .map_err(|err| format!("mic clip zlib decode failed: {err}"))?;
                decode_pcm_payload(&decoded, frames, channels)?
            }
            PMIC_CODEC_DELTA => decode_delta_payload(payload, expected_samples, channels)?,
            PMIC_CODEC_ZLIB_DELTA => {
                let decoded = perro_io::decompress_zlib_limited(payload, expected_samples * 3)
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
    frames
        .checked_mul(channels as usize)
        .ok_or_else(|| "mic clip sample len overflow".to_string())
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

#[derive(Clone, Copy, Debug)]
pub struct MicSettings {
    pub max_seconds: f32,
    pub denoise: MicDenoiseSettings,
}

impl Default for MicSettings {
    fn default() -> Self {
        Self {
            max_seconds: 30.0,
            denoise: MicDenoiseSettings::off(),
        }
    }
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
        reply: std::sync::mpsc::Sender<Result<(u32, u16), String>>,
    },
    Stop {
        reply: std::sync::mpsc::Sender<()>,
    },
}

pub struct MicRecorder {
    #[cfg(not(target_arch = "wasm32"))]
    tx: std::sync::mpsc::Sender<MicCommand>,
    #[cfg(not(target_arch = "wasm32"))]
    samples: Arc<Mutex<Vec<i16>>>,
    #[cfg(not(target_arch = "wasm32"))]
    stream_cursor: Arc<Mutex<usize>>,
    #[cfg(not(target_arch = "wasm32"))]
    meta: Arc<Mutex<Option<(u32, u16)>>>,
    #[cfg(not(target_arch = "wasm32"))]
    listening: Arc<std::sync::atomic::AtomicBool>,
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
            let samples = Arc::new(Mutex::new(Vec::<i16>::new()));
            let worker_samples = Arc::clone(&samples);
            let stream_cursor = Arc::new(Mutex::new(0usize));
            let worker_stream_cursor = Arc::clone(&stream_cursor);
            let meta = Arc::new(Mutex::new(None));
            let worker_meta = Arc::clone(&meta);
            let listening = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_listening = Arc::clone(&listening);
            std::thread::Builder::new()
                .name("perro_pawdio_mic".to_string())
                .spawn(move || {
                    mic_worker(
                        rx,
                        worker_samples,
                        worker_stream_cursor,
                        worker_meta,
                        worker_listening,
                    )
                })
                .ok();
            Self {
                tx,
                samples,
                stream_cursor,
                meta,
                listening,
            }
        }
    }

    pub fn is_listening(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.listening.load(std::sync::atomic::Ordering::Relaxed)
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
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
                .map_err(|_| "mic worker no reply".to_string())??;
            Ok(())
        }
    }

    pub fn stop(&mut self) -> Option<MicClip> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.is_listening() {
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
            let (sample_rate, channels) = self.meta.lock().ok()?.as_ref().copied()?;
            let samples = self.samples.lock().ok()?;
            let mut cursor = self.stream_cursor.lock().ok()?;
            let start = (*cursor).min(samples.len());
            if start == samples.len() {
                return None;
            }
            let chunk = samples[start..].to_vec();
            *cursor = samples.len();
            Some(MicClip::new(chunk, sample_rate, channels))
        }
    }

    pub fn stream_bytes(&self) -> Option<Vec<u8>> {
        self.stream_clip().map(|clip| clip.pack())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn clip_from_state(&self) -> Option<MicClip> {
        let (sample_rate, channels) = self.meta.lock().ok()?.as_ref().copied()?;
        let samples = self.samples.lock().ok()?.clone();
        Some(MicClip::new(samples, sample_rate, channels))
    }
}

impl Default for MicRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn mic_worker(
    rx: std::sync::mpsc::Receiver<MicCommand>,
    samples: Arc<Mutex<Vec<i16>>>,
    stream_cursor: Arc<Mutex<usize>>,
    meta: Arc<Mutex<Option<(u32, u16)>>>,
    listening: Arc<std::sync::atomic::AtomicBool>,
) {
    let mut active: Option<ActiveMic> = None;
    while let Ok(cmd) = rx.recv() {
        match cmd {
            MicCommand::Start { settings, reply } => {
                let res = start_stream(settings, &samples, &stream_cursor).map(
                    |(stream, sample_rate, channels)| {
                        active = Some(ActiveMic { _stream: stream });
                        if let Ok(mut meta) = meta.lock() {
                            *meta = Some((sample_rate, channels));
                        }
                        listening.store(true, std::sync::atomic::Ordering::Relaxed);
                        (sample_rate, channels)
                    },
                );
                let _ = reply.send(res);
            }
            MicCommand::Stop { reply } => {
                active = None;
                listening.store(false, std::sync::atomic::Ordering::Relaxed);
                let _ = reply.send(());
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn start_stream(
    settings: MicSettings,
    samples: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
) -> Result<(cpal::Stream, u32, u16), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    if let Ok(mut samples) = samples.lock() {
        samples.clear();
    }
    if let Ok(mut cursor) = stream_cursor.lock() {
        *cursor = 0;
    }
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default mic input device".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|err| format!("mic input cfg failed: {err}"))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels().max(1);
    let max_samples = ((settings.max_seconds.max(0.1) * sample_rate as f32) as usize)
        .saturating_mul(channels as usize);
    let out = Arc::clone(samples);
    let cursor = Arc::clone(stream_cursor);
    let err_fn = |err| eprintln!("mic input stream err: {err}");
    let stream_config = config.config();
    let denoise = settings.denoise;
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            {
                let mut state = MicDenoiseState::new(denoise);
                move |data: &[f32], _| push_f32(data, &out, &cursor, max_samples, &mut state)
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            {
                let mut state = MicDenoiseState::new(denoise);
                move |data: &[i16], _| push_i16(data, &out, &cursor, max_samples, &mut state)
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            {
                let mut state = MicDenoiseState::new(denoise);
                move |data: &[u16], _| push_u16(data, &out, &cursor, max_samples, &mut state)
            },
            err_fn,
            None,
        ),
        other => return Err(format!("unsupported mic sample format: {other:?}")),
    }
    .map_err(|err| format!("mic input stream failed: {err}"))?;
    stream
        .play()
        .map_err(|err| format!("mic input play failed: {err}"))?;
    Ok((stream, sample_rate, channels))
}

#[cfg(not(target_arch = "wasm32"))]
fn push_i16(
    data: &[i16],
    out: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
    max_samples: usize,
    denoise: &mut MicDenoiseState,
) {
    if let Ok(mut samples) = out.lock() {
        if denoise.settings.enabled {
            samples.extend(data.iter().map(|sample| denoise.process_i16(*sample)));
        } else {
            samples.extend_from_slice(data);
        }
        trim_samples(&mut samples, stream_cursor, max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_f32(
    data: &[f32],
    out: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
    max_samples: usize,
    denoise: &mut MicDenoiseState,
) {
    if let Ok(mut samples) = out.lock() {
        samples.extend(data.iter().map(|sample| {
            let sample = denoise.process_f32(*sample);
            (sample * i16::MAX as f32) as i16
        }));
        trim_samples(&mut samples, stream_cursor, max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_u16(
    data: &[u16],
    out: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
    max_samples: usize,
    denoise: &mut MicDenoiseState,
) {
    if let Ok(mut samples) = out.lock() {
        samples.extend(data.iter().map(|sample| {
            let centered = *sample as i32 - i16::MAX as i32 - 1;
            denoise.process_i16(centered.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        }));
        trim_samples(&mut samples, stream_cursor, max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn trim_samples(samples: &mut Vec<i16>, stream_cursor: &Arc<Mutex<usize>>, max_samples: usize) {
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
    use super::{MicClip, MicDenoiseSettings, PMIC_VERSION, PMIC_VERSION_COMPRESSED};

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
}
