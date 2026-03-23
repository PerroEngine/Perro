use crate::Runtime;
use perro_nodes::AnimationPlayer;
use perro_runtime_context::sub_apis::{AnimPlayerAPI, NodeAPI};

impl AnimPlayerAPI for Runtime {
    fn animation_set_clip(
        &mut self,
        player: perro_ids::NodeID,
        animation: perro_ids::AnimationID,
    ) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.set_animation(animation);
        })
        .is_some()
    }

    fn animation_play(&mut self, player: perro_ids::NodeID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.play();
        })
        .is_some()
    }

    fn animation_pause(&mut self, player: perro_ids::NodeID, paused: bool) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.pause(paused);
        })
        .is_some()
    }

    fn animation_seek_time(&mut self, player: perro_ids::NodeID, time_seconds: f32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.set_current_time(time_seconds);
        })
        .is_some()
    }

    fn animation_seek_frame(&mut self, player: perro_ids::NodeID, frame: u32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.set_current_frame(frame);
        })
        .is_some()
    }

    fn animation_set_speed(&mut self, player: perro_ids::NodeID, speed: f32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.set_speed(speed);
        })
        .is_some()
    }

    fn animation_bind(
        &mut self,
        player: perro_ids::NodeID,
        track: &str,
        node: perro_ids::NodeID,
    ) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |anim| {
            anim.set_binding(track, node);
        })
        .is_some()
    }

    fn animation_clear_bindings(&mut self, player: perro_ids::NodeID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |anim| {
            anim.clear_bindings();
        })
        .is_some()
    }
}
