use std::borrow::Cow;
use perro_scene::{Node3DField, NodeField};
mod panim;
pub use panim::parse_panim;

#[derive(Clone, Debug, Default)]
pub struct AnimationClip {
    pub name: Cow<'static, str>,
    pub fps: f32,
    pub total_frames: u32,
    pub objects: Cow<'static, [AnimationObject]>,
    pub object_tracks: Cow<'static, [AnimationObjectTrack]>,
    pub frame_events: Cow<'static, [AnimationFrameEvent]>,
}

impl AnimationClip {
    #[inline]
    pub fn frame_count(&self) -> u32 {
        self.total_frames.max(1)
    }

    #[inline]
    pub fn duration_seconds(&self) -> f32 {
        if self.fps <= 0.0 || self.total_frames <= 1 {
            0.0
        } else {
            self.total_frames as f32 / self.fps
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationObject {
    pub name: Cow<'static, str>,
    pub node_type: Cow<'static, str>,
}

#[derive(Clone, Debug)]
pub struct AnimationObjectTrack {
    pub object: Cow<'static, str>,
    pub field: NodeField,
    pub bone_target: Option<AnimationBoneTarget>,
    pub interpolation: AnimationInterpolation,
    pub ease: AnimationEase,
    pub keys: Cow<'static, [AnimationObjectKey]>,
}

impl Default for AnimationObjectTrack {
    fn default() -> Self {
        Self {
            object: Cow::Borrowed(""),
            field: NodeField::Node3D(Node3DField::Visible),
            bone_target: None,
            interpolation: AnimationInterpolation::Linear,
            ease: AnimationEase::Linear,
            keys: Cow::Borrowed(&[]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationBoneTarget {
    pub selector: AnimationBoneSelector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimationBoneSelector {
    Index(u32),
    Name(Cow<'static, str>),
}

#[derive(Clone, Debug)]
pub struct AnimationObjectKey {
    pub frame: u32,
    pub interpolation: AnimationInterpolation,
    pub ease: AnimationEase,
    pub value: AnimationTrackValue,
}

impl Default for AnimationObjectKey {
    fn default() -> Self {
        Self {
            frame: 0,
            interpolation: AnimationInterpolation::Linear,
            ease: AnimationEase::Linear,
            value: AnimationTrackValue::F32(0.0),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationFrameEvent {
    pub frame: u32,
    pub scope: AnimationEventScope,
    pub event: AnimationEvent,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AnimationEventScope {
    #[default]
    Global,
    Object(Cow<'static, str>),
}

#[derive(Clone, Debug)]
pub enum AnimationEvent {
    EmitSignal {
        name: Cow<'static, str>,
        params: Cow<'static, [AnimationParam]>,
    },
    SetVar {
        name: Cow<'static, str>,
        value: AnimationParam,
    },
    CallMethod {
        name: Cow<'static, str>,
        params: Cow<'static, [AnimationParam]>,
    },
}

impl Default for AnimationEvent {
    fn default() -> Self {
        Self::EmitSignal {
            name: Cow::Borrowed(""),
            params: Cow::Borrowed(&[]),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AnimationParam {
    Bool(bool),
    I32(i32),
    U32(u32),
    F32(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    String(Cow<'static, str>),
    Transform2D(perro_structs::Transform2D),
    Transform3D(perro_structs::Transform3D),
    ObjectNode(Cow<'static, str>),
    ObjectField {
        object: Cow<'static, str>,
        field: Cow<'static, str>,
    },
}

impl Default for AnimationParam {
    fn default() -> Self {
        Self::F32(0.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationInterpolation {
    #[default]
    Linear,
    Step,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationEase {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

#[derive(Clone, Debug)]
pub enum AnimationTrackValue {
    Bool(bool),
    I32(i32),
    U32(u32),
    F32(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    AssetPath(Cow<'static, str>),
    Transform2D(perro_structs::Transform2D),
    Transform3D(perro_structs::Transform3D),
}

impl Default for AnimationTrackValue {
    fn default() -> Self {
        Self::F32(0.0)
    }
}
