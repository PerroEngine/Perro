use crate::node_3d::Node3D;
use perro_animation::{AnimationNodeBinding, AnimationSceneBinding};
use perro_ids::AnimationID;
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};

impl Deref for AnimationPlayer {
    type Target = Node3D;
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for AnimationPlayer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationPlayer {
    pub base: Node3D,
    pub animation_source: Option<Cow<'static, str>>,
    pub animation: AnimationID,
    pub current_time: f32,
    pub current_frame: u32,
    pub speed: f32,
    pub playing: bool,
    pub paused: bool,
    pub looping: bool,
    pub scene_bindings: Vec<AnimationSceneBinding>,
    pub runtime_bindings: Vec<AnimationNodeBinding>,
}

impl AnimationPlayer {
    pub const fn new() -> Self {
        Self {
            base: Node3D::new(),
            animation_source: None,
            animation: AnimationID::nil(),
            current_time: 0.0,
            current_frame: 0,
            speed: 1.0,
            playing: false,
            paused: false,
            looping: true,
            scene_bindings: Vec::new(),
            runtime_bindings: Vec::new(),
        }
    }
}
