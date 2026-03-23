use std::borrow::Cow;
mod panim;
pub use panim::parse_panim;

#[derive(Clone, Debug, Default)]
pub struct AnimationClip {
    pub name: Cow<'static, str>,
    pub fps: f32,
    pub total_frames: u32,
    pub looping: bool,
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

#[derive(Clone, Debug, Default)]
pub struct AnimationObjectTrack {
    pub object: Cow<'static, str>,
    pub channel: AnimationChannel,
    pub interpolation: AnimationInterpolation,
    pub keys: Cow<'static, [AnimationObjectKey]>,
}

#[derive(Clone, Debug)]
pub struct AnimationObjectKey {
    pub frame: u32,
    pub value: AnimationTrackValue,
}

impl Default for AnimationObjectKey {
    fn default() -> Self {
        Self {
            frame: 0,
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
}

impl Default for AnimationParam {
    fn default() -> Self {
        Self::F32(0.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimationChannel {
    Node2D(Node2DChannel),
    Node3D(Node3DChannel),
    Camera3D(Camera3DChannel),
    Light3D(Light3DChannel),
    PointLight3D(PointLight3DChannel),
    SpotLight3D(SpotLight3DChannel),
    Custom(Cow<'static, str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node2DChannel {
    Transform,
    Visible,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node3DChannel {
    Transform,
    Visible,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Camera3DChannel {
    Zoom,
    PerspectiveFovYDegrees,
    PerspectiveNear,
    PerspectiveFar,
    OrthographicSize,
    OrthographicNear,
    OrthographicFar,
    FrustumLeft,
    FrustumRight,
    FrustumBottom,
    FrustumTop,
    FrustumNear,
    FrustumFar,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Light3DChannel {
    Color,
    Intensity,
    Active,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointLight3DChannel {
    Range,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpotLight3DChannel {
    Range,
    InnerAngleRadians,
    OuterAngleRadians,
}

impl Default for AnimationChannel {
    fn default() -> Self {
        Self::Custom(Cow::Borrowed("custom"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationInterpolation {
    #[default]
    Step,
    Linear,
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
    Transform2D(perro_structs::Transform2D),
    Transform3D(perro_structs::Transform3D),
}

impl Default for AnimationTrackValue {
    fn default() -> Self {
        Self::F32(0.0)
    }
}
