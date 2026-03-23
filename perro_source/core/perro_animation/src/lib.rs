use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct AnimationClip {
    pub name: Cow<'static, str>,
    pub fps: f32,
    pub frame_count: u32,
    pub duration: f32,
    pub tracks: Cow<'static, [AnimationTrack]>,
}

#[derive(Clone, Debug)]
pub struct AnimationTrack {
    pub key: Cow<'static, str>,
    pub channel: AnimationChannel,
    pub interpolation: AnimationInterpolation,
    pub values: AnimationTrackValues,
}

impl Default for AnimationTrack {
    fn default() -> Self {
        Self {
            key: Cow::Borrowed("Track"),
            channel: AnimationChannel::Custom(Cow::Borrowed("custom")),
            interpolation: AnimationInterpolation::Step,
            values: AnimationTrackValues::F32(Cow::Borrowed(&[])),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimationChannel {
    Transform2D,
    Transform3D,
    NodeVisible,
    Custom(Cow<'static, str>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum AnimationInterpolation {
    #[default]
    Step,
    Linear,
}

#[derive(Clone, Debug)]
pub enum AnimationTrackValues {
    F32(Cow<'static, [f32]>),
    Vec2(Cow<'static, [[f32; 2]]>),
    Vec3(Cow<'static, [[f32; 3]]>),
    Vec4(Cow<'static, [[f32; 4]]>),
    Transform2D(Cow<'static, [perro_structs::Transform2D]>),
    Transform3D(Cow<'static, [perro_structs::Transform3D]>),
    Bool(Cow<'static, [bool]>),
}

impl Default for AnimationTrackValues {
    fn default() -> Self {
        Self::F32(Cow::Borrowed(&[]))
    }
}

#[derive(Clone, Debug, Default)]
pub struct AnimationNodeBinding {
    pub track: Cow<'static, str>,
    pub node: perro_ids::NodeID,
}
