use perro_ids::{AnimationID, NodeID};
use perro_nodes::animation_player::AnimationPlaybackType;

pub enum AnimTreeSlotArg<'a> {
    Name(&'a str),
    Index(usize),
}

pub trait IntoAnimTreeSlotArg<'a> {
    fn into_slot_arg(self) -> AnimTreeSlotArg<'a>;
}

impl<'a> IntoAnimTreeSlotArg<'a> for &'a str {
    fn into_slot_arg(self) -> AnimTreeSlotArg<'a> {
        AnimTreeSlotArg::Name(self)
    }
}

impl<'a> IntoAnimTreeSlotArg<'a> for usize {
    fn into_slot_arg(self) -> AnimTreeSlotArg<'a> {
        AnimTreeSlotArg::Index(self)
    }
}

pub trait AnimTreeAPI {
    fn animation_tree_set_clip_by_name(
        &mut self,
        tree: NodeID,
        slot: &str,
        animation: AnimationID,
    ) -> bool;
    fn animation_tree_set_clip_by_index(
        &mut self,
        tree: NodeID,
        slot: usize,
        animation: AnimationID,
    ) -> bool;
    fn animation_tree_play_slot(&mut self, tree: NodeID, slot: &str) -> bool;
    fn animation_tree_pause_slot(&mut self, tree: NodeID, slot: &str, paused: bool) -> bool;
    fn animation_tree_seek_slot_frame(&mut self, tree: NodeID, slot: &str, frame: u32) -> bool;
    fn animation_tree_set_slot_speed(&mut self, tree: NodeID, slot: &str, speed: f32) -> bool;
    fn animation_tree_set_slot_playback(
        &mut self,
        tree: NodeID,
        slot: &str,
        playback_type: AnimationPlaybackType,
    ) -> bool;
    fn animation_tree_seek_node_time(&mut self, tree: NodeID, node: &str, seconds: f32) -> bool;
    fn animation_tree_set_weight(
        &mut self,
        tree: NodeID,
        node: &str,
        input: &str,
        weight: f32,
    ) -> bool;
    fn animation_tree_pause(&mut self, tree: NodeID, paused: bool) -> bool;
}

pub struct AnimTreeModule<'rt, R: AnimTreeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: AnimTreeAPI + ?Sized> AnimTreeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn set_clip<'a, S: IntoAnimTreeSlotArg<'a>>(
        &mut self,
        tree: NodeID,
        slot: S,
        animation: AnimationID,
    ) -> bool {
        match slot.into_slot_arg() {
            AnimTreeSlotArg::Name(slot) => self
                .rt
                .animation_tree_set_clip_by_name(tree, slot, animation),
            AnimTreeSlotArg::Index(slot) => self
                .rt
                .animation_tree_set_clip_by_index(tree, slot, animation),
        }
    }

    pub fn play_slot(&mut self, tree: NodeID, slot: &str) -> bool {
        self.rt.animation_tree_play_slot(tree, slot)
    }

    pub fn pause_slot(&mut self, tree: NodeID, slot: &str, paused: bool) -> bool {
        self.rt.animation_tree_pause_slot(tree, slot, paused)
    }

    pub fn seek_slot_frame(&mut self, tree: NodeID, slot: &str, frame: u32) -> bool {
        self.rt.animation_tree_seek_slot_frame(tree, slot, frame)
    }

    pub fn set_slot_speed(&mut self, tree: NodeID, slot: &str, speed: f32) -> bool {
        self.rt.animation_tree_set_slot_speed(tree, slot, speed)
    }

    pub fn set_slot_playback(
        &mut self,
        tree: NodeID,
        slot: &str,
        playback_type: AnimationPlaybackType,
    ) -> bool {
        self.rt
            .animation_tree_set_slot_playback(tree, slot, playback_type)
    }

    pub fn seek_node_time(&mut self, tree: NodeID, node: &str, seconds: f32) -> bool {
        self.rt.animation_tree_seek_node_time(tree, node, seconds)
    }

    pub fn set_weight(&mut self, tree: NodeID, node: &str, input: &str, weight: f32) -> bool {
        self.rt.animation_tree_set_weight(tree, node, input, weight)
    }

    pub fn pause(&mut self, tree: NodeID, paused: bool) -> bool {
        self.rt.animation_tree_pause(tree, paused)
    }
}

#[macro_export]
macro_rules! anim_tree_set_clip {
    ($ctx:expr, $tree:expr, $slot:expr, $animation:expr) => {
        $ctx.AnimTree().set_clip($tree, $slot, $animation)
    };
}

#[macro_export]
macro_rules! anim_tree_play_slot {
    ($ctx:expr, $tree:expr, $slot:expr) => {
        $ctx.AnimTree().play_slot($tree, $slot)
    };
}

#[macro_export]
macro_rules! anim_tree_pause_slot {
    ($ctx:expr, $tree:expr, $slot:expr, $paused:expr) => {
        $ctx.AnimTree().pause_slot($tree, $slot, $paused)
    };
}

#[macro_export]
macro_rules! anim_tree_seek_slot_frame {
    ($ctx:expr, $tree:expr, $slot:expr, $frame:expr) => {
        $ctx.AnimTree().seek_slot_frame($tree, $slot, $frame)
    };
}

#[macro_export]
macro_rules! anim_tree_set_slot_speed {
    ($ctx:expr, $tree:expr, $slot:expr, $speed:expr) => {
        $ctx.AnimTree().set_slot_speed($tree, $slot, $speed)
    };
}

#[macro_export]
macro_rules! anim_tree_set_slot_playback {
    ($ctx:expr, $tree:expr, $slot:expr, $playback:expr) => {
        $ctx.AnimTree().set_slot_playback($tree, $slot, $playback)
    };
}

#[macro_export]
macro_rules! anim_tree_seek_node_time {
    ($ctx:expr, $tree:expr, $node:expr, $seconds:expr) => {
        $ctx.AnimTree().seek_node_time($tree, $node, $seconds)
    };
}

#[macro_export]
macro_rules! anim_tree_set_weight {
    ($ctx:expr, $tree:expr, $node:expr, $input:expr, $weight:expr) => {
        $ctx.AnimTree().set_weight($tree, $node, $input, $weight)
    };
}

#[macro_export]
macro_rules! anim_tree_pause {
    ($ctx:expr, $tree:expr, $paused:expr) => {
        $ctx.AnimTree().pause($tree, $paused)
    };
}
