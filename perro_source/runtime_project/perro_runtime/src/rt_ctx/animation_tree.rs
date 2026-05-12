use crate::Runtime;
use perro_nodes::AnimationTree;
use perro_nodes::animation_player::AnimationPlaybackType;
use perro_runtime_api::sub_apis::{AnimTreeAPI, NodeAPI};

impl AnimTreeAPI for Runtime {
    fn animation_tree_set_clip_by_name(
        &mut self,
        tree: perro_ids::NodeID,
        slot: &str,
        animation: perro_ids::AnimationID,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.set_clip_by_name(slot, animation)
        })
        .unwrap_or(false)
    }

    fn animation_tree_set_clip_by_index(
        &mut self,
        tree: perro_ids::NodeID,
        slot: usize,
        animation: perro_ids::AnimationID,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.set_clip_by_index(slot, animation)
        })
        .unwrap_or(false)
    }

    fn animation_tree_play_slot(&mut self, tree: perro_ids::NodeID, slot: &str) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| node.play_slot(slot))
            .unwrap_or(false)
    }

    fn animation_tree_pause_slot(
        &mut self,
        tree: perro_ids::NodeID,
        slot: &str,
        paused: bool,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| node.pause_slot(slot, paused))
            .unwrap_or(false)
    }

    fn animation_tree_seek_slot_frame(
        &mut self,
        tree: perro_ids::NodeID,
        slot: &str,
        frame: u32,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| node.seek_slot_frame(slot, frame))
            .unwrap_or(false)
    }

    fn animation_tree_set_slot_speed(
        &mut self,
        tree: perro_ids::NodeID,
        slot: &str,
        speed: f32,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| node.set_slot_speed(slot, speed))
            .unwrap_or(false)
    }

    fn animation_tree_set_slot_playback(
        &mut self,
        tree: perro_ids::NodeID,
        slot: &str,
        playback_type: AnimationPlaybackType,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.set_slot_playback_type(slot, playback_type)
        })
        .unwrap_or(false)
    }

    fn animation_tree_seek_node_time(
        &mut self,
        tree: perro_ids::NodeID,
        node_name: &str,
        seconds: f32,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            let frame = seconds.max(0.0).floor() as u32;
            node.seek_slot_frame(node_name, frame)
        })
        .unwrap_or(false)
    }

    fn animation_tree_set_weight(
        &mut self,
        tree: perro_ids::NodeID,
        node_name: &str,
        input: &str,
        weight: f32,
    ) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.set_weight(node_name, input, weight)
        })
        .unwrap_or(false)
    }

    fn animation_tree_pause(&mut self, tree: perro_ids::NodeID, paused: bool) -> bool {
        self.with_node_mut::<AnimationTree, _, _>(tree, |node| {
            node.paused = paused;
        })
        .is_some()
    }
}
