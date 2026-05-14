use perro_api::prelude::*;
use std::time::Duration;

type SelfNodeType = Node3D;

const SPEAKER_A: &str = "SpeakerA";
const SPEAKER_B: &str = "SpeakerB";
const SPEAKER_C: &str = "SpeakerC";
const AUDIO_WALL: &str = "AudioWall";
const DEBUG_LABEL: &str = "AudioDebugLabel";

#[State]
struct PositionalAudioDemoState {
    #[default = vec![NodeID::nil(); 3]]
    pub speakers: Vec<NodeID>,
    #[default = 0.0]
    pub timer: f32,
    #[default = true]
    pub debug_rays: bool,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let speakers = vec![
            get_child!(ctx.run, ctx.id, SPEAKER_A).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, ctx.id, SPEAKER_B).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, ctx.id, SPEAKER_C).unwrap_or(NodeID::nil()),
        ];
        with_state_mut!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.speakers = speakers;
            state.debug_rays = true;
        });
        if let Some(wall) = get_child!(ctx.run, ctx.id, AUDIO_WALL) {
            with_node_mut!(ctx.run, AudioMask3D, wall, |wall| {
                wall.material.transmission = 0.18;
                wall.material.low_pass_strength = 0.65;
                wall.material.reflection = 0.22;
                wall.material.thickness_multiplier = 0.8;
            });
        }
        ctx.run.Audio().set_debug_rays(true);
        self.sync_label(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        if key_pressed!(ctx.ipt, KeyCode::KeyR) {
            let debug = with_state_mut!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
                state.debug_rays = !state.debug_rays;
                state.debug_rays
            })
            .unwrap_or(false);
            ctx.run.Audio().set_debug_rays(debug);
            self.sync_label(ctx);
        }

        let dt = delta_time!(ctx.run);
        let play = with_state_mut!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.timer -= dt;
            if state.timer <= 0.0 {
                state.timer = 0.34;
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if play {
            self.play_chord(ctx);
        }
    }

    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {
        ctx.run.Audio().set_debug_rays(false);
    }
});

methods!({
    fn play_chord(&self, ctx: &mut ScriptContext<'_, API>) {
        let speakers = with_state!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.speakers.clone()
        });
        let spatial = SpatialAudioOptions {
            range: 38.0,
            audio_layer: BitMask::ALL,
            enable_propagation: true,
            direction_2d: AudioDirection::Omni,
            direction_3d: AudioDirection::Omni,
        };
        let base = MidiNoteOptions {
            velocity: 78,
            sustain: Duration::from_millis(520),
            program: program::Piano::Electric1,
            volume: 0.45,
            ..MidiNoteOptions::default()
        };

        for (speaker, note, velocity) in [
            (speakers[0], Note::C4, 64),
            (speakers[1], Note::E4, 72),
            (speakers[2], Note::G4, 80),
        ] {
            if speaker.is_nil() {
                continue;
            }
            let opts = MidiNoteOptions { velocity, ..base };
            let _ = ctx
                .run
                .Audio()
                .midi()
                .play_note_attached(note, speaker, opts, spatial);
        }
    }

    fn sync_label(&self, ctx: &mut ScriptContext<'_, API>) {
        let debug = with_state!(ctx.run, PositionalAudioDemoState, ctx.id, |state| state
            .debug_rays);
        let Some(label) = get_child!(ctx.run, ctx.id, DEBUG_LABEL) else {
            return;
        };
        with_node_mut!(ctx.run, UiLabel, label, |label| {
            label.text = if debug {
                "Audio debug rays: ON  |  R toggles"
            } else {
                "Audio debug rays: OFF |  R toggles"
            }
            .into();
        });
    }
});
