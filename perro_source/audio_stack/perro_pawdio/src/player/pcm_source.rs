use super::*;

pub(super) struct CachedPcmSource {
    pcm: Arc<CachedPcm>,
    position: usize,
}

impl CachedPcmSource {
    // Trim from the front by indexing instead of `skip_duration`, which pulls
    // (and discards) every trimmed sample one at a time.
    pub(super) fn starting_at(pcm: Arc<CachedPcm>, trim_start: Duration) -> Self {
        let channels = pcm.channels.max(1) as usize;
        let frames = (trim_start.as_secs_f64() * pcm.sample_rate.max(1) as f64) as usize;
        let position = frames
            .saturating_mul(channels)
            .min(pcm.samples.len() - pcm.samples.len() % channels.max(1));
        Self { pcm, position }
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

// Shared append tail: apply the optional take/loop, then route through the DSP
// chain into the sink. The front trim is already applied by the caller.
fn append_trimmed<S>(
    sink: &SpatialSink,
    source: S,
    dsp: Arc<DspControl>,
    play_duration: Option<Duration>,
    looped: bool,
) where
    S: Source<Item = f32> + Send + 'static,
{
    match (play_duration, looped) {
        (Some(duration), true) => {
            sink.append(DspSource::new(
                source.take_duration(duration).repeat_infinite(),
                dsp,
            ));
        }
        (Some(duration), false) => {
            sink.append(DspSource::new(source.take_duration(duration), dsp));
        }
        (None, true) => sink.append(DspSource::new(source.repeat_infinite(), dsp)),
        (None, false) => sink.append(DspSource::new(source, dsp)),
    }
}

// Streaming decode path: the decoder has no cheap seek, so the front trim
// still costs one pulled sample per trimmed sample.
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
    append_trimmed(
        sink,
        source.skip_duration(trim_start),
        dsp,
        play_duration,
        looped,
    );
}

// Cached PCM path: the front trim is an index offset, so a trimmed replay
// costs nothing extra.
pub(super) fn append_cached_with_trims(
    sink: &SpatialSink,
    pcm: Arc<CachedPcm>,
    dsp: Arc<DspControl>,
    trim_start: Duration,
    play_duration: Option<Duration>,
    looped: bool,
) {
    append_trimmed(
        sink,
        CachedPcmSource::starting_at(pcm, trim_start),
        dsp,
        play_duration,
        looped,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(channels: u16, sample_rate: u32, samples: &[f32]) -> Arc<CachedPcm> {
        Arc::new(CachedPcm {
            channels,
            sample_rate,
            samples: Arc::from(samples.to_vec().into_boxed_slice()),
        })
    }

    /// The front trim lands on a frame boundary and costs no per-sample work.
    #[test]
    fn cached_trim_seeks_by_index() {
        let clip = pcm(2, 4, &[0.0, 0.1, 1.0, 1.1, 2.0, 2.1, 3.0, 3.1]);
        let source = CachedPcmSource::starting_at(clip.clone(), Duration::from_millis(500));
        assert_eq!(source.position, 4);
        assert_eq!(source.collect::<Vec<_>>(), vec![2.0, 2.1, 3.0, 3.1]);

        // Past the end: empty, never out of bounds.
        let source = CachedPcmSource::starting_at(clip, Duration::from_secs(9));
        assert!(source.collect::<Vec<_>>().is_empty());
    }
}
