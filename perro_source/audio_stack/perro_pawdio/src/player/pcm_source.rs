use super::*;

pub(super) struct CachedPcmSource {
    pcm: Arc<CachedPcm>,
    position: usize,
}

impl CachedPcmSource {
    pub(super) fn new(pcm: Arc<CachedPcm>) -> Self {
        Self { pcm, position: 0 }
    }
}

impl Iterator for CachedPcmSource {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        let sample = self.pcm.samples.get(self.position).copied()?;
        self.position += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.pcm.samples.len().saturating_sub(self.position);
        (remaining, Some(remaining))
    }
}

impl Source for CachedPcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.pcm.samples.len().saturating_sub(self.position))
    }

    fn channels(&self) -> u16 {
        self.pcm.channels
    }

    fn sample_rate(&self) -> u32 {
        self.pcm.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.pcm.duration())
    }
}

// Shared append tail for both the PCM and streaming decode paths: apply trim
// (and optional take/loop) then route through the DSP chain into the sink.
pub(super) fn append_with_trims<S>(
    sink: &SpatialSink,
    source: S,
    dsp: Arc<DspControl>,
    trim_start: Duration,
    play_duration: Option<Duration>,
    looped: bool,
) where
    S: Source<Item = f32> + Send + 'static,
{
    match (play_duration, looped) {
        (Some(duration), true) => sink.append(DspSource::new(
            source
                .skip_duration(trim_start)
                .take_duration(duration)
                .repeat_infinite(),
            dsp,
        )),
        (Some(duration), false) => sink.append(DspSource::new(
            source.skip_duration(trim_start).take_duration(duration),
            dsp,
        )),
        (None, true) => sink.append(DspSource::new(
            source.skip_duration(trim_start).repeat_infinite(),
            dsp,
        )),
        (None, false) => sink.append(DspSource::new(source.skip_duration(trim_start), dsp)),
    }
}
