//! Baseline-compatible probes; run serially from matching release test binaries.
use super::*;
use std::hint::black_box;
use std::time::Instant;

fn bytes(wet: &WetDelays) -> usize {
    (wet.echo.buffer.capacity() + wet.reverb_a.buffer.capacity() + wet.reverb_b.buffer.capacity())
        * size_of::<f32>()
}

#[test]
#[ignore = "manual release performance probe"]
fn audit_wet_delay_storage() {
    let mut params = DspParams::dry();
    params.echo = 0.5;
    let controls: Vec<_> = (0..100).map(|_| DspControl::new(params)).collect();
    let sources: Vec<_> = controls
        .iter()
        .map(|control| {
            DspSource::new(
                rodio::buffer::SamplesBuffer::new(2, 48_000, vec![0.25_f32; 256]),
                Arc::clone(control),
            )
        })
        .collect();
    let owned: usize = sources
        .iter()
        .map(|s| bytes(s.wet.as_ref().expect("wet source")))
        .sum();
    for control in &controls {
        control.update_spatial(SpatialAudioParams {
            echo: 0.5,
            ..Default::default()
        });
    }
    let spare: usize = controls
        .iter()
        .map(|control| {
            control
                .pending_wet
                .lock()
                .expect("stage slot")
                .as_ref()
                .map_or(0, |wet| bytes(wet))
        })
        .sum();
    println!("AUDIT audio_wet_storage voices=100 owned_bytes={owned} spare_bytes={spare}");
    assert_eq!(owned, 11_443_200);
    let mut samples = Vec::new();
    for _ in 0..15 {
        let begin = Instant::now();
        for _ in 0..100 {
            for control in &controls {
                control.update_spatial(black_box(SpatialAudioParams {
                    echo: 0.5,
                    ..Default::default()
                }));
            }
        }
        samples.push(begin.elapsed().as_nanos());
    }
    println!("AUDIT audio_wet_updates operations=10000 samples_ns={samples:?}");
    black_box(sources);
}
