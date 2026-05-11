use criterion::{Criterion, criterion_group, criterion_main};
use perro_pawdio::{AudioPan, AudioPlaybackRequest, BarkPlayer};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const SOURCE: &str = "res://audio/bench.wav";
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const SAMPLE_COUNT: usize = 2_400;
const SOURCE_HASH: u64 = perro_ids::string_to_u64(SOURCE);

static WAV_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

fn bench_wav() -> &'static [u8] {
    WAV_BYTES.get_or_init(|| {
        let data_len = SAMPLE_COUNT * CHANNELS as usize * (BITS_PER_SAMPLE as usize / 8);
        let byte_rate = SAMPLE_RATE * CHANNELS as u32 * BITS_PER_SAMPLE as u32 / 8;
        let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
        let mut bytes = Vec::with_capacity(44 + data_len);

        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&CHANNELS.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(data_len as u32).to_le_bytes());

        for i in 0..SAMPLE_COUNT {
            let phase = i as f32 / SAMPLE_RATE as f32 * 440.0 * std::f32::consts::TAU;
            let sample = (phase.sin() * 0.2 * i16::MAX as f32) as i16;
            bytes.extend_from_slice(&sample.to_le_bytes());
        }

        bytes
    })
}

fn lookup_audio(path_hash: u64) -> &'static [u8] {
    match path_hash {
        SOURCE_HASH => bench_wav(),
        _ => b"",
    }
}

fn request(id: u64, from_end: f32) -> AudioPlaybackRequest<'static> {
    AudioPlaybackRequest {
        id,
        source: SOURCE,
        bus_id: None,
        looped: false,
        volume: 0.0,
        speed: 1.0,
        pan: AudioPan::CENTER,
        low_pass: 0.0,
        reverb_send: 0.0,
        echo: 0.0,
        reflection: 0.0,
        occlusion: 0.0,
        eq: Default::default(),
        compression: Default::default(),
        from_start: 0.0,
        from_end,
    }
}

fn bench_play(c: &mut Criterion) {
    let Ok(player) = BarkPlayer::new(Some(lookup_audio)) else {
        eprintln!("skip perro_pawdio play_source bench: no audio output device");
        return;
    };

    player
        .load_source(SOURCE, true)
        .expect("preload bench audio source");
    let _ = player.source_length_seconds(SOURCE);

    c.bench_function("pawdio_play_hot_cached_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                player.play_source(request(id, 0.0)).unwrap();
                elapsed += start.elapsed();
                player.stop_source(SOURCE);
            }
            elapsed
        });
    });

    c.bench_function("pawdio_play_hot_cached_wav_trim_end", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                player.play_source(request(id, 0.005)).unwrap();
                elapsed += start.elapsed();
                player.stop_source(SOURCE);
            }
            elapsed
        });
    });

    c.bench_function("pawdio_play_cold_static_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                player.drop_source_asset(SOURCE);
                let start = Instant::now();
                player.play_source(request(id, 0.0)).unwrap();
                elapsed += start.elapsed();
                player.stop_source(SOURCE);
            }
            elapsed
        });
    });
}

criterion_group!(benches, bench_play);
criterion_main!(benches);
