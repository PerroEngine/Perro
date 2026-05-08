use perro_ids::{AnimationID, AnimationMixClipID};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimationMixerSlotState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationMixerSlot {
    pub animation: AnimationID,
    pub time_seconds: f32,
    pub speed: f32,
    pub state: AnimationMixerSlotState,
    pub looping: bool,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationMixerGraph {
    pub nodes: Vec<AnimationMixerNode>,
    pub output: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct AnimationMixerNode {
    pub kind: AnimationMixerNodeKind,
}

#[derive(Clone, Debug)]
pub enum AnimationMixerNodeKind {
    SlotRef {
        slot_index: usize,
    },
    BlendN {
        inputs: Vec<usize>,
        weights: Vec<f32>,
    },
    AddN {
        inputs: Vec<usize>,
        weights: Vec<f32>,
    },
    Invert {
        input: usize,
    },
    Output {
        input: usize,
    },
}

#[derive(Clone, Debug, Default)]
pub struct AnimationMixerInternalData {
    pub last_resolved_animation: AnimationID,
}

#[derive(Clone, Debug)]
pub struct AnimationMixer {
    pub slots: Vec<AnimationMixerSlot>,
    pub graph: AnimationMixerGraph,
    pub blend_tree: AnimationMixClipID,
    pub reverse_slot_lookup: HashMap<AnimationID, Vec<usize>>,
    pub internal: AnimationMixerInternalData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationMixerError {
    InvalidSlot(usize),
    InvalidNode(usize),
    InvalidNodeInput { node_id: usize, input_index: usize },
}

impl AnimationMixer {
    pub fn new(slot_count: usize) -> Self {
        Self {
            slots: (0..slot_count)
                .map(|_| AnimationMixerSlot {
                    speed: 1.0,
                    looping: true,
                    weight: 1.0,
                    ..AnimationMixerSlot::default()
                })
                .collect(),
            graph: AnimationMixerGraph::default(),
            blend_tree: AnimationMixClipID::nil(),
            reverse_slot_lookup: HashMap::new(),
            internal: AnimationMixerInternalData::default(),
        }
    }

    pub fn set_mix_clip(&mut self, mix_clip: AnimationMixClipID) {
        self.blend_tree = mix_clip;
    }

    pub fn set_slot_animation(
        &mut self,
        slot_index: usize,
        animation: AnimationID,
    ) -> Result<(), AnimationMixerError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationMixerError::InvalidSlot(slot_index));
        };
        if !slot.animation.is_nil() {
            if let Some(v) = self.reverse_slot_lookup.get_mut(&slot.animation) {
                v.retain(|idx| *idx != slot_index);
            }
        }
        slot.animation = animation;
        self.reverse_slot_lookup
            .entry(animation)
            .or_default()
            .push(slot_index);
        Ok(())
    }

    pub fn slot_indices_for_animation(&self, animation: AnimationID) -> &[usize] {
        self.reverse_slot_lookup
            .get(&animation)
            .map_or(&[], |v| v.as_slice())
    }

    pub fn play_slot(&mut self, slot_index: usize) -> Result<(), AnimationMixerError> {
        self.set_slot_state(slot_index, AnimationMixerSlotState::Playing)
    }
    pub fn pause_slot(&mut self, slot_index: usize) -> Result<(), AnimationMixerError> {
        self.set_slot_state(slot_index, AnimationMixerSlotState::Paused)
    }
    pub fn stop_slot(&mut self, slot_index: usize) -> Result<(), AnimationMixerError> {
        self.set_slot_state(slot_index, AnimationMixerSlotState::Stopped)?;
        self.seek_slot(slot_index, 0.0)
    }
    pub fn seek_slot(
        &mut self,
        slot_index: usize,
        time_seconds: f32,
    ) -> Result<(), AnimationMixerError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationMixerError::InvalidSlot(slot_index));
        };
        slot.time_seconds = time_seconds.max(0.0);
        Ok(())
    }

    pub fn play_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationMixerSlotState::Playing;
        }
    }
    pub fn pause_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationMixerSlotState::Paused;
        }
    }
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationMixerSlotState::Stopped;
            slot.time_seconds = 0.0;
        }
    }

    pub fn play_animation(&mut self, animation: AnimationID) {
        for idx in self.slot_indices_for_animation(animation).to_vec() {
            let _ = self.play_slot(idx);
        }
    }
    pub fn pause_animation(&mut self, animation: AnimationID) {
        for idx in self.slot_indices_for_animation(animation).to_vec() {
            let _ = self.pause_slot(idx);
        }
    }
    pub fn seek_animation(&mut self, animation: AnimationID, time_seconds: f32) {
        for idx in self.slot_indices_for_animation(animation).to_vec() {
            let _ = self.seek_slot(idx, time_seconds);
        }
    }

    pub fn set_blend_weight(
        &mut self,
        node_id: usize,
        input_index: usize,
        weight: f32,
    ) -> Result<(), AnimationMixerError> {
        let Some(node) = self.graph.nodes.get_mut(node_id) else {
            return Err(AnimationMixerError::InvalidNode(node_id));
        };
        match &mut node.kind {
            AnimationMixerNodeKind::BlendN { weights, .. }
            | AnimationMixerNodeKind::AddN { weights, .. } => {
                let Some(input) = weights.get_mut(input_index) else {
                    return Err(AnimationMixerError::InvalidNodeInput {
                        node_id,
                        input_index,
                    });
                };
                *input = weight;
                Ok(())
            }
            _ => Err(AnimationMixerError::InvalidNodeInput {
                node_id,
                input_index,
            }),
        }
    }

    pub fn evaluate_output_animation(&self) -> Result<Option<AnimationID>, AnimationMixerError> {
        let Some(output) = self.graph.output else {
            return Ok(None);
        };
        self.eval_node(output)
    }

    fn eval_node(&self, node_id: usize) -> Result<Option<AnimationID>, AnimationMixerError> {
        let Some(node) = self.graph.nodes.get(node_id) else {
            return Err(AnimationMixerError::InvalidNode(node_id));
        };
        match &node.kind {
            AnimationMixerNodeKind::SlotRef { slot_index } => Ok(self
                .slots
                .get(*slot_index)
                .map(|s| s.animation)
                .filter(|id| !id.is_nil())),
            AnimationMixerNodeKind::BlendN { inputs, weights }
            | AnimationMixerNodeKind::AddN { inputs, weights } => {
                if inputs.is_empty() {
                    return Ok(None);
                }
                let mut best: Option<(AnimationID, f32)> = None;
                for (i, input) in inputs.iter().enumerate() {
                    let Some(anim) = self.eval_node(*input)? else {
                        continue;
                    };
                    let w = *weights.get(i).unwrap_or(&1.0);
                    if best.as_ref().is_none_or(|(_, bw)| w > *bw) {
                        best = Some((anim, w));
                    }
                }
                Ok(best.map(|v| v.0))
            }
            AnimationMixerNodeKind::Invert { input } | AnimationMixerNodeKind::Output { input } => {
                self.eval_node(*input)
            }
        }
    }

    fn set_slot_state(
        &mut self,
        slot_index: usize,
        state: AnimationMixerSlotState,
    ) -> Result<(), AnimationMixerError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationMixerError::InvalidSlot(slot_index));
        };
        slot.state = state;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_tree_core_api() {
        let mut tree = AnimationMixer::new(3);
        assert_eq!(tree.slots.len(), 3);
        tree.set_slot_animation(0, AnimationID::from_u32(10))
            .unwrap();
        tree.set_slot_animation(1, AnimationID::from_u32(11))
            .unwrap();
        tree.set_slot_animation(2, AnimationID::from_u32(11))
            .unwrap();
        assert_eq!(
            tree.slot_indices_for_animation(AnimationID::from_u32(11)),
            &[1, 2]
        );
        tree.play_slot(1).unwrap();
        tree.pause_slot(1).unwrap();
        tree.seek_slot(1, 2.5).unwrap();
        tree.stop_slot(1).unwrap();
    }
}

pub type AnimationTree = AnimationMixer;
