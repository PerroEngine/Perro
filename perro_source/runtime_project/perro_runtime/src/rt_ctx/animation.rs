use crate::Runtime;
use perro_animation::AnimationNodeBinding;
use perro_nodes::AnimationPlayer;
use perro_runtime_context::sub_apis::{AnimationAPI, NodeAPI};

impl AnimationAPI for Runtime {
    fn animation_set_clip(&mut self, player: perro_ids::NodeID, animation: perro_ids::AnimationID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.animation = animation;
            node.current_frame = 0;
            node.current_time = 0.0;
        })
        .is_some()
    }

    fn animation_play(&mut self, player: perro_ids::NodeID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.playing = true;
            node.paused = false;
        })
        .is_some()
    }

    fn animation_pause(&mut self, player: perro_ids::NodeID, paused: bool) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.paused = paused;
        })
        .is_some()
    }

    fn animation_seek_time(&mut self, player: perro_ids::NodeID, time_seconds: f32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.current_time = time_seconds.max(0.0);
        })
        .is_some()
    }

    fn animation_seek_frame(&mut self, player: perro_ids::NodeID, frame: u32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.current_frame = frame;
        })
        .is_some()
    }

    fn animation_set_speed(&mut self, player: perro_ids::NodeID, speed: f32) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |node| {
            node.speed = speed;
        })
        .is_some()
    }

    fn animation_bind(&mut self, player: perro_ids::NodeID, track: &str, node: perro_ids::NodeID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |anim| {
            if let Some(binding) = anim.runtime_bindings.iter_mut().find(|b| b.track.as_ref() == track) {
                binding.node = node;
            } else {
                anim.runtime_bindings.push(AnimationNodeBinding {
                    track: track.to_string().into(),
                    node,
                });
            }
        })
        .is_some()
    }

    fn animation_clear_bindings(&mut self, player: perro_ids::NodeID) -> bool {
        self.with_node_mut::<AnimationPlayer, _, _>(player, |anim| {
            anim.runtime_bindings.clear();
        })
        .is_some()
    }
}
