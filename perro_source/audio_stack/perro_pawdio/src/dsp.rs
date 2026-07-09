use crate::types::{AudioCompression, AudioEq, SpatialAudioParams};
use rodio::Source;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct DspControl {
    low_pass: AtomicU32,
    reverb_send: AtomicU32,
    echo: AtomicU32,
    reflection: AtomicU32,
    occlusion: AtomicU32,
    eq_low_gain: AtomicU32,
    eq_mid_gain: AtomicU32,
    eq_high_gain: AtomicU32,
    compression_threshold: AtomicU32,
    compression_ratio: AtomicU32,
}

impl DspControl {
    pub(crate) fn new(params: DspParams) -> Arc<Self> {
        Arc::new(Self {
            low_pass: AtomicU32::new(params.low_pass.to_bits()),
            reverb_send: AtomicU32::new(params.reverb_send.to_bits()),
            echo: AtomicU32::new(params.echo.to_bits()),
            reflection: AtomicU32::new(params.reflection.to_bits()),
            occlusion: AtomicU32::new(params.occlusion.to_bits()),
            eq_low_gain: AtomicU32::new(params.eq.low_gain.to_bits()),
            eq_mid_gain: AtomicU32::new(params.eq.mid_gain.to_bits()),
            eq_high_gain: AtomicU32::new(params.eq.high_gain.to_bits()),
            compression_threshold: AtomicU32::new(params.compression.threshold.to_bits()),
            compression_ratio: AtomicU32::new(params.compression.ratio.to_bits()),
        })
    }

    pub(crate) fn update_spatial(&self, params: SpatialAudioParams) {
        self.low_pass
            .store(params.low_pass.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        self.reverb_send.store(
            params.reverb_send.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
        self.echo
            .store(params.echo.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        self.reflection.store(
            params.reflection.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
        self.occlusion.store(
            params.occlusion.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
        self.eq_low_gain
            .store(params.eq.low_gain.max(0.0).to_bits(), Ordering::Relaxed);
        self.eq_mid_gain
            .store(params.eq.mid_gain.max(0.0).to_bits(), Ordering::Relaxed);
        self.eq_high_gain
            .store(params.eq.high_gain.max(0.0).to_bits(), Ordering::Relaxed);
        self.compression_threshold.store(
            params.compression.threshold.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
        self.compression_ratio.store(
            params.compression.ratio.max(1.0).to_bits(),
            Ordering::Relaxed,
        );
    }

    fn snapshot(&self) -> DspParams {
        DspParams {
            low_pass: load_f32(&self.low_pass).clamp(0.0, 1.0),
            reverb_send: load_f32(&self.reverb_send).clamp(0.0, 1.0),
            echo: load_f32(&self.echo).clamp(0.0, 1.0),
            reflection: load_f32(&self.reflection).clamp(0.0, 1.0),
            occlusion: load_f32(&self.occlusion).clamp(0.0, 1.0),
            eq: AudioEq {
                low_gain: load_f32(&self.eq_low_gain).max(0.0),
                mid_gain: load_f32(&self.eq_mid_gain).max(0.0),
                high_gain: load_f32(&self.eq_high_gain).max(0.0),
            },
            compression: AudioCompression {
                threshold: load_f32(&self.compression_threshold).clamp(0.0, 1.0),
                ratio: load_f32(&self.compression_ratio).max(1.0),
                attack: 0.01,
                release: 0.1,
            },
        }
    }
}

fn load_f32(value: &AtomicU32) -> f32 {
    f32::from_bits(value.load(Ordering::Relaxed))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DspParams {
    pub(crate) low_pass: f32,
    pub(crate) reverb_send: f32,
    pub(crate) echo: f32,
    pub(crate) reflection: f32,
    pub(crate) occlusion: f32,
    pub(crate) eq: AudioEq,
    pub(crate) compression: AudioCompression,
}

impl DspParams {
    pub(crate) fn dry() -> Self {
        Self {
            low_pass: 0.0,
            reverb_send: 0.0,
            echo: 0.0,
            reflection: 0.0,
            occlusion: 0.0,
            eq: AudioEq::default(),
            compression: AudioCompression::default(),
        }
    }
}

impl From<SpatialAudioParams> for DspParams {
    fn from(value: SpatialAudioParams) -> Self {
        Self {
            low_pass: value.low_pass,
            reverb_send: value.reverb_send,
            echo: value.echo,
            reflection: value.reflection,
            occlusion: value.occlusion,
            eq: value.eq,
            compression: value.compression,
        }
    }
}

// Refresh the atomic param snapshot only every N samples: snapshotting per
// sample costs 10 relaxed loads + clamps (~1M loads/s for a 48kHz stereo
// source). At 48kHz, 64 samples is sub-2ms, inaudibly coarse for spatial ramps.
const PARAM_REFRESH_SAMPLES: u32 = 64;

pub(crate) struct DspSource<S> {
    input: S,
    control: Arc<DspControl>,
    channels: usize,
    channel_index: usize,
    params: DspParams,
    param_counter: u32,
    // Delay lines run only while wet; on a dry->wet transition their buffers
    // hold stale audio, so track wet state to clear them at the edge.
    wet_active: bool,
    low_state: Vec<f32>,
    eq_low_state: Vec<f32>,
    eq_high_prev_in: Vec<f32>,
    eq_high_prev_out: Vec<f32>,
    echo: DelayLine,
    reverb_a: DelayLine,
    reverb_b: DelayLine,
}

impl<S> DspSource<S>
where
    S: Source<Item = f32>,
{
    pub(crate) fn new(input: S, control: Arc<DspControl>) -> Self {
        let channels = input.channels().max(1) as usize;
        let sample_rate = input.sample_rate().max(1) as usize;
        Self {
            input,
            control,
            channels,
            channel_index: 0,
            params: DspParams::dry(),
            param_counter: 0,
            wet_active: false,
            low_state: vec![0.0; channels],
            eq_low_state: vec![0.0; channels],
            eq_high_prev_in: vec![0.0; channels],
            eq_high_prev_out: vec![0.0; channels],
            echo: DelayLine::new(delay_len(sample_rate, channels, 0.18), 0.32),
            reverb_a: DelayLine::new(delay_len(sample_rate, channels, 0.047), 0.62),
            reverb_b: DelayLine::new(delay_len(sample_rate, channels, 0.071), 0.54),
        }
    }
}

impl<S> Iterator for DspSource<S>
where
    S: Source<Item = f32>,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let input = self.input.next()?;
        // First call (counter 0) always snapshots; refresh every N samples.
        if self.param_counter == 0 {
            self.params = self.control.snapshot();
        }
        self.param_counter += 1;
        if self.param_counter >= PARAM_REFRESH_SAMPLES {
            self.param_counter = 0;
        }
        let params = self.params;
        let ch = self.channel_index.min(self.channels - 1);
        let mut sample = input;

        // Unity EQ is an identity passthrough; skip the two filter updates.
        if !eq_is_dry(params.eq) {
            sample = self.apply_eq(ch, sample, params.eq);
        }
        sample = self.apply_low_pass(ch, sample, params.low_pass.max(params.occlusion * 0.8));

        let echo_wet = params.echo.max(params.reflection * 0.35).clamp(0.0, 1.0);
        let reverb_wet = params
            .reverb_send
            .max(params.reflection * 0.25)
            .clamp(0.0, 1.0);
        // All-dry: the wet mix collapses to a passthrough, so skip the three
        // delay-line read/writes entirely.
        if echo_wet > 0.001 || reverb_wet > 0.001 {
            if !self.wet_active {
                // Dry->wet edge: buffers hold stale audio from the skipped dry
                // stretch; clear so re-engaging does not burst old signal.
                self.echo.clear();
                self.reverb_a.clear();
                self.reverb_b.clear();
                self.wet_active = true;
            }
            let echo_sample = self.echo.process(ch, sample, echo_wet);
            let reverb_sample = (self.reverb_a.process(ch, sample, reverb_wet)
                + self.reverb_b.process(ch, sample, reverb_wet))
                * 0.5;
            sample = sample * (1.0 - (echo_wet + reverb_wet * 0.5).min(0.45))
                + echo_sample * echo_wet * 0.45
                + reverb_sample * reverb_wet * 0.35;
        } else {
            self.wet_active = false;
        }

        sample = apply_compression(sample, params.compression);
        sample *= 1.0 - params.occlusion.clamp(0.0, 1.0) * 0.45;

        self.channel_index += 1;
        if self.channel_index >= self.channels {
            self.channel_index = 0;
        }

        Some(sample.clamp(-1.0, 1.0))
    }
}

impl<S> Source for DspSource<S>
where
    S: Source<Item = f32>,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.input.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }
}

impl<S> DspSource<S>
where
    S: Source<Item = f32>,
{
    fn apply_low_pass(&mut self, ch: usize, sample: f32, amount: f32) -> f32 {
        if amount <= 0.001 {
            self.low_state[ch] = sample;
            return sample;
        }
        let alpha = (1.0 - amount.clamp(0.0, 0.98)).powi(2).max(0.015);
        self.low_state[ch] += (sample - self.low_state[ch]) * alpha;
        self.low_state[ch]
    }

    fn apply_eq(&mut self, ch: usize, sample: f32, eq: AudioEq) -> f32 {
        let low_alpha = 0.08;
        self.eq_low_state[ch] += (sample - self.eq_low_state[ch]) * low_alpha;
        let low = self.eq_low_state[ch];

        let high_alpha = 0.28;
        let high = high_alpha * (self.eq_high_prev_out[ch] + sample - self.eq_high_prev_in[ch]);
        self.eq_high_prev_in[ch] = sample;
        self.eq_high_prev_out[ch] = high;

        let mid = sample - low - high;
        low * eq.low_gain.max(0.0) + mid * eq.mid_gain.max(0.0) + high * eq.high_gain.max(0.0)
    }
}

fn eq_is_dry(eq: AudioEq) -> bool {
    (eq.low_gain - 1.0).abs() <= 1e-3
        && (eq.mid_gain - 1.0).abs() <= 1e-3
        && (eq.high_gain - 1.0).abs() <= 1e-3
}

fn apply_compression(sample: f32, compression: AudioCompression) -> f32 {
    let threshold = compression.threshold.clamp(0.0, 1.0);
    let ratio = compression.ratio.max(1.0);
    if threshold >= 0.999 || ratio <= 1.001 {
        return sample;
    }
    let sign = sample.signum();
    let amp = sample.abs();
    if amp <= threshold {
        sample
    } else {
        sign * (threshold + (amp - threshold) / ratio)
    }
}

fn delay_len(sample_rate: usize, channels: usize, seconds: f32) -> usize {
    ((sample_rate as f32 * seconds).round() as usize)
        .max(1)
        .saturating_mul(channels.max(1))
}

struct DelayLine {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl DelayLine {
    fn new(len: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
            feedback,
        }
    }

    fn process(&mut self, _channel: usize, input: f32, wet: f32) -> f32 {
        let delayed = self.buffer[self.index];
        self.buffer[self.index] = input + delayed * self.feedback * wet.clamp(0.0, 1.0);
        self.index += 1;
        if self.index >= self.buffer.len() {
            self.index = 0;
        }
        delayed
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSource {
        samples: std::vec::IntoIter<f32>,
        channels: u16,
        rate: u32,
    }

    impl Iterator for TestSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            self.samples.next()
        }
    }

    impl Source for TestSource {
        fn current_frame_len(&self) -> Option<usize> {
            Some(self.samples.len())
        }

        fn channels(&self) -> u16 {
            self.channels
        }

        fn sample_rate(&self) -> u32 {
            self.rate
        }

        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }

    #[test]
    fn low_pass_smooths_step() {
        let source = TestSource {
            samples: vec![0.0, 1.0, 1.0, 1.0].into_iter(),
            channels: 1,
            rate: 48_000,
        };
        let control = DspControl::new(DspParams {
            low_pass: 0.9,
            ..DspParams::dry()
        });
        let out = DspSource::new(source, control).collect::<Vec<_>>();
        assert!(out[1] < 0.1);
        assert!(out[3] < 0.3);
    }

    #[test]
    fn compression_reduces_peak() {
        let sample = apply_compression(
            1.0,
            AudioCompression {
                threshold: 0.5,
                ratio: 4.0,
                ..AudioCompression::default()
            },
        );
        assert!(sample < 0.7);
    }
}
