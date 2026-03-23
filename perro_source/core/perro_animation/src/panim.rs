use crate::{
    AnimationChannel, AnimationClip, AnimationEvent, AnimationEventScope, AnimationFrameEvent,
    AnimationInterpolation, AnimationObject, AnimationObjectKey, AnimationObjectTrack,
    AnimationParam, AnimationTrackValue, Camera3DChannel, Light3DChannel, Node2DChannel,
    Node3DChannel, PointLight3DChannel, SpotLight3DChannel,
};
use perro_scene::{NodeField, Parser as SceneParser, SceneValue, resolve_node_field};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

include!("panim/core.rs");
include!("panim/fields.rs");
include!("panim/tracks.rs");
include!("panim/events.rs");
include!("panim/syntax.rs");
include!("panim/tests.rs");
