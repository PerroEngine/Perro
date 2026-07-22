use perro_api::prelude::*;

type SelfNodeType = Node3D;

// Floor slide half-range in world units.
const SLIDE_RANGE: f32 = 4.4;

#[State]
struct DecalsDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    // Emissive rune that slides across the floor.
    #[default = NodeID::nil()]
    pub slider: NodeID,
    // Hazard ring spinning about its projection axis.
    #[default = NodeID::nil()]
    pub spinner: NodeID,
    // Paint splat whose opacity pulses.
    #[default = NodeID::nil()]
    pub pulse: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.push_overlay(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let t = elapsed_time!(ctx.run);
        let (slider, spinner, pulse) = with_state!(ctx.run, DecalsDemoState, ctx.id, |state| {
            (state.slider, state.spinner, state.pulse)
        }).unwrap_or_default();

        if !slider.is_nil() {
            let x = (t * 0.7).sin() * SLIDE_RANGE;
            let _ = set_local_pos_3d!(ctx.run, slider, Vector3::new(x, 0.4, 4.6));
        }

        if !spinner.is_nil() {
            // Aim the projection straight down and roll it so the ring spins
            // in the floor plane (up vector sweeps around the horizon).
            let roll = t * 0.9;
            let rot = Quaternion::looking_at(
                Vector3::new(0.0, -1.0, 0.0),
                Vector3::new(roll.cos(), 0.0, roll.sin()),
            );
            let _ = set_local_rot_3d!(ctx.run, spinner, rot);
        }

        if !pulse.is_nil() {
            let alpha = 0.55 + 0.45 * (t * 1.6).sin().abs();
            with_node_mut!(ctx.run, Decal3D, pulse, |node| {
                node.modulate = Color::new(1.0, 1.0, 1.0, alpha);
            });
        }

        self.push_overlay(ctx);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, DecalsDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, DecalsDemoState, ctx.id, |state| state.overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let decals = query!(ctx.run, all(node_type[Decal3D]), in_subtree(ctx.id)).len();
        let body = format!(
            "decal projectors {}\nalbedo | normal | emission patches\nslide, spin, and opacity pulse are scripted",
            decals
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Decals".to_string(), body]
        );
    }
});
