use crate::prelude::*;
use perro_nodes::ParticleEmitter3D;

type SelfNodeType = ParticleEmitter3D;

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    _res_w: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let delta_seconds = delta_time!(ctx);
    let mut emit_finished_signal = false;
    let _ = with_node_mut!(ctx, SelfNodeType, id, |emitter| {
        emit_finished_signal = internal_step_update(emitter, delta_seconds);
    });
    if emit_finished_signal && let Some(node_name) = get_node_name!(ctx, id) {
        let signal_name = format!("{}_PARTICLES_FINISHED", node_name);

        let signal_id = SignalID::from_string(&signal_name);
        let _ = signal_emit!(ctx, signal_id);
    }
}

pub fn internal_fixed_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    _res_w: &ResourceWindow<'_, R>,
    _ipt_w: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let fixed_delta = fixed_delta_time!(ctx);
    let _ = with_node_mut!(ctx, SelfNodeType, id, |emitter| {
        internal_step_fixed_update(emitter, fixed_delta);
    });
}

fn internal_step_update(emitter: &mut ParticleEmitter3D, delta_seconds: f32) -> bool {
    let mut emit_finished_signal = false;
    if emitter.active && !emitter.internal_prev_active {
        emitter.internal_simulation_time = 0.0;
        emitter.internal_finished_emitted = false;
    }

    if emitter.active {
        emitter.internal_simulation_time += delta_seconds.max(0.0);
        if emitter.looping {
            emitter.internal_finished_emitted = false;
        } else if let Some(done_after) =
            non_looping_done_after(emitter, emitter.internal_lifetime_max.max(0.001))
            && emitter.internal_simulation_time > done_after
        {
            emitter.active = false;
            if !emitter.internal_finished_emitted {
                emit_finished_signal = true;
            }
            emitter.internal_finished_emitted = true;
        }
    }

    emitter.internal_prev_active = emitter.active;
    emit_finished_signal
}

fn internal_step_fixed_update(_emitter: &mut ParticleEmitter3D, _fixed_delta_seconds: f32) {}

fn non_looping_done_after(emitter: &ParticleEmitter3D, lifetime_max: f32) -> Option<f32> {
    if emitter.spawn_rate <= 0.0 || lifetime_max <= 0.0 {
        return None;
    }
    let budget = ((emitter.spawn_rate * lifetime_max).ceil() as u32 + 2).clamp(1, 1_000_000);
    let last_spawn_t = (budget.saturating_sub(1) as f32) / emitter.spawn_rate.max(1.0e-6);
    Some(last_spawn_t + lifetime_max)
}
