use perro_api::prelude::*;

type SelfNodeType = Node3D;

const LIGHT_MARKERS: [&str; 10] = [
    "RedOrb",
    "BlueOrb",
    "GreenOrb",
    "AmberOrb",
    "VioletOrb",
    "CyanOrb",
    "PinkOrb",
    "LimeOrb",
    "WhiteSpotRig",
    "GoldSpotRig",
];

const SPEED: f32 = 0.78;

#[State]
struct LightsDemoState {
    #[default = NodeID::nil()]
    pub red: NodeID,
    #[default = NodeID::nil()]
    pub blue: NodeID,
    #[default = NodeID::nil()]
    pub green: NodeID,
    #[default = NodeID::nil()]
    pub amber: NodeID,
    #[default = NodeID::nil()]
    pub violet: NodeID,
    #[default = NodeID::nil()]
    pub cyan: NodeID,
    #[default = NodeID::nil()]
    pub pink: NodeID,
    #[default = NodeID::nil()]
    pub lime: NodeID,
    #[default = NodeID::nil()]
    pub white_spot: NodeID,
    #[default = NodeID::nil()]
    pub gold_spot: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let ids = LIGHT_MARKERS.map(|name| get_child!(ctx.run, ctx.id, name).unwrap_or(NodeID::nil()));
        with_state_mut!(ctx.run, LightsDemoState, ctx.id, |state| {
            state.red = ids[0];
            state.blue = ids[1];
            state.green = ids[2];
            state.amber = ids[3];
            state.violet = ids[4];
            state.cyan = ids[5];
            state.pink = ids[6];
            state.lime = ids[7];
            state.white_spot = ids[8];
            state.gold_spot = ids[9];
        });
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let t = elapsed_time!(ctx.run) * SPEED;
        let ids = with_state!(ctx.run, LightsDemoState, ctx.id, |state| {
            [
                state.red,
                state.blue,
                state.green,
                state.amber,
                state.violet,
                state.cyan,
                state.pink,
                state.lime,
                state.white_spot,
                state.gold_spot,
            ]
        });

        for (i, id) in ids.into_iter().enumerate() {
            if id.is_nil() {
                continue;
            }
            let phase = t + i as f32 * 0.73;
            let pos = light_pos(i, phase);
            let rot = light_rot(i, phase, pos);
            let _ = set_local_pos_3d!(ctx.run, id, pos);
            let _ = set_local_rot_3d!(ctx.run, id, rot);
        }
    }
});

methods!({});

fn light_pos(index: usize, phase: f32) -> Vector3 {
    match index {
        0 => orbit(phase, 5.2, 2.6, 0.0),
        1 => orbit(-phase * 1.15, 4.4, 1.8, 1.8),
        2 => Vector3::new(phase.sin() * 4.7, 3.3 + (phase * 1.7).sin() * 1.1, phase.cos() * 1.7),
        3 => Vector3::new((phase * 1.3).sin() * 1.4, 0.55, (phase * 0.8).cos() * 4.8),
        4 => Vector3::new(-4.2, 2.3 + phase.sin() * 1.7, (phase * 1.6).cos() * 3.3),
        5 => Vector3::new(4.2, 2.0 + (phase * 1.2).cos() * 1.4, (phase * 1.5).sin() * 3.0),
        6 => Vector3::new((phase * 0.65).sin() * 5.4, 4.7, (phase * 1.4).sin() * 2.5),
        7 => Vector3::new((phase * 1.8).sin() * 2.6, 1.1 + (phase * 2.2).sin() * 0.55, (phase * 0.9).cos() * 5.0),
        8 => Vector3::new((phase * 0.7).sin() * 3.2, 5.3, -4.2 + phase.cos() * 0.8),
        _ => Vector3::new((phase * 0.9).cos() * 4.1, 3.8 + phase.sin() * 0.8, 4.1),
    }
}

fn orbit(phase: f32, radius: f32, height: f32, y_sway: f32) -> Vector3 {
    Vector3::new(
        phase.cos() * radius,
        height + (phase * 1.9).sin() * y_sway,
        phase.sin() * radius,
    )
}

fn light_rot(index: usize, phase: f32, pos: Vector3) -> Quaternion {
    if index >= 8 {
        return Quaternion::looking_at(Vector3::new(-pos.x, 0.8 - pos.y, -pos.z), Vector3::new(0.0, 1.0, 0.0));
    }
    Quaternion::from_euler_xyz(phase * 0.6, phase * 0.9, phase * 0.35)
}
