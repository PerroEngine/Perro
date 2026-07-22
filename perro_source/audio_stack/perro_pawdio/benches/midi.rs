use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use perro_pawdio::{
    AudioController, AudioPan,
    midi::{
        BuiltInMidiMixerSource, BuiltInMidiSource, MidiMixerControl, MidiMixerNote,
        MidiNoteOptions, MidiNoteRequest, MidiSong, Note, parse_built_in_midi_file,
    },
};
use std::sync::OnceLock;
use std::time::Duration;

const SAMPLE_BLOCK: usize = 128;
const MIDI_EVENTS: usize = 4096;

static MIDI_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

fn mixer_note(id: u64) -> MidiMixerNote {
    MidiMixerNote {
        id,
        note: Note::from_midi(48 + (id % 36) as u8),
        velocity: 100,
        sustain: Duration::from_secs(10),
        held: true,
        program: Default::default(),
        volume: 1.0,
    }
}

fn note_request(id: u64) -> MidiNoteRequest {
    MidiNoteRequest {
        id,
        note: Note::from_midi(48 + (id % 36) as u8),
        options: MidiNoteOptions {
            sustain: Duration::from_secs(10),
            ..Default::default()
        },
        held: true,
    }
}

fn write_vlq(mut value: u32, out: &mut Vec<u8>) {
    let mut buf = [0u8; 5];
    let mut i = 4usize;
    buf[i] = (value & 0x7f) as u8;
    while {
        value >>= 7;
        value != 0
    } {
        i -= 1;
        buf[i] = ((value & 0x7f) as u8) | 0x80;
    }
    out.extend_from_slice(&buf[i..]);
}

fn bench_midi_bytes() -> &'static [u8] {
    MIDI_BYTES.get_or_init(|| {
        let mut track = Vec::new();
        track.extend_from_slice(&[0x00, 0xff, 0x51, 0x03, 0x07, 0xa1, 0x20]);
        track.extend_from_slice(&[0x00, 0xc0, 0x00]);
        for i in 0..(MIDI_EVENTS / 2) {
            write_vlq(1, &mut track);
            track.extend_from_slice(&[0x90, 48 + (i % 36) as u8, 100]);
            write_vlq(24, &mut track);
            track.extend_from_slice(&[0x80, 48 + (i % 36) as u8, 0]);
        }
        track.extend_from_slice(&[0x00, 0xff, 0x2f, 0x00]);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MThd");
        bytes.extend_from_slice(&6u32.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&480u16.to_be_bytes());
        bytes.extend_from_slice(b"MTrk");
        bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&track);
        bytes
    })
}

fn bench_midi(c: &mut Criterion) {
    c.bench_function("pawdio_midi_builtin_render/one_mixer/1", |b| {
        b.iter_batched(
            || {
                let (tx, rx) = crossbeam_channel::unbounded();
                let mut source = BuiltInMidiMixerSource::new(rx);
                tx.send(MidiMixerControl::Note(mixer_note(0)))
                    .expect("test setup/result must succeed");
                black_box(source.next());
                source
            },
            |mut source| {
                let mut mixed = 0.0f32;
                for _ in 0..SAMPLE_BLOCK {
                    mixed += source.next().unwrap_or_default();
                }
                black_box(mixed)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    let mut group = c.benchmark_group("pawdio_midi_builtin_render");
    for voices in [16usize, 64, 256, 1024] {
        group.bench_with_input(
            BenchmarkId::new("one_mixer", voices),
            &voices,
            |b, &voices| {
                b.iter_batched(
                    || {
                        let (tx, rx) = crossbeam_channel::unbounded();
                        let mut source = BuiltInMidiMixerSource::new(rx);
                        for id in 0..voices as u64 {
                            tx.send(MidiMixerControl::Note(mixer_note(id)))
                                .expect("test setup/result must succeed");
                        }
                        black_box(source.next());
                        source
                    },
                    |mut source| {
                        let mut mixed = 0.0f32;
                        for _ in 0..SAMPLE_BLOCK {
                            mixed += source.next().unwrap_or_default();
                        }
                        black_box(mixed)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("many_sources", voices),
            &voices,
            |b, &voices| {
                b.iter_batched(
                    || {
                        (0..voices as u64)
                            .map(|id| {
                                let (_tx, rx) = crossbeam_channel::unbounded();
                                BuiltInMidiSource::note(note_request(id), rx)
                            })
                            .collect::<Vec<_>>()
                    },
                    |mut sources| {
                        let mut mixed = 0.0f32;
                        for _ in 0..SAMPLE_BLOCK {
                            for source in &mut sources {
                                mixed += source.next().unwrap_or_default();
                            }
                        }
                        black_box(mixed)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();

    c.bench_function("pawdio_midi_builtin_mixer_enqueue_1024", |b| {
        b.iter_batched(
            crossbeam_channel::unbounded,
            |(tx, rx)| {
                let mut source = BuiltInMidiMixerSource::new(rx);
                for id in 0..1024u64 {
                    tx.send(MidiMixerControl::Note(mixer_note(id)))
                        .expect("test setup/result must succeed");
                }
                black_box(source.next())
            },
            criterion::BatchSize::SmallInput,
        );
    });

    let controller = AudioController::new(None).ok();
    if let Some(controller) = controller {
        c.bench_function("pawdio_controller_enqueue_midi_note_1", |b| {
            b.iter(|| black_box(controller.play_midi_note(note_request(0))));
        });

        c.bench_function("pawdio_controller_enqueue_midi_file_1", |b| {
            b.iter(|| {
                black_box(
                    controller.play_midi_file(perro_pawdio::midi::MidiFileRequest {
                        id: 0,
                        song: MidiSong {
                            source: "res://bench.mid",
                            sound: Default::default(),
                            bus_id: None,
                            volume: 1.0,
                            looped: false,
                        },
                        pan: AudioPan::CENTER,
                    }),
                )
            });
        });

        c.bench_function("pawdio_controller_enqueue_midi_note_1024_single", |b| {
            b.iter_batched(
                || (0..1024u64).map(note_request).collect::<Vec<_>>(),
                |requests| {
                    for request in requests {
                        black_box(controller.play_midi_note(request));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });

        c.bench_function("pawdio_controller_enqueue_midi_note_1024_batch", |b| {
            b.iter_batched(
                || (0..1024u64).map(note_request).collect::<Vec<_>>(),
                |requests| black_box(controller.play_midi_notes(requests)),
                criterion::BatchSize::SmallInput,
            );
        });

        c.bench_function("pawdio_controller_enqueue_midi_note_1024_slice", |b| {
            b.iter_batched(
                || (0..1024u64).map(note_request).collect::<Vec<_>>(),
                |requests| black_box(controller.play_midi_note_slice(&requests)),
                criterion::BatchSize::SmallInput,
            );
        });
    }

    c.bench_function("pawdio_midi_file_parse_4096_events", |b| {
        b.iter(|| {
            parse_built_in_midi_file(black_box(bench_midi_bytes()))
                .expect("test setup/result must succeed")
        });
    });

    c.bench_function("pawdio_midi_file_cached_source_create_4096_events", |b| {
        let data =
            parse_built_in_midi_file(bench_midi_bytes()).expect("test setup/result must succeed");
        b.iter_batched(
            crossbeam_channel::unbounded,
            |(_tx, rx)| {
                BuiltInMidiSource::file_data(
                    data.clone(),
                    MidiSong {
                        source: "res://bench.mid",
                        sound: Default::default(),
                        bus_id: None,
                        volume: 1.0,
                        looped: true,
                    },
                    rx,
                )
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

criterion_group!(benches, bench_midi);
criterion_main!(benches);
