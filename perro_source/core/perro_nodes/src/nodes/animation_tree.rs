use perro_ids::{AnimationBlendTreeID, AnimationID};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnimationTreeSlotState {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeSlot {
    pub animation: AnimationID,
    pub time_seconds: f32,
    pub speed: f32,
    pub state: AnimationTreeSlotState,
    pub looping: bool,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationTreeGraph {
    pub nodes: Vec<AnimationTreeNode>,
    pub output: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct AnimationTreeNode {
    pub kind: AnimationTreeNodeKind,
}

#[derive(Clone, Debug)]
pub enum AnimationTreeNodeKind {
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
pub struct AnimationTreeInternalData {
    pub last_resolved_animation: AnimationID,
}

#[derive(Clone, Debug)]
pub struct AnimationTree {
    pub slots: Vec<AnimationTreeSlot>,
    pub graph: AnimationTreeGraph,
    pub blend_tree: AnimationBlendTreeID,
    pub reverse_slot_lookup: HashMap<AnimationID, Vec<usize>>,
    pub internal: AnimationTreeInternalData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationTreeError {
    InvalidSlot(usize),
    InvalidNode(usize),
    InvalidNodeInput { node_id: usize, input_index: usize },
}

impl AnimationTree {
    pub fn new(slot_count: usize) -> Self {
        Self {
            slots: (0..slot_count)
                .map(|_| AnimationTreeSlot {
                    speed: 1.0,
                    looping: true,
                    weight: 1.0,
                    ..AnimationTreeSlot::default()
                })
                .collect(),
            graph: AnimationTreeGraph::default(),
            blend_tree: AnimationBlendTreeID::nil(),
            reverse_slot_lookup: HashMap::new(),
            internal: AnimationTreeInternalData::default(),
        }
    }

    pub fn set_blend_tree(&mut self, blend_tree: AnimationBlendTreeID) {
        self.blend_tree = blend_tree;
    }

    pub fn set_slot_animation(
        &mut self,
        slot_index: usize,
        animation: AnimationID,
    ) -> Result<(), AnimationTreeError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationTreeError::InvalidSlot(slot_index));
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

    pub fn play_slot(&mut self, slot_index: usize) -> Result<(), AnimationTreeError> {
        self.set_slot_state(slot_index, AnimationTreeSlotState::Playing)
    }
    pub fn pause_slot(&mut self, slot_index: usize) -> Result<(), AnimationTreeError> {
        self.set_slot_state(slot_index, AnimationTreeSlotState::Paused)
    }
    pub fn stop_slot(&mut self, slot_index: usize) -> Result<(), AnimationTreeError> {
        self.set_slot_state(slot_index, AnimationTreeSlotState::Stopped)?;
        self.seek_slot(slot_index, 0.0)
    }
    pub fn seek_slot(
        &mut self,
        slot_index: usize,
        time_seconds: f32,
    ) -> Result<(), AnimationTreeError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationTreeError::InvalidSlot(slot_index));
        };
        slot.time_seconds = time_seconds.max(0.0);
        Ok(())
    }

    pub fn play_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationTreeSlotState::Playing;
        }
    }
    pub fn pause_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationTreeSlotState::Paused;
        }
    }
    pub fn stop_all(&mut self) {
        for slot in &mut self.slots {
            slot.state = AnimationTreeSlotState::Stopped;
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
    ) -> Result<(), AnimationTreeError> {
        let Some(node) = self.graph.nodes.get_mut(node_id) else {
            return Err(AnimationTreeError::InvalidNode(node_id));
        };
        match &mut node.kind {
            AnimationTreeNodeKind::BlendN { weights, .. }
            | AnimationTreeNodeKind::AddN { weights, .. } => {
                let Some(input) = weights.get_mut(input_index) else {
                    return Err(AnimationTreeError::InvalidNodeInput {
                        node_id,
                        input_index,
                    });
                };
                *input = weight;
                Ok(())
            }
            _ => Err(AnimationTreeError::InvalidNodeInput {
                node_id,
                input_index,
            }),
        }
    }

    pub fn evaluate_output_animation(&self) -> Result<Option<AnimationID>, AnimationTreeError> {
        let Some(output) = self.graph.output else {
            return Ok(None);
        };
        self.eval_node(output)
    }

    fn eval_node(&self, node_id: usize) -> Result<Option<AnimationID>, AnimationTreeError> {
        let Some(node) = self.graph.nodes.get(node_id) else {
            return Err(AnimationTreeError::InvalidNode(node_id));
        };
        match &node.kind {
            AnimationTreeNodeKind::SlotRef { slot_index } => Ok(self
                .slots
                .get(*slot_index)
                .map(|s| s.animation)
                .filter(|id| !id.is_nil())),
            AnimationTreeNodeKind::BlendN { inputs, weights }
            | AnimationTreeNodeKind::AddN { inputs, weights } => {
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
            AnimationTreeNodeKind::Invert { input } | AnimationTreeNodeKind::Output { input } => {
                self.eval_node(*input)
            }
        }
    }

    fn set_slot_state(
        &mut self,
        slot_index: usize,
        state: AnimationTreeSlotState,
    ) -> Result<(), AnimationTreeError> {
        let Some(slot) = self.slots.get_mut(slot_index) else {
            return Err(AnimationTreeError::InvalidSlot(slot_index));
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
        let mut tree = AnimationTree::new(3);
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

pub type AnimationMixer = AnimationTree;
