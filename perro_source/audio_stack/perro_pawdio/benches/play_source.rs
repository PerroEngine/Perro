use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::AudioBusID;
use perro_pawdio::{
    Audio2D, Audio3D, AudioCompression, AudioController, AudioEq, AudioListener2D, AudioListener3D,
    AudioPan, AudioPlaybackRequest, BarkPlayer, SpatialAudioParams,
};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const SOURCE: &str = "res://audio/bench.wav";
const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const SAMPLE_COUNT: usize = 2_400;
const SOURCE_POOL: usize = 256;

static WAV_BYTES: OnceLock<Vec<u8>> = OnceLock::new();
static BENCH_SOURCES: OnceLock<Vec<&'static str>> = OnceLock::new();

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

fn bench_sources() -> &'static [&'static str] {
    BENCH_SOURCES.get_or_init(|| {
        (0..SOURCE_POOL)
            .map(|i| format!("res://audio/bench_{i}.wav").leak() as &'static str)
            .collect()
    })
}

fn lookup_audio(_path_hash: u64) -> &'static [u8] {
    bench_wav()
}

fn request(id: u64, from_end: f32) -> AudioPlaybackRequest<'static> {
    request_for(id, SOURCE, None, from_end)
}

fn spatial_params(id: u64) -> SpatialAudioParams {
    let t = (id % 1024) as f32 / 1024.0;
    SpatialAudioParams {
        pan: AudioPan::new(t.mul_add(2.0, -1.0), 0.25 - t * 0.5, t * 0.75),
        volume: t,
        low_pass: t,
        reverb_send: 1.0 - t,
        echo: (t * 1.7).fract(),
        reflection: (t * 2.3).fract(),
        occlusion: (t * 3.1).fract(),
        eq: AudioEq {
            low_gain: 0.75 + t * 0.5,
            mid_gain: 1.0 - t * 0.25,
            high_gain: 0.5 + t,
        },
        compression: AudioCompression {
            threshold: 0.2 + t * 0.7,
            ratio: 1.0 + t * 7.0,
            attack: 0.005 + t * 0.04,
            release: 0.05 + t * 0.3,
        },
    }
}

fn request_for(
    id: u64,
    source: &'static str,
    bus_id: Option<AudioBusID>,
    from_end: f32,
) -> AudioPlaybackRequest<'static> {
    AudioPlaybackRequest {
        id,
        source,
        bus_id,
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
                player
                    .play_source(request(id, 0.0))
                    .expect("test setup/result must succeed");
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
                player
                    .play_source(request(id, 0.005))
                    .expect("test setup/result must succeed");
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
                player
                    .play_source(request(id, 0.0))
                    .expect("test setup/result must succeed");
                elapsed += start.elapsed();
                player.stop_source(SOURCE);
            }
            elapsed
        });
    });

    for source in bench_sources() {
        player
            .load_source(source, true)
            .expect("preload pooled bench audio source");
    }

    let mut many_active = c.benchmark_group("pawdio_play_cached_many_active");
    for active in [16usize, 64, 256] {
        many_active.bench_with_input(
            BenchmarkId::new("multi_bus_sources", active),
            &active,
            |b, &active| {
                b.iter_custom(|iters| {
                    let sources = bench_sources();
                    let mut elapsed = Duration::ZERO;
                    for id in 0..iters {
                        let idx = id as usize % active;
                        let bus = AudioBusID::from_u64((idx % 64) as u64 + 1);
                        let start = Instant::now();
                        player
                            .play_source(request_for(id, sources[idx], Some(bus), 0.0))
                            .expect("test setup/result must succeed");
                        elapsed += start.elapsed();
                    }
                    elapsed
                });
            },
        );
    }
    many_active.finish();

    let listener_2d = AudioListener2D::default();
    c.bench_function("pawdio_request_2d_to_playback", |b| {
        b.iter(|| {
            black_box(
                Audio2D::new(black_box(SOURCE), black_box([5.0, 2.0]), black_box(32.0))
                    .to_playback(black_box(listener_2d)),
            )
        });
    });

    let listener_3d = AudioListener3D::default();
    c.bench_function("pawdio_request_3d_to_playback", |b| {
        b.iter(|| {
            black_box(
                Audio3D::new(
                    black_box(SOURCE),
                    black_box([5.0, 2.0, -8.0]),
                    black_box(32.0),
                )
                .to_playback(black_box(listener_3d)),
            )
        });
    });

    c.bench_function("pawdio_play_cached_2d_64_active", |b| {
        b.iter_custom(|iters| {
            let sources = bench_sources();
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let idx = id as usize % 64;
                let audio = Audio2D::new(sources[idx], [idx as f32 * 0.1, 2.0], 32.0);
                let mut req = audio
                    .to_playback(listener_2d)
                    .expect("test setup/result must succeed");
                req.id = id;
                req.bus_id = Some(AudioBusID::from_u64((idx % 16) as u64 + 1));
                let start = Instant::now();
                player
                    .play_source(req)
                    .expect("test setup/result must succeed");
                elapsed += start.elapsed();
            }
            elapsed
        });
    });

    c.bench_function("pawdio_play_cached_3d_64_active", |b| {
        b.iter_custom(|iters| {
            let sources = bench_sources();
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let idx = id as usize % 64;
                let audio = Audio3D::new(sources[idx], [idx as f32 * 0.1, 2.0, -8.0], 32.0);
                let mut req = audio
                    .to_playback(listener_3d)
                    .expect("test setup/result must succeed");
                req.id = id;
                req.bus_id = Some(AudioBusID::from_u64((idx % 16) as u64 + 1));
                let start = Instant::now();
                player
                    .play_source(req)
                    .expect("test setup/result must succeed");
                elapsed += start.elapsed();
            }
            elapsed
        });
    });

    const UPDATE_BENCH_ID: u64 = 900_000_000;
    let mut update_req = request(UPDATE_BENCH_ID, 0.0);
    update_req.looped = true;
    update_req.volume = 0.0;
    player
        .play_source(update_req)
        .expect("test setup/result must succeed");

    c.bench_function("pawdio_update_spatial_direct_full_dsp", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                assert!(player.update_spatial(UPDATE_BENCH_ID, black_box(spatial_params(id))));
                elapsed += start.elapsed();
            }
            elapsed
        });
    });

    let Ok(controller) = AudioController::new(Some(lookup_audio)) else {
        eprintln!("skip perro_pawdio controller enqueue bench: no audio output device");
        return;
    };
    controller.reserve_source(SOURCE);
    let _ = controller.source_length_seconds(SOURCE);
    let source_handle = controller.source_handle(SOURCE);

    c.bench_function("pawdio_controller_enqueue_play_cached_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                assert!(controller.play_source(request(id, 0.0)));
                elapsed += start.elapsed();
                if id % 1024 == 1023 {
                    let _ = controller.source_length_seconds(SOURCE);
                }
            }
            let _ = controller.source_length_seconds(SOURCE);
            elapsed
        });
    });

    c.bench_function("pawdio_controller_enqueue_play_handle_cached_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                assert!(controller.play_source_handle(&source_handle, request(id, 0.0)));
                elapsed += start.elapsed();
                if id % 1024 == 1023 {
                    let _ = controller.source_length_seconds(SOURCE);
                }
            }
            let _ = controller.source_length_seconds(SOURCE);
            elapsed
        });
    });

    c.bench_function("pawdio_controller_enqueue_spatial_cached_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let idx = id as usize % 64;
                let audio = Audio3D::new(SOURCE, [idx as f32 * 0.1, 2.0, -8.0], 32.0);
                let req = audio
                    .to_playback(listener_3d)
                    .expect("test setup/result must succeed");
                let start = Instant::now();
                assert!(controller.play_spatial_source(req).is_some());
                elapsed += start.elapsed();
                if id % 1024 == 1023 {
                    let _ = controller.source_length_seconds(SOURCE);
                }
            }
            let _ = controller.source_length_seconds(SOURCE);
            elapsed
        });
    });

    c.bench_function("pawdio_controller_enqueue_spatial_handle_cached_wav", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let idx = id as usize % 64;
                let audio = Audio3D::new(SOURCE, [idx as f32 * 0.1, 2.0, -8.0], 32.0);
                let req = audio
                    .to_playback(listener_3d)
                    .expect("test setup/result must succeed");
                let start = Instant::now();
                assert!(
                    controller
                        .play_spatial_source_handle(&source_handle, req)
                        .is_some()
                );
                elapsed += start.elapsed();
                if id % 1024 == 1023 {
                    let _ = controller.source_length_seconds(SOURCE);
                }
            }
            let _ = controller.source_length_seconds(SOURCE);
            elapsed
        });
    });

    let update_id = controller
        .play_spatial_source(request(0, 0.0))
        .expect("start controller update bench playback");
    let _ = controller.source_length_seconds(SOURCE);
    c.bench_function("pawdio_controller_enqueue_update_spatial_full_dsp", |b| {
        b.iter_custom(|iters| {
            let mut elapsed = Duration::ZERO;
            for id in 0..iters {
                let start = Instant::now();
                assert!(controller.update_spatial(update_id, black_box(spatial_params(id))));
                elapsed += start.elapsed();
                if id % 1024 == 1023 {
                    let _ = controller.source_length_seconds(SOURCE);
                }
            }
            let _ = controller.source_length_seconds(SOURCE);
            elapsed
        });
    });
}

criterion_group!(benches, bench_play);
criterion_main!(benches);
