use perro_api::prelude::*;
use std::time::Duration;

type SelfNodeType = Node3D;

#[State]
struct PositionalAudioDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = NodeID::nil()]
    pub speaker_a: NodeID,
    #[default = NodeID::nil()]
    pub speaker_b: NodeID,
    #[default = NodeID::nil()]
    pub speaker_c: NodeID,
    #[default = NodeID::nil()]
    pub audio_wall: NodeID,
    #[default = NodeID::nil()]
    pub debug_label: NodeID,
    #[default = vec![NodeID::nil(); 3]]
    pub speakers: Vec<NodeID>,
    #[default = 0.0]
    pub timer: f32,
    #[default = true]
    pub debug_rays: bool,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let (speakers, wall) = with_state!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            (
                vec![state.speaker_a, state.speaker_b, state.speaker_c],
                state.audio_wall,
            )
        });
        with_state_mut!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.overlay = NodeID::nil();
            state.speakers = speakers;
            state.debug_rays = true;
        });
        if !wall.is_nil() {
            with_node_mut!(ctx.run, AudioMask3D, wall, |wall| {
                wall.material.transmission = 0.18;
                wall.material.low_pass_strength = 0.65;
                wall.material.reflection = 0.22;
                wall.material.thickness_multiplier = 0.8;
            });
        }
        ctx.run.Audio().set_debug_rays(true);
        self.sync_label(ctx);
        self.push_overlay(ctx);
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
        self.push_overlay(ctx);
    }

    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {
        ctx.run.Audio().set_debug_rays(false);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

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
        let label = with_state!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
            state.debug_label
        });
        if label.is_nil() {
            return;
        }
        with_node_mut!(ctx.run, UiLabel, label, |label| {
            label.text = if debug {
                "Audio debug rays: ON  |  R toggles"
            } else {
                "Audio debug rays: OFF |  R toggles"
            }
            .into();
        });
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let (overlay, debug, speakers) =
            with_state!(ctx.run, PositionalAudioDemoState, ctx.id, |state| {
                (state.overlay, state.debug_rays, state.speakers.clone())
            });
        if overlay.is_nil() {
            return;
        }
        let ready = speakers.iter().filter(|id| !id.is_nil()).count();
        let zones = query!(
            ctx.run,
            all(node_type[AudioEffectZone3D]),
            in_subtree(ctx.id)
        )
        .len();
        let body = format!(
            "speakers {}\nfx zones {} | debug {}",
            ready,
            zones,
            if debug { "on" } else { "off" }
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Positional Audio".to_string(), body]
        );
    }
});
