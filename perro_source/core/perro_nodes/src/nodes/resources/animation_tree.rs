use perro_ids::{AnimationID, AnimationTreeID};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct AnimationTreeAnimation {
    pub animation: AnimationID,
    pub bindings: Vec<super::animation_player::AnimationObjectBinding>,
    pub speed: f32,
    pub paused: bool,
    pub playback_type: super::animation_player::AnimationPlaybackType,
}

impl Default for AnimationTreeAnimation {
    fn default() -> Self {
        default_tree_animation()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeSlotPlayback {
    pub name: Cow<'static, str>,
    pub current_frame: u32,
    pub playback_frame: f32,
    pub boomerang_direction: f32,
    pub paused: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeRuntimeWeight {
    pub node: Cow<'static, str>,
    pub input: Cow<'static, str>,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeInternalData {
    pub last_tree: AnimationTreeID,
    pub last_binding_hash: u64,
    pub last_pose_hash: u64,
    pub slots: Vec<AnimationTreeSlotPlayback>,
    pub weights: Vec<AnimationTreeRuntimeWeight>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTree {
    pub tree: AnimationTreeID,
    pub animations: Vec<AnimationTreeAnimation>,
    pub speed: f32,
    pub paused: bool,
    pub internal: AnimationTreeInternalData,
}

impl AnimationTree {
    pub fn new() -> Self {
        Self {
            speed: 1.0,
            ..Self::default()
        }
    }

    pub fn set_tree(&mut self, tree: AnimationTreeID) {
        self.tree = tree;
        self.internal.last_tree = AnimationTreeID::nil();
    }

    pub fn set_clip_by_name(&mut self, slot: &str, animation: AnimationID) -> bool {
        let Some(index) = self.slot_index(slot) else {
            return false;
        };
        self.set_clip_by_index(index, animation)
    }

    pub fn set_clip_by_index(&mut self, slot: usize, animation: AnimationID) -> bool {
        self.resize_animations(slot + 1);
        self.animations[slot].animation = animation;
        if let Some(state) = self.internal.slots.get_mut(slot) {
            state.current_frame = 0;
            state.playback_frame = 0.0;
            state.boomerang_direction = 1.0;
        }
        true
    }

    pub fn play_slot(&mut self, slot: &str) -> bool {
        if let Some(state) = self
            .internal
            .slots
            .iter_mut()
            .find(|s| s.name.as_ref() == slot)
        {
            state.paused = false;
            return true;
        }
        false
    }

    pub fn pause_slot(&mut self, slot: &str, paused: bool) -> bool {
        if let Some(state) = self
            .internal
            .slots
            .iter_mut()
            .find(|s| s.name.as_ref() == slot)
        {
            state.paused = paused;
            return true;
        }
        false
    }

    pub fn seek_slot_frame(&mut self, slot: &str, frame: u32) -> bool {
        if let Some(state) = self
            .internal
            .slots
            .iter_mut()
            .find(|s| s.name.as_ref() == slot)
        {
            state.current_frame = frame;
            state.playback_frame = frame as f32;
            state.boomerang_direction = 1.0;
            return true;
        }
        false
    }

    pub fn set_slot_speed(&mut self, slot: &str, speed: f32) -> bool {
        let Some(index) = self.slot_index(slot) else {
            return false;
        };
        self.resize_animations(index + 1);
        self.animations[index].speed = speed;
        true
    }

    pub fn set_slot_playback_type(
        &mut self,
        slot: &str,
        playback_type: super::animation_player::AnimationPlaybackType,
    ) -> bool {
        let Some(index) = self.slot_index(slot) else {
            return false;
        };
        self.resize_animations(index + 1);
        self.animations[index].playback_type = playback_type;
        if let Some(state) = self.internal.slots.get_mut(index) {
            state.boomerang_direction = 1.0;
        }
        true
    }

    pub fn set_weight(&mut self, node: &str, input: &str, weight: f32) -> bool {
        if let Some(existing) = self
            .internal
            .weights
            .iter_mut()
            .find(|w| w.node.as_ref() == node && w.input.as_ref() == input)
        {
            existing.weight = weight;
        } else {
            self.internal.weights.push(AnimationTreeRuntimeWeight {
                node: node.to_string().into(),
                input: input.to_string().into(),
                weight,
            });
        }
        true
    }

    pub fn set_slot_binding(&mut self, slot: usize, object: &str, node: perro_ids::NodeID) {
        self.resize_animations(slot + 1);
        set_binding(&mut self.animations[slot].bindings, object, node);
    }

    pub fn resize_animations(&mut self, len: usize) {
        self.animations.resize_with(len, default_tree_animation);
    }

    fn slot_index(&self, slot: &str) -> Option<usize> {
        self.internal
            .slots
            .iter()
            .position(|state| state.name.as_ref() == slot)
    }
}

fn default_tree_animation() -> AnimationTreeAnimation {
    AnimationTreeAnimation {
        animation: AnimationID::nil(),
        bindings: Vec::new(),
        speed: 1.0,
        paused: false,
        playback_type: super::animation_player::AnimationPlaybackType::Loop,
    }
}

fn set_binding(
    bindings: &mut Vec<super::animation_player::AnimationObjectBinding>,
    object: &str,
    node: perro_ids::NodeID,
) {
    if let Some(binding) = bindings.iter_mut().find(|b| b.object.as_ref() == object) {
        binding.node = node;
    } else {
        bindings.push(super::animation_player::AnimationObjectBinding {
            object: object.to_string().into(),
            node,
        });
    }
}
