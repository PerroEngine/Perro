//! Matched public-API lookup probe. Paused, muted voices keep output silent.
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_ids::AudioBusID;
use perro_pawdio::{AudioPlaybackRequest, BarkPlayer, SpatialAudioParams};
use std::sync::OnceLock;

fn wav(_: u64) -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let n = 4800u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + n).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&96_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&n.to_le_bytes());
        bytes.resize(44 + n as usize, 0);
        bytes
    })
}

fn playback(c: &mut Criterion) {
    let Ok(player) = BarkPlayer::new(Some(wav)) else {
        panic!("audit_playback requires an audio output device");
    };
    player.set_master_volume(0.0);
    let bus = AudioBusID::from_u64(912_334);
    player.pause_bus(bus);
    let mut group = c.benchmark_group("audit_audio/update_voice");
    for count in [1usize, 8, 32, 256, 1000] {
        player.stop_all();
        let sources: Vec<String> = (0..count)
            .map(|i| format!("res://audit/voice_{i}.wav"))
            .collect();
        for (i, source) in sources.iter().enumerate() {
            player.load_source(source, true).expect("load silent PCM");
            player
                .play_source(AudioPlaybackRequest {
                    id: i as u64 + 1,
                    source,
                    bus_id: Some(bus),
                    looped: true,
                    volume: 0.0,
                    speed: 1.0,
                    pan: Default::default(),
                    low_pass: 0.0,
                    reverb_send: 0.0,
                    echo: 0.0,
                    reflection: 0.0,
                    occlusion: 0.0,
                    eq: Default::default(),
                    compression: Default::default(),
                    from_start: 0.0,
                    from_end: 0.0,
                })
                .expect("start paused voice");
        }
        let params = SpatialAudioParams {
            volume: 0.0,
            ..Default::default()
        };
        assert!(player.update_spatial(count as u64, params));
        group.bench_with_input(BenchmarkId::new("last", count), &count, |b, &count| {
            b.iter(|| assert!(player.update_spatial(black_box(count as u64), black_box(params))));
        });
        group.bench_with_input(BenchmarkId::new("cycle", count), &count, |b, &count| {
            let mut cursor = 0usize;
            b.iter(|| {
                cursor = (cursor + 1) % count;
                assert!(player.update_spatial(black_box(cursor as u64 + 1), black_box(params)));
            });
        });
    }
    group.finish();
    player.stop_all();
}

criterion_group!(benches, playback);
criterion_main!(benches);
