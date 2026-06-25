use std::sync::{Arc, Mutex};
use std::time::Duration;

const PMIC_MAGIC: &[u8; 4] = b"PMIC";
const PMIC_VERSION: u16 = 1;
const PMIC_HEADER_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct MicClip {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl MicClip {
    pub fn new(samples: Vec<i16>, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
        }
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
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != PMIC_VERSION {
            return Err(format!("unsupported mic clip version {version}"));
        }
        let channels = u16::from_le_bytes([bytes[6], bytes[7]]).max(1);
        let sample_rate = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]).max(1);
        let frames = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        let payload = &bytes[PMIC_HEADER_LEN..];
        if !payload.len().is_multiple_of(2) {
            return Err("mic clip odd payload len".to_string());
        }
        let expected_samples = frames
            .checked_mul(channels as usize)
            .ok_or_else(|| "mic clip sample len overflow".to_string())?;
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
        Ok(Self {
            samples,
            sample_rate,
            channels,
        })
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
}

#[derive(Clone, Copy, Debug)]
pub struct MicSettings {
    pub max_seconds: f32,
}

impl Default for MicSettings {
    fn default() -> Self {
        Self { max_seconds: 30.0 }
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
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _| push_f32(data, &out, &cursor, max_samples),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _| push_i16(data, &out, &cursor, max_samples),
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _| push_u16(data, &out, &cursor, max_samples),
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
) {
    if let Ok(mut samples) = out.lock() {
        samples.extend_from_slice(data);
        trim_samples(&mut samples, stream_cursor, max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_f32(
    data: &[f32],
    out: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
    max_samples: usize,
) {
    if let Ok(mut samples) = out.lock() {
        samples.extend(
            data.iter()
                .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
        );
        trim_samples(&mut samples, stream_cursor, max_samples);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn push_u16(
    data: &[u16],
    out: &Arc<Mutex<Vec<i16>>>,
    stream_cursor: &Arc<Mutex<usize>>,
    max_samples: usize,
) {
    if let Ok(mut samples) = out.lock() {
        samples.extend(data.iter().map(|sample| {
            let centered = *sample as i32 - i16::MAX as i32 - 1;
            centered.clamp(i16::MIN as i32, i16::MAX as i32) as i16
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
    use super::MicClip;

    #[test]
    fn mic_clip_pack_roundtrip() {
        let clip = MicClip::new(vec![-1, 0, 1, 32000], 48_000, 2);
        let packed = clip.pack();
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
}
