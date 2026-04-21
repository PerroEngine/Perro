use perro::prelude::*;

type SelfNodeType = MeshInstance3D;

const CHAIN_DEPTH: usize = 100;
const LERP_SECONDS: f32 = 3.0;
const TARGET_RADIUS: f32 = 12.0;
const MAX_DT: f32 = 1.0 / 20.0;

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32) -> f32 {
    hash_u32(seed) as f32 / u32::MAX as f32
}

fn rand11(seed: u32) -> f32 {
    rand01(seed) * 2.0 - 1.0
}

fn next_target(seed: u32) -> Vector3 {
    let dir = Vector3::new(
        rand11(seed.wrapping_add(0x9E37_79B9)),
        rand11(seed.wrapping_add(0x243F_6A88)),
        rand11(seed.wrapping_add(0xB7E1_5162)),
    )
    .normalized();

    let radius = TARGET_RADIUS * (0.25 + rand01(seed.wrapping_add(0xDEAD_BEEF)) * 0.75);
    dir * radius
}

#[State]
pub struct InstanceState {
    #[default = Vector3::ZERO]
    from: Vector3,
    #[default = Vector3::ZERO]
    to: Vector3,
    #[default = 0.0]
    timer: f32,
    #[default = 1]
    seed: u32,
}

lifecycle!({
    fn on_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let mesh_id = res.Meshes().load("__cube__");
        let _ = with_node_mut!(ctx, SelfNodeType, self_id, |node| {
            node.mesh = mesh_id;
            node.scale = Vector3::new(0.7, 0.7, 0.7);
        });

        let mut parent = self_id;
        for depth in 0..CHAIN_DEPTH {
            let child = create_node!(ctx, MeshInstance3D, format!("chain_{depth}"));
            let _ = reparent!(ctx, parent, child);
            let _ = with_node_mut!(ctx, MeshInstance3D, child, |node| {
                node.mesh = mesh_id;
                node.position = Vector3::new(0.8, 0.5, 0.8);
                node.scale = Vector3::new(0.995, 0.995, 0.995);
            });
            parent = child;
        }

        let start = with_node!(ctx, SelfNodeType, self_id, |node| { node.position });
        let seed = hash_u32((self_id.as_u64() as u32).wrapping_add(0xA341_316C));
        let target = next_target(seed);
        let _ = with_state_mut!(ctx, InstanceState, self_id, |state| {
            state.from = start;
            state.to = target;
            state.timer = 0.0;
            state.seed = seed;
        });
    }

    fn on_all_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_update(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let dt = delta_time_capped!(ctx, MAX_DT);
        let (from, to, timer) = with_state_mut!(ctx, InstanceState, self_id, |state| {
            state.timer += dt;
            if state.timer >= LERP_SECONDS {
                state.timer -= LERP_SECONDS;
                state.from = state.to;
                state.seed = hash_u32(state.seed.wrapping_add(0x9E37_79B9));
                state.to = next_target(state.seed);
            }
            (state.from, state.to, state.timer)
        })
        .unwrap_or((Vector3::ZERO, Vector3::ZERO, 0.0));

        let t = (timer / LERP_SECONDS).clamp(0.0, 1.0);
        let _ = with_node_mut!(ctx, SelfNodeType, self_id, |node| {
            node.position = from.lerped(to, t);
        });
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});

methods!({
    fn default_method(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {}
});
