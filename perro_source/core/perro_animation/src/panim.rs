use crate::{
    AnimationBoneSelector, AnimationBoneTarget, AnimationClip, AnimationEase, AnimationEvent,
    AnimationEventScope, AnimationFrameEvent, AnimationInterpolation, AnimationObject,
    AnimationObjectKey, AnimationObjectTrack, AnimationParam, AnimationTrackValue,
};
use perro_scene::{
    Camera3DField, Light3DField, MeshInstance3DField, Node2DField, Node3DField, NodeField,
    Parser as SceneParser, PointLight3DField, SceneValue, Skeleton3DField, SpotLight3DField,
    Sprite2DField, resolve_node_field,
};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

include!("panim/core.rs");
include!("panim/fields.rs");
include!("panim/tracks.rs");
include!("panim/events.rs");
include!("panim/syntax.rs");
