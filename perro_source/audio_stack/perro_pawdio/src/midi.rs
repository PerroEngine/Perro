use crossbeam_channel::Receiver;
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use perro_ids::AudioBusID;
use perro_ids::SoundFontID;
use rodio::Source;
use rustysynth::{
    MidiFile as RustyMidiFile, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings,
};
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::types::AudioPan;

pub const SAMPLE_RATE: u32 = 44_100;
const BUILT_IN_MIXER_MAX_VOICES: usize = 4096;
const SINE_TABLE_SIZE: usize = 2048;
const SINE_TABLE_MASK: usize = SINE_TABLE_SIZE - 1;
const ATTACK_STEP: f32 = 128.0 / SAMPLE_RATE as f32;
const RELEASE_STEP: f32 = 1.0 / (SAMPLE_RATE as f32 * 0.18);
const OUTPUT_GAIN: f32 = 0.18;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Note(pub u8);

#[allow(non_upper_case_globals)]
impl Note {
    pub const fn from_midi(value: u8) -> Self {
        Self(value)
    }

    pub const fn midi_key(self) -> u8 {
        self.0
    }

    pub fn frequency_hz(self) -> f32 {
        440.0 * 2.0_f32.powf((self.0 as f32 - 69.0) / 12.0)
    }

    pub const C0: Self = Self(12);
    pub const Cs0: Self = Self(13);
    pub const D0: Self = Self(14);
    pub const Ds0: Self = Self(15);
    pub const E0: Self = Self(16);
    pub const F0: Self = Self(17);
    pub const Fs0: Self = Self(18);
    pub const G0: Self = Self(19);
    pub const Gs0: Self = Self(20);
    pub const A0: Self = Self(21);
    pub const As0: Self = Self(22);
    pub const B0: Self = Self(23);
    pub const C1: Self = Self(24);
    pub const Cs1: Self = Self(25);
    pub const D1: Self = Self(26);
    pub const Ds1: Self = Self(27);
    pub const E1: Self = Self(28);
    pub const F1: Self = Self(29);
    pub const Fs1: Self = Self(30);
    pub const G1: Self = Self(31);
    pub const Gs1: Self = Self(32);
    pub const A1: Self = Self(33);
    pub const As1: Self = Self(34);
    pub const B1: Self = Self(35);
    pub const C2: Self = Self(36);
    pub const Cs2: Self = Self(37);
    pub const D2: Self = Self(38);
    pub const Ds2: Self = Self(39);
    pub const E2: Self = Self(40);
    pub const F2: Self = Self(41);
    pub const Fs2: Self = Self(42);
    pub const G2: Self = Self(43);
    pub const Gs2: Self = Self(44);
    pub const A2: Self = Self(45);
    pub const As2: Self = Self(46);
    pub const B2: Self = Self(47);
    pub const C3: Self = Self(48);
    pub const Cs3: Self = Self(49);
    pub const D3: Self = Self(50);
    pub const Ds3: Self = Self(51);
    pub const E3: Self = Self(52);
    pub const F3: Self = Self(53);
    pub const Fs3: Self = Self(54);
    pub const G3: Self = Self(55);
    pub const Gs3: Self = Self(56);
    pub const A3: Self = Self(57);
    pub const As3: Self = Self(58);
    pub const B3: Self = Self(59);
    pub const C4: Self = Self(60);
    pub const Cs4: Self = Self(61);
    pub const D4: Self = Self(62);
    pub const Ds4: Self = Self(63);
    pub const E4: Self = Self(64);
    pub const F4: Self = Self(65);
    pub const Fs4: Self = Self(66);
    pub const G4: Self = Self(67);
    pub const Gs4: Self = Self(68);
    pub const A4: Self = Self(69);
    pub const As4: Self = Self(70);
    pub const B4: Self = Self(71);
    pub const C5: Self = Self(72);
    pub const Cs5: Self = Self(73);
    pub const D5: Self = Self(74);
    pub const Ds5: Self = Self(75);
    pub const E5: Self = Self(76);
    pub const F5: Self = Self(77);
    pub const Fs5: Self = Self(78);
    pub const G5: Self = Self(79);
    pub const Gs5: Self = Self(80);
    pub const A5: Self = Self(81);
    pub const As5: Self = Self(82);
    pub const B5: Self = Self(83);
    pub const C6: Self = Self(84);
    pub const Cs6: Self = Self(85);
    pub const D6: Self = Self(86);
    pub const Ds6: Self = Self(87);
    pub const E6: Self = Self(88);
    pub const F6: Self = Self(89);
    pub const Fs6: Self = Self(90);
    pub const G6: Self = Self(91);
    pub const Gs6: Self = Self(92);
    pub const A6: Self = Self(93);
    pub const As6: Self = Self(94);
    pub const B6: Self = Self(95);
    pub const C7: Self = Self(96);
    pub const Cs7: Self = Self(97);
    pub const D7: Self = Self(98);
    pub const Ds7: Self = Self(99);
    pub const E7: Self = Self(100);
    pub const F7: Self = Self(101);
    pub const Fs7: Self = Self(102);
    pub const G7: Self = Self(103);
    pub const Gs7: Self = Self(104);
    pub const A7: Self = Self(105);
    pub const As7: Self = Self(106);
    pub const B7: Self = Self(107);
    pub const C8: Self = Self(108);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MidiChannel(pub u8);

impl MidiChannel {
    pub const DEFAULT: Self = Self(0);
    pub const DRUMS: Self = Self(9);

    pub const fn new(channel: u8) -> Self {
        Self(if channel > 15 { 15 } else { channel })
    }
}

impl Default for MidiChannel {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MidiProgram(pub u8);

impl MidiProgram {
    pub const ACOUSTIC_GRAND_PIANO: Self = Self(0);
    pub const fn new(program: u8) -> Self {
        Self(if program > 127 { 127 } else { program })
    }
}

impl Default for MidiProgram {
    fn default() -> Self {
        Self::ACOUSTIC_GRAND_PIANO
    }
}

macro_rules! program_group {
    ($name:ident { $($variant:ident = $value:expr,)* }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name;

        #[allow(non_upper_case_globals)]
        impl $name {
            $(pub const $variant: MidiProgram = MidiProgram($value);)*
        }
    };
}

pub mod program {
    use super::MidiProgram;

    program_group!(Piano {
        AcousticGrand = 0, BrightAcoustic = 1, ElectricGrand = 2, HonkyTonk = 3,
        Electric1 = 4, Electric2 = 5, Harpsichord = 6, Clavinet = 7,
    });
    program_group!(Chromatic {
        Celesta = 8, Glockenspiel = 9, MusicBox = 10, Vibraphone = 11,
        Marimba = 12, Xylophone = 13, TubularBells = 14, Dulcimer = 15,
    });
    program_group!(Organ {
        Drawbar = 16, Percussive = 17, Rock = 18, Church = 19,
        Reed = 20, Accordion = 21, Harmonica = 22, TangoAccordion = 23,
    });
    program_group!(Guitar {
        Nylon = 24, Steel = 25, Jazz = 26, Clean = 27,
        Muted = 28, Overdriven = 29, Distortion = 30, Harmonics = 31,
    });
    program_group!(Bass {
        Acoustic = 32, Finger = 33, Pick = 34, Fretless = 35,
        Slap1 = 36, Slap2 = 37, Synth1 = 38, Synth2 = 39,
    });
    program_group!(Strings {
        Violin = 40, Viola = 41, Cello = 42, Contrabass = 43,
        Tremolo = 44, Pizzicato = 45, Harp = 46, Timpani = 47,
    });
    program_group!(Ensemble {
        String1 = 48, String2 = 49, SynthStrings1 = 50, SynthStrings2 = 51,
        ChoirAahs = 52, VoiceOohs = 53, SynthVoice = 54, OrchestraHit = 55,
    });
    program_group!(Brass {
        Trumpet = 56, Trombone = 57, Tuba = 58, MutedTrumpet = 59,
        FrenchHorn = 60, BrassSection = 61, SynthBrass1 = 62, SynthBrass2 = 63,
    });
    program_group!(Reed {
        SopranoSax = 64, AltoSax = 65, TenorSax = 66, BaritoneSax = 67,
        Oboe = 68, EnglishHorn = 69, Bassoon = 70, Clarinet = 71,
    });
    program_group!(Pipe {
        Piccolo = 72, Flute = 73, Recorder = 74, PanFlute = 75,
        BlownBottle = 76, Shakuhachi = 77, Whistle = 78, Ocarina = 79,
    });
    program_group!(SynthLead {
        Square = 80, Saw = 81, Calliope = 82, Chiff = 83,
        Charang = 84, Voice = 85, Fifths = 86, BassLead = 87,
    });
    program_group!(SynthPad {
        NewAge = 88, Warm = 89, Polysynth = 90, Choir = 91,
        Bowed = 92, Metallic = 93, Halo = 94, Sweep = 95,
    });
    program_group!(SynthFx {
        Rain = 96, Soundtrack = 97, Crystal = 98, Atmosphere = 99,
        Brightness = 100, Goblins = 101, Echoes = 102, SciFi = 103,
    });
    program_group!(World {
        Sitar = 104, Banjo = 105, Shamisen = 106, Koto = 107,
        Kalimba = 108, Bagpipe = 109, Fiddle = 110, Shanai = 111,
    });
    program_group!(Percussive {
        TinkleBell = 112, Agogo = 113, SteelDrums = 114, Woodblock = 115,
        TaikoDrum = 116, MelodicTom = 117, SynthDrum = 118, ReverseCymbal = 119,
    });
    program_group!(SoundFx {
        GuitarFretNoise = 120, BreathNoise = 121, Seashore = 122, BirdTweet = 123,
        TelephoneRing = 124, Helicopter = 125, Applause = 126, Gunshot = 127,
    });
    program_group!(DrumKit {
        Standard = 0, Room = 8, Power = 16, Electronic = 24,
        Analog = 25, Jazz = 32, Brush = 40, Orchestra = 48, Sfx = 56,
    });
}

#[derive(Clone, Copy, Debug, Default)]
pub enum MidiSound {
    #[default]
    BuiltIn,
    SoundFont(SoundFontID),
}

#[derive(Clone, Copy, Debug)]
pub struct MidiNoteOptions {
    pub velocity: u8,
    pub sustain: Duration,
    pub channel: MidiChannel,
    pub program: MidiProgram,
    pub sound: MidiSound,
    pub bus_id: Option<AudioBusID>,
    pub volume: f32,
    pub pan: AudioPan,
}

impl MidiNoteOptions {
    pub const fn new() -> Self {
        Self {
            velocity: 100,
            sustain: Duration::from_millis(250),
            channel: MidiChannel::DEFAULT,
            program: MidiProgram::ACOUSTIC_GRAND_PIANO,
            sound: MidiSound::BuiltIn,
            bus_id: None,
            volume: 1.0,
            pan: AudioPan::CENTER,
        }
    }

    pub const fn with_bus(mut self, bus_id: AudioBusID) -> Self {
        self.bus_id = Some(bus_id);
        self
    }

    pub const fn with_program(mut self, program: MidiProgram) -> Self {
        self.program = program;
        self
    }

    pub const fn with_sound(mut self, sound: MidiSound) -> Self {
        self.sound = sound;
        self
    }
}

impl Default for MidiNoteOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MidiSong<'a> {
    pub source: &'a str,
    pub sound: MidiSound,
    pub bus_id: Option<AudioBusID>,
    pub volume: f32,
    pub looped: bool,
}

impl<'a> MidiSong<'a> {
    pub const fn new(source: &'a str) -> Self {
        Self {
            source,
            sound: MidiSound::BuiltIn,
            bus_id: None,
            volume: 1.0,
            looped: false,
        }
    }

    pub const fn with_sound(mut self, sound: MidiSound) -> Self {
        self.sound = sound;
        self
    }

    pub const fn looped(mut self) -> Self {
        self.looped = true;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MidiNoteHandle(pub u64);

#[derive(Clone, Copy, Debug)]
pub struct MidiNoteRequest {
    pub id: u64,
    pub note: Note,
    pub options: MidiNoteOptions,
    pub held: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct MidiFileRequest<'a> {
    pub id: u64,
    pub song: MidiSong<'a>,
    pub pan: AudioPan,
}

#[derive(Clone, Copy, Debug)]
pub enum MidiControl {
    Release,
    Stop,
}

#[derive(Clone, Copy, Debug)]
pub enum MidiMixerControl {
    Note(MidiMixerNote),
    Release { id: u64 },
    Stop,
}

#[derive(Clone, Copy, Debug)]
pub enum SoundFontMixerControl {
    Note(SoundFontMixerNote),
    Release { id: u64 },
    Stop,
}

#[derive(Clone, Copy, Debug)]
pub struct SoundFontMixerNote {
    pub id: u64,
    pub note: Note,
    pub velocity: u8,
    pub sustain: Duration,
    pub held: bool,
    pub channel: MidiChannel,
    pub program: MidiProgram,
}

#[derive(Clone, Copy, Debug)]
pub struct MidiMixerNote {
    pub id: u64,
    pub note: Note,
    pub velocity: u8,
    pub sustain: Duration,
    pub held: bool,
    pub program: MidiProgram,
    pub volume: f32,
}

#[derive(Clone, Copy)]
struct MidiEvent {
    sample: u64,
    kind: MidiEventKind,
}

#[doc(hidden)]
pub struct BuiltInMidiFileData {
    events: Arc<[MidiEvent]>,
    end_sample: u64,
}

#[derive(Clone, Copy)]
enum MidiEventKind {
    NoteOn {
        note: Note,
        velocity: u8,
        program: MidiProgram,
    },
    NoteOff {
        note: Note,
    },
    Program(MidiProgram),
}

#[derive(Clone, Copy)]
enum WaveKind {
    Sine,
    Square,
    Saw,
    Triangle,
    Noise,
}

struct Voice {
    id: Option<u64>,
    note: Note,
    velocity: f32,
    volume: f32,
    gain: f32,
    release_gain: f32,
    phase: f32,
    phase_step: f32,
    age_samples: u64,
    auto_release_sample: Option<u64>,
    released: bool,
    wave: WaveKind,
    noise: u32,
}

impl Voice {
    fn new(note: Note, velocity: u8, volume: f32, program: MidiProgram) -> Self {
        Self::new_internal(None, note, velocity, volume, program, None)
    }

    fn live(note: MidiMixerNote) -> Self {
        let auto_release_sample = (!note.held).then_some(duration_samples(note.sustain));
        Self::new_internal(
            Some(note.id),
            note.note,
            note.velocity,
            note.volume,
            note.program,
            auto_release_sample,
        )
    }

    fn new_internal(
        id: Option<u64>,
        note: Note,
        velocity: u8,
        volume: f32,
        program: MidiProgram,
        auto_release_sample: Option<u64>,
    ) -> Self {
        let wave = program_wave(program);
        Self {
            id,
            note,
            velocity: (velocity as f32 / 127.0).clamp(0.0, 1.0) * volume.max(0.0) * OUTPUT_GAIN,
            volume: 1.0,
            gain: 0.0,
            release_gain: 1.0,
            phase: 0.0,
            phase_step: TAU * note.frequency_hz() / SAMPLE_RATE as f32,
            age_samples: 0,
            auto_release_sample,
            released: false,
            wave,
            noise: 0x1234_5678 ^ ((note.0 as u32) << 8) ^ program.0 as u32,
        }
    }

    fn release(&mut self) {
        self.released = true;
    }

    fn next_sample(&mut self) -> Option<f32> {
        if self
            .auto_release_sample
            .is_some_and(|sample| self.age_samples >= sample)
        {
            self.release();
            self.auto_release_sample = None;
        }
        if self.gain < 1.0 {
            self.gain = (self.gain + ATTACK_STEP).min(1.0);
        }
        if self.released {
            self.release_gain -= RELEASE_STEP;
        }
        if self.release_gain <= 0.0 {
            return None;
        }

        let raw = match self.wave {
            WaveKind::Sine => fast_sine(self.phase),
            WaveKind::Square => {
                if self.phase < std::f32::consts::PI {
                    1.0
                } else {
                    -1.0
                }
            }
            WaveKind::Saw => (self.phase / TAU) * 2.0 - 1.0,
            WaveKind::Triangle => {
                let t = self.phase / TAU;
                4.0 * (t - 0.5).abs() - 1.0
            }
            WaveKind::Noise => {
                self.noise = self.noise.wrapping_mul(1664525).wrapping_add(1013904223);
                (((self.noise >> 16) & 0xffff) as f32 / 32768.0) - 1.0
            }
        };
        self.phase += self.phase_step;
        if self.phase >= TAU {
            self.phase -= TAU;
        }
        self.age_samples = self.age_samples.saturating_add(1);
        Some(raw * self.gain * self.release_gain * self.velocity * self.volume)
    }
}

fn fast_sine(phase: f32) -> f32 {
    let table = SINE_TABLE.get_or_init(|| {
        let mut table = [0.0f32; SINE_TABLE_SIZE];
        let mut i = 0usize;
        while i < SINE_TABLE_SIZE {
            table[i] = (i as f32 * TAU / SINE_TABLE_SIZE as f32).sin();
            i += 1;
        }
        table
    });
    let idx = ((phase * (SINE_TABLE_SIZE as f32 / TAU)) as usize) & SINE_TABLE_MASK;
    table[idx]
}

static SINE_TABLE: OnceLock<[f32; SINE_TABLE_SIZE]> = OnceLock::new();

pub struct BuiltInMidiMixerSource {
    voices: Vec<Voice>,
    voice_index: HashMap<u64, usize>,
    rx: Receiver<MidiMixerControl>,
    stopped: bool,
}

impl BuiltInMidiMixerSource {
    pub fn new(rx: Receiver<MidiMixerControl>) -> Self {
        Self {
            voices: Vec::with_capacity(BUILT_IN_MIXER_MAX_VOICES),
            voice_index: HashMap::with_capacity(BUILT_IN_MIXER_MAX_VOICES),
            rx,
            stopped: false,
        }
    }

    fn process_controls(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                MidiMixerControl::Note(note) => self.push_voice(note),
                MidiMixerControl::Release { id } => {
                    if let Some(index) = self.voice_index.get(&id).copied()
                        && let Some(voice) = self.voices.get_mut(index)
                    {
                        voice.release();
                    }
                }
                MidiMixerControl::Stop => self.stopped = true,
            }
        }
    }

    fn push_voice(&mut self, note: MidiMixerNote) {
        if self.voices.len() >= BUILT_IN_MIXER_MAX_VOICES {
            self.remove_voice_at(0);
        }
        let index = self.voices.len();
        self.voices.push(Voice::live(note));
        self.voice_index.insert(note.id, index);
    }

    fn remove_voice_at(&mut self, index: usize) {
        let removed_id = self.voices[index].id;
        self.voices.swap_remove(index);
        if let Some(id) = removed_id {
            self.voice_index.remove(&id);
        }
        if index < self.voices.len()
            && let Some(id) = self.voices[index].id
        {
            self.voice_index.insert(id, index);
        }
    }

    #[inline]
    pub fn active_voice_count(&self) -> usize {
        self.voices.len()
    }
}

impl Iterator for BuiltInMidiMixerSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        self.process_controls();
        let mut mixed = 0.0;
        let mut i = 0usize;
        while i < self.voices.len() {
            if let Some(sample) = self.voices[i].next_sample() {
                mixed += sample;
                i += 1;
            } else {
                self.remove_voice_at(i);
            }
        }
        Some(mixed.clamp(-1.0, 1.0))
    }
}

impl Source for BuiltInMidiMixerSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn program_wave(program: MidiProgram) -> WaveKind {
    match program.0 {
        16..=23 => WaveKind::Square,
        24..=39 => WaveKind::Saw,
        40..=55 => WaveKind::Triangle,
        56..=79 => WaveKind::Saw,
        80..=87 => WaveKind::Square,
        88..=103 => WaveKind::Sine,
        112..=127 => WaveKind::Noise,
        _ => WaveKind::Sine,
    }
}

pub struct BuiltInMidiSource {
    voices: Vec<Voice>,
    events: Arc<[MidiEvent]>,
    event_index: usize,
    sample: u64,
    loop_samples: Option<u64>,
    rx: Receiver<MidiControl>,
    released: bool,
    stopped: bool,
    volume: f32,
    current_program: MidiProgram,
}

impl BuiltInMidiSource {
    pub fn note(request: MidiNoteRequest, rx: Receiver<MidiControl>) -> Self {
        let mut source = Self {
            voices: Vec::new(),
            events: Arc::from(Vec::<MidiEvent>::new().into_boxed_slice()),
            event_index: 0,
            sample: 0,
            loop_samples: None,
            rx,
            released: false,
            stopped: false,
            volume: request.options.volume.max(0.0),
            current_program: request.options.program,
        };
        source.voices.push(Voice::new(
            request.note,
            request.options.velocity,
            source.volume,
            request.options.program,
        ));
        if !request.held {
            source.events = Arc::from(
                vec![MidiEvent {
                    sample: duration_samples(request.options.sustain),
                    kind: MidiEventKind::NoteOff { note: request.note },
                }]
                .into_boxed_slice(),
            );
        }
        source
    }

    pub fn file(bytes: &[u8], song: MidiSong, rx: Receiver<MidiControl>) -> Result<Self, String> {
        let data = parse_built_in_midi_file(bytes)?;
        Ok(Self::file_data(data, song, rx))
    }

    #[doc(hidden)]
    pub fn file_data(
        data: Arc<BuiltInMidiFileData>,
        song: MidiSong,
        rx: Receiver<MidiControl>,
    ) -> Self {
        let loop_samples = song
            .looped
            .then_some(data.end_sample.max(SAMPLE_RATE as u64 / 2));
        Self {
            voices: Vec::new(),
            events: data.events.clone(),
            event_index: 0,
            sample: 0,
            loop_samples,
            rx,
            released: false,
            stopped: false,
            volume: song.volume.max(0.0),
            current_program: MidiProgram::default(),
        }
    }

    fn process_controls(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                MidiControl::Release => {
                    self.released = true;
                    for voice in &mut self.voices {
                        voice.release();
                    }
                }
                MidiControl::Stop => self.stopped = true,
            }
        }
    }

    fn process_events(&mut self) {
        while self
            .events
            .get(self.event_index)
            .is_some_and(|event| event.sample <= self.sample)
        {
            let event = self.events[self.event_index];
            match event.kind {
                MidiEventKind::NoteOn {
                    note,
                    velocity,
                    program,
                } => self
                    .voices
                    .push(Voice::new(note, velocity, self.volume, program)),
                MidiEventKind::NoteOff { note } => {
                    for voice in &mut self.voices {
                        if voice.note == note {
                            voice.release();
                        }
                    }
                }
                MidiEventKind::Program(program) => self.current_program = program,
            }
            self.event_index += 1;
        }
        if let Some(loop_samples) = self.loop_samples
            && self.sample >= loop_samples
        {
            self.sample = 0;
            self.event_index = 0;
            self.voices.clear();
        }
    }
}

impl Iterator for BuiltInMidiSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped {
            return None;
        }
        self.process_controls();
        self.process_events();
        let mut mixed = 0.0;
        let mut i = 0usize;
        while i < self.voices.len() {
            if let Some(sample) = self.voices[i].next_sample() {
                mixed += sample;
                i += 1;
            } else {
                self.voices.swap_remove(i);
            }
        }
        self.sample = self.sample.saturating_add(1);
        if self.released && self.voices.is_empty() {
            return None;
        }
        if self.events.is_empty() && self.voices.is_empty() {
            return None;
        }
        Some(mixed.clamp(-1.0, 1.0))
    }
}

impl Source for BuiltInMidiSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        self.loop_samples
            .map(|samples| Duration::from_secs_f64(samples as f64 / SAMPLE_RATE as f64))
    }
}

pub struct RustyNoteMixerSource {
    synth: Synthesizer,
    rx: Receiver<SoundFontMixerControl>,
    notes: HashMap<u64, (MidiChannel, Note)>,
    auto_releases: Vec<(u64, u64)>,
    sample: u64,
    stopped: bool,
    next_right: Option<f32>,
}

impl RustyNoteMixerSource {
    pub fn new(font: Arc<SoundFont>, rx: Receiver<SoundFontMixerControl>) -> Result<Self, String> {
        let settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        let synth = Synthesizer::new(&font, &settings).map_err(|err| err.to_string())?;
        Ok(Self {
            synth,
            rx,
            notes: HashMap::with_capacity(BUILT_IN_MIXER_MAX_VOICES),
            auto_releases: Vec::with_capacity(BUILT_IN_MIXER_MAX_VOICES),
            sample: 0,
            stopped: false,
            next_right: None,
        })
    }

    fn process_controls(&mut self) {
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                SoundFontMixerControl::Note(note) => self.start_note(note),
                SoundFontMixerControl::Release { id } => self.release_note(id),
                SoundFontMixerControl::Stop => self.stopped = true,
            }
        }
    }

    fn start_note(&mut self, note: SoundFontMixerNote) {
        self.synth
            .process_midi_message(note.channel.0 as i32, 0xC0, note.program.0 as i32, 0);
        self.synth.process_midi_message(
            note.channel.0 as i32,
            0x90,
            note.note.0 as i32,
            note.velocity as i32,
        );
        self.notes.insert(note.id, (note.channel, note.note));
        if !note.held {
            self.auto_releases
                .push((self.sample + duration_samples(note.sustain), note.id));
        }
    }

    fn release_note(&mut self, id: u64) {
        if let Some((channel, note)) = self.notes.remove(&id) {
            self.synth
                .process_midi_message(channel.0 as i32, 0x80, note.0 as i32, 0);
        }
    }

    fn process_auto_releases(&mut self) {
        let mut i = 0usize;
        while i < self.auto_releases.len() {
            if self.auto_releases[i].0 <= self.sample {
                let (_, id) = self.auto_releases.swap_remove(i);
                self.release_note(id);
            } else {
                i += 1;
            }
        }
    }
}

impl Iterator for RustyNoteMixerSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(right) = self.next_right.take() {
            return Some(right);
        }
        if self.stopped {
            return None;
        }
        self.process_controls();
        self.process_auto_releases();
        let mut left = [0.0f32];
        let mut right = [0.0f32];
        self.synth.render(&mut left, &mut right);
        self.sample = self.sample.saturating_add(1);
        self.next_right = Some(right[0]);
        Some(left[0])
    }
}

impl Source for RustyNoteMixerSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct RustyFileSource {
    sequencer: MidiFileSequencer,
    rx: Receiver<MidiControl>,
    looped: bool,
    stopped: bool,
    next_right: Option<f32>,
}

impl RustyFileSource {
    pub fn new(
        font: Arc<SoundFont>,
        bytes: &[u8],
        looped: bool,
        rx: Receiver<MidiControl>,
    ) -> Result<Self, String> {
        let settings = SynthesizerSettings::new(SAMPLE_RATE as i32);
        let synth = Synthesizer::new(&font, &settings).map_err(|err| err.to_string())?;
        let mut cursor = Cursor::new(bytes);
        let midi = Arc::new(RustyMidiFile::new(&mut cursor).map_err(|err| err.to_string())?);
        let mut sequencer = MidiFileSequencer::new(synth);
        sequencer.play(&midi, looped);
        Ok(Self {
            sequencer,
            rx,
            looped,
            stopped: false,
            next_right: None,
        })
    }
}

impl Iterator for RustyFileSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(right) = self.next_right.take() {
            return Some(right);
        }
        if self.stopped || (!self.looped && self.sequencer.end_of_sequence()) {
            return None;
        }
        while let Ok(cmd) = self.rx.try_recv() {
            match cmd {
                MidiControl::Release | MidiControl::Stop => self.stopped = true,
            }
        }
        let mut left = [0.0f32];
        let mut right = [0.0f32];
        self.sequencer.render(&mut left, &mut right);
        self.next_right = Some(right[0]);
        Some(left[0])
    }
}

impl Source for RustyFileSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        self.sequencer
            .get_midi_file()
            .map(|file| Duration::from_secs_f64(file.get_length()))
    }
}

fn duration_samples(duration: Duration) -> u64 {
    (duration.as_secs_f64() * SAMPLE_RATE as f64).max(0.0) as u64
}

#[doc(hidden)]
pub fn parse_built_in_midi_file(bytes: &[u8]) -> Result<Arc<BuiltInMidiFileData>, String> {
    let smf = Smf::parse(bytes).map_err(|err| err.to_string())?;
    let ticks_per_beat = match smf.header.timing {
        Timing::Metrical(ticks) => ticks.as_int().max(1) as f64,
        Timing::Timecode(_, ticks) => ticks as f64,
    };
    let mut events = Vec::<MidiEvent>::new();
    let mut end_sample = 0u64;
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut tempo_us = 500_000u64;
        let mut programs = [MidiProgram::default(); 16];
        for event in track {
            tick = tick.saturating_add(event.delta.as_int() as u64);
            if let TrackEventKind::Meta(MetaMessage::Tempo(tempo)) = event.kind {
                tempo_us = tempo.as_int() as u64;
                continue;
            }
            let seconds = (tick as f64 * tempo_us as f64) / (ticks_per_beat * 1_000_000.0);
            let sample = (seconds * SAMPLE_RATE as f64).max(0.0) as u64;
            end_sample = end_sample.max(sample);
            if let TrackEventKind::Midi { channel, message } = event.kind {
                let channel_index = channel.as_int() as usize;
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        events.push(MidiEvent {
                            sample,
                            kind: MidiEventKind::NoteOn {
                                note: Note::from_midi(key.as_int()),
                                velocity: vel.as_int(),
                                program: programs[channel_index],
                            },
                        });
                    }
                    MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                        events.push(MidiEvent {
                            sample,
                            kind: MidiEventKind::NoteOff {
                                note: Note::from_midi(key.as_int()),
                            },
                        });
                    }
                    MidiMessage::ProgramChange { program } => {
                        programs[channel_index] = MidiProgram::new(program.as_int());
                        events.push(MidiEvent {
                            sample,
                            kind: MidiEventKind::Program(programs[channel_index]),
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    events.sort_by_key(|event| event.sample);
    Ok(Arc::new(BuiltInMidiFileData {
        events: Arc::from(events.into_boxed_slice()),
        end_sample,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_frequency_maps_a4() {
        assert!((Note::A4.frequency_hz() - 440.0).abs() < 0.01);
    }

    #[test]
    fn grouped_program_maps_to_gm_number() {
        let program = program::Brass::Trumpet;
        assert_eq!(program.0, 56);
    }

    #[test]
    fn built_in_mixer_plays_many_notes_from_one_source() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut source = BuiltInMidiMixerSource::new(rx);
        for id in 0..128u64 {
            tx.send(MidiMixerControl::Note(MidiMixerNote {
                id,
                note: Note::from_midi(48 + (id % 24) as u8),
                velocity: 100,
                sustain: Duration::from_secs(1),
                held: true,
                program: MidiProgram::default(),
                volume: 1.0,
            }))
            .unwrap();
        }

        let mut peak = 0.0f32;
        for _ in 0..1024 {
            peak = peak.max(source.next().unwrap().abs());
        }

        assert_eq!(source.active_voice_count(), 128);
        assert!(peak > 0.0);
    }

    #[test]
    fn built_in_mixer_releases_note_by_id() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut source = BuiltInMidiMixerSource::new(rx);
        tx.send(MidiMixerControl::Note(MidiMixerNote {
            id: 7,
            note: Note::A4,
            velocity: 100,
            sustain: Duration::from_secs(10),
            held: true,
            program: MidiProgram::default(),
            volume: 1.0,
        }))
        .unwrap();
        assert!(source.next().is_some());
        tx.send(MidiMixerControl::Release { id: 7 }).unwrap();

        for _ in 0..SAMPLE_RATE {
            let _ = source.next();
        }

        assert_eq!(source.active_voice_count(), 0);
    }
}
