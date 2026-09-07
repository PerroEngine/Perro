//! One phase order for normal and profiled runtime execution.
use super::{
    Runtime, RuntimeFixedUpdateTiming, RuntimePhysicsStepTiming, RuntimeUpdateTiming,
    UpdateScheduleTiming,
};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

// Const specialization removes clocks and result bookkeeping from normal frames.
#[inline]
fn start<const TIMED: bool>() -> Option<Instant> {
    if TIMED { Some(Instant::now()) } else { None }
}
#[inline]
fn elapsed(clock: Option<Instant>) -> Duration {
    clock.map_or(Duration::ZERO, |clock| clock.elapsed())
}

impl Runtime {
    #[inline]
    pub fn update(&mut self, delta_time: f32) {
        self.update_phases::<false>(delta_time);
    }
    #[inline]
    pub fn update_timed(&mut self, delta_time: f32) -> RuntimeUpdateTiming {
        self.update_phases::<true>(delta_time)
    }
    #[inline(always)]
    fn update_phases<const TIMED: bool>(&mut self, delta_time: f32) -> RuntimeUpdateTiming {
        let total = start::<TIMED>();
        self.clear_startup_keyboard_mouse();
        self.time.delta = delta_time;
        self.advance_timers(delta_time);
        self.flush_queued_ui_signals();
        self.process_pending_web_route_change();
        self.apply_loaded_skeleton_bones();
        self.poll_async_scene_preloads();
        let clock = start::<TIMED>();
        self.run_start_schedule();
        let start_schedule = elapsed(clock);
        let clock = start::<TIMED>();
        self.schedules.snapshot_update(&self.scripts);
        let snapshot_update = elapsed(clock);
        let update_schedule = if TIMED {
            self.run_update_schedule_timed()
        } else {
            self.run_update_schedule();
            UpdateScheduleTiming::default()
        };
        #[cfg(feature = "steamworks")]
        let _ = perro_steamworks::runtime::run_callbacks();
        let clock = start::<TIMED>();
        self.run_internal_update_schedule();
        let internal_update = elapsed(clock);
        self.nodes.refresh_packed_children();
        self.propagate_pending_transform_dirty();
        self.update_audio_propagation(delta_time);
        RuntimeUpdateTiming {
            start_schedule,
            snapshot_update,
            update_schedule,
            internal_update,
            total: elapsed(total),
        }
    }

    #[inline]
    pub fn fixed_update(&mut self, delta: f32) {
        self.fixed_phases::<false>(delta);
    }
    #[inline]
    pub fn fixed_update_timed(&mut self, delta: f32) -> RuntimeFixedUpdateTiming {
        self.fixed_phases::<true>(delta)
    }
    #[inline(always)]
    fn fixed_phases<const TIMED: bool>(&mut self, delta: f32) -> RuntimeFixedUpdateTiming {
        let total = start::<TIMED>();
        self.clear_startup_keyboard_mouse();
        self.time.fixed_delta = delta;
        let clock = start::<TIMED>();
        self.schedules.snapshot_fixed(&self.scripts);
        let snapshot_update = elapsed(clock);
        let clock = start::<TIMED>();
        self.run_fixed_schedule();
        let script_fixed_update = elapsed(clock);
        self.nodes.refresh_packed_children();
        let physics = if TIMED {
            self.physics_fixed_step_timed()
        } else {
            self.physics_fixed_step();
            RuntimePhysicsStepTiming::default()
        };
        let clock = start::<TIMED>();
        self.run_internal_fixed_update_schedule();
        let internal_fixed_update = elapsed(clock);
        self.nodes.refresh_packed_children();
        self.propagate_pending_transform_dirty();
        RuntimeFixedUpdateTiming {
            snapshot_update,
            script_fixed_update,
            physics: physics.total,
            physics_pre_transforms: physics.pre_transforms,
            physics_collect: physics.collect,
            physics_sync_world: physics.sync_world,
            physics_apply_forces_impulses: physics.apply_forces_impulses,
            physics_step: physics.step,
            physics_sync_nodes: physics.sync_nodes,
            physics_post_transforms: physics.post_transforms,
            physics_signals: physics.signals,
            internal_fixed_update,
            total: elapsed(total),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::NodeID;
    use perro_nodes::{AnimatedSprite, AnimatedSprite2D, IKTarget2D};
    use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec};

    fn sprite() -> AnimatedSprite2D {
        let mut sprite = AnimatedSprite2D::new();
        let mut animation = AnimatedSprite::new("test");
        animation.frame_count = 8;
        animation.fps = 8.0;
        sprite.animations.push(animation);
        sprite
    }

    #[test]
    fn normal_and_timed_phases_produce_same_world_and_draw_commands() {
        let mut normal = Runtime::new();
        let mut timed = Runtime::new();
        let a = NodeAPI::create_nodes(&mut normal, &[NodeSpec::new(sprite())], NodeID::nil())[0];
        let b = NodeAPI::create_nodes(&mut timed, &[NodeSpec::new(sprite())], NodeID::nil())[0];
        let mut normal_commands = Vec::new();
        let mut timed_commands = Vec::new();
        for _ in 0..16 {
            normal.fixed_update(0.125);
            timed.fixed_update_timed(0.125);
            normal.update(0.125);
            timed.update_timed(0.125);
            assert_eq!(
                format!("{:?}", normal.nodes.get(a)),
                format!("{:?}", timed.nodes.get(b))
            );
            normal.extract_render_2d_commands();
            timed.extract_render_2d_commands();
            normal.drain_render_commands(&mut normal_commands);
            timed.drain_render_commands(&mut timed_commands);
            assert_eq!(
                format!("{normal_commands:?}"),
                format!("{timed_commands:?}")
            );
            normal_commands.clear();
            timed_commands.clear();
        }
    }

    struct ResetFrame;
    impl perro_scripting::ScriptLifecycle<super::super::RuntimeScriptApi> for ResetFrame {
        fn on_update(
            &self,
            ctx: &mut perro_scripting::ScriptContext<'_, super::super::RuntimeScriptApi>,
        ) {
            ctx.run
                .Nodes()
                .with_node_mut::<AnimatedSprite2D, _, _>(ctx.id, |node| node.current_frame = 3);
        }
    }
    impl perro_scripting::ScriptBehavior<super::super::RuntimeScriptApi> for ResetFrame {
        fn script_flags(&self) -> perro_scripting::ScriptFlags {
            perro_scripting::ScriptFlags::new(perro_scripting::ScriptFlags::HAS_UPDATE)
        }
        fn get_var(
            &self,
            _: &dyn std::any::Any,
            _: perro_ids::ScriptMemberID,
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
        }
        fn set_var(
            &self,
            _: &mut dyn std::any::Any,
            _: perro_ids::ScriptMemberID,
            _: perro_variant::Variant,
        ) {
        }
        fn call_method(
            &self,
            _: perro_ids::ScriptMemberID,
            _: &mut perro_scripting::ScriptContext<'_, super::super::RuntimeScriptApi>,
            _: &[perro_variant::Variant],
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
        }
    }

    #[test]
    fn script_runs_before_builtin_behavior_in_both_modes() {
        for timed in [false, true] {
            let mut runtime = Runtime::new();
            let id =
                NodeAPI::create_nodes(&mut runtime, &[NodeSpec::new(sprite())], NodeID::nil())[0];
            runtime
                .scripts
                .insert(id, std::rc::Rc::new(ResetFrame), Box::new(()));
            if timed {
                runtime.update_timed(0.125);
            } else {
                runtime.update(0.125);
            }
            assert_eq!(
                runtime.with_node::<AnimatedSprite2D, _>(id, |node| node.current_frame),
                Some(4)
            );
        }
    }

    #[test]
    fn dispatch_observes_public_node_type_replacement() {
        let mut runtime = Runtime::new();
        let id = NodeAPI::create::<IKTarget2D>(&mut runtime);
        runtime.update(0.125); // Populate dispatch snapshot.
        runtime.nodes.get_mut(id).expect("live node").data = sprite().into();
        runtime.update(0.125);
        let frame = runtime.with_node::<AnimatedSprite2D, _>(id, |node| node.current_frame);
        assert_eq!(frame, Some(1));
        NodeAPI::remove_node(&mut runtime, id);
        runtime.update(0.125); // Stale snapshot entries must not dispatch.
    }
}
