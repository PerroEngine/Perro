use perro_ids::AnimationID;
use perro_structs::{Transform2D, Transform3D};
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppliedAnimationTransformKind {
    Node2D,
    Node3D,
}

#[derive(Clone, Copy, Debug)]
pub struct AppliedAnimationTransform {
    pub node: perro_ids::NodeID,
    pub kind: AppliedAnimationTransformKind,
    pub transform_2d: Transform2D,
    pub transform_3d: Transform3D,
}

#[derive(Clone, Debug, Default)]
pub struct InternalAnimationData {
    pub last_applied_animation: AnimationID,
    pub last_applied_frame: u32,
    pub last_binding_hash: u64,
    pub playback_frame: f32,
    pub boomerang_direction: f32,
    pub applied_transforms: Vec<AppliedAnimationTransform>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationObjectBinding {
    pub object: Cow<'static, str>,
    pub node: perro_ids::NodeID,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationPlaybackType {
    Once,
    #[default]
    Loop,
    Boomerang,
}

#[derive(Clone, Debug, Default)]
pub struct AnimationPlayer {
    pub animation: AnimationID,
    pub current_frame: u32,
    pub speed: f32,
    pub paused: bool,
    pub playback_type: AnimationPlaybackType,
    pub bindings: Vec<AnimationObjectBinding>,
    pub internal: InternalAnimationData,
}

impl AnimationPlayer {
    pub fn new() -> Self {
        Self {
            animation: AnimationID::nil(),
            current_frame: 0,
            speed: 1.0,
            paused: false,
            playback_type: AnimationPlaybackType::Loop,
            bindings: Vec::new(),
            internal: InternalAnimationData {
                last_applied_animation: AnimationID::nil(),
                last_applied_frame: 0,
                last_binding_hash: 0,
                playback_frame: 0.0,
                boomerang_direction: 1.0,
                applied_transforms: Vec::new(),
            },
        }
    }

    #[inline]
    pub fn set_animation(&mut self, animation: AnimationID) {
        self.animation = animation;
        self.current_frame = 0;
        self.internal.last_applied_animation = AnimationID::nil();
        self.internal.playback_frame = 0.0;
        self.internal.boomerang_direction = 1.0;
    }

    #[inline]
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    #[inline]
    pub fn set_playback_type(&mut self, playback_type: AnimationPlaybackType) {
        self.playback_type = playback_type;
    }

    #[inline]
    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
        self.internal.playback_frame = frame as f32;
        self.internal.boomerang_direction = 1.0;
    }

    #[inline]
    pub fn play(&mut self) {
        self.paused = false;
    }

    #[inline]
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
    }

    #[inline]
    pub fn set_binding(&mut self, object: &str, node: perro_ids::NodeID) {
        if let Some(binding) = self
            .bindings
            .iter_mut()
            .find(|b| b.object.as_ref() == object)
        {
            binding.node = node;
        } else {
            self.bindings.push(AnimationObjectBinding {
                object: object.to_string().into(),
                node,
            });
        }
    }

    #[inline]
    pub fn clear_bindings(&mut self) {
        self.bindings.clear();
    }
}
