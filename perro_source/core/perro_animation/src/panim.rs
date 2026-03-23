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

pub fn parse_panim(source: &str) -> Result<AnimationClip, String> {
    let mut parser = PanimParser::new(source);
    parser.parse()
}

struct PanimParser<'a> {
    lines: Vec<&'a str>,
    index: usize,
}

impl<'a> PanimParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
            index: 0,
        }
    }

    fn parse(&mut self) -> Result<AnimationClip, String> {
        let mut name = Cow::Borrowed("Animation");
        let mut fps = 60.0f32;
        let mut looping = true;
        let mut objects: Vec<AnimationObject> = Vec::new();
        let mut object_types = HashMap::<String, String>::new();
        let mut frame_actions: Vec<FrameAction> = Vec::new();
        let mut max_frame = 0u32;

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }

            if line.eq_ignore_ascii_case("[Animation]") {
                let (n, f, l) = self.parse_animation_block(line_no)?;
                name = Cow::Owned(n);
                fps = f;
                looping = l;
                continue;
            }
            if line.eq_ignore_ascii_case("[Objects]") {
                let parsed_objects = self.parse_objects_block(line_no)?;
                object_types.clear();
                for obj in &parsed_objects {
                    object_types.insert(obj.name.to_string(), obj.node_type.to_string());
                }
                objects = parsed_objects;
                continue;
            }
            if let Some(frame) = parse_frame_header(line) {
                max_frame = max_frame.max(frame);
                frame_actions.extend(self.parse_frame_block(line_no, frame, &object_types)?);
                continue;
            }

            return Err(format!(
                "line {}: unexpected top-level token `{}`",
                line_no, line
            ));
        }

        let (object_tracks, mut frame_events) =
            build_tracks_and_events(frame_actions, &object_types)?;
        frame_events.sort_by_key(|e| e.frame);

        let total_frames = max_frame.saturating_add(1).max(1);

        Ok(AnimationClip {
            name,
            fps,
            total_frames,
            looping,
            objects: Cow::Owned(objects),
            object_tracks: Cow::Owned(object_tracks),
            frame_events: Cow::Owned(frame_events),
        })
    }

    fn parse_animation_block(&mut self, start_line: usize) -> Result<(String, f32, bool), String> {
        let mut name = String::from("Animation");
        let mut fps = 60.0f32;
        let mut looping = true;

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("[/Animation]") {
                if !fps.is_finite() || fps <= 0.0 {
                    return Err(format!(
                        "line {}: fps must be a finite positive number",
                        start_line
                    ));
                }
                return Ok((name, fps, looping));
            }

            let Some((k, v)) = split_key_value(line) else {
                return Err(format!("line {}: expected `key = value`", line_no));
            };
            let value = parse_scene_value(v, line_no)?;
            match k {
                "name" => {
                    name = as_text(&value)
                        .ok_or_else(|| format!("line {}: name must be text", line_no))?
                        .to_string();
                }
                "fps" => {
                    fps = value
                        .as_f32()
                        .ok_or_else(|| format!("line {}: fps must be a number", line_no))?;
                }
                "looping" | "loop" => {
                    looping = value
                        .as_bool()
                        .ok_or_else(|| format!("line {}: looping must be bool", line_no))?;
                }
                _ => {}
            }
        }

        Err(format!(
            "line {}: missing closing `[/Animation]` block",
            start_line
        ))
    }

    fn parse_objects_block(&mut self, start_line: usize) -> Result<Vec<AnimationObject>, String> {
        let mut objects = Vec::new();
        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line.eq_ignore_ascii_case("[/Objects]") {
                return Ok(objects);
            }

            let Some(rest) = line.strip_prefix('@') else {
                return Err(format!(
                    "line {}: object definition must start with `@`",
                    line_no
                ));
            };
            let Some((name, ty)) = rest.split_once('=') else {
                return Err(format!("line {}: expected `@Name = NodeType`", line_no));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(format!("line {}: object name cannot be empty", line_no));
            }

            let ty_value = parse_scene_value(ty.trim(), line_no)?;
            let ty = as_text(&ty_value)
                .ok_or_else(|| format!("line {}: object type must be text", line_no))?;
            objects.push(AnimationObject {
                name: name.to_string().into(),
                node_type: ty.to_string().into(),
            });
        }

        Err(format!(
            "line {}: missing closing `[/Objects]` block",
            start_line
        ))
    }

    fn parse_frame_block(
        &mut self,
        start_line: usize,
        frame: u32,
        object_types: &HashMap<String, String>,
    ) -> Result<Vec<FrameAction>, String> {
        let mut actions = Vec::new();

        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if is_frame_footer(line) {
                return Ok(actions);
            }

            if line.starts_with('@') && line.ends_with('{') {
                actions.extend(self.parse_object_block(line_no, frame, line, object_types)?);
                continue;
            }

            if let Some((k, v)) = split_key_value(line)
                && k == "emit_signal"
            {
                actions.push(FrameAction::Event {
                    frame,
                    scope: AnimationEventScope::Global,
                    event: parse_emit_signal(v, line_no)?,
                });
                continue;
            }

            return Err(format!("line {}: invalid frame entry `{}`", line_no, line));
        }

        Err(format!("line {}: missing frame footer", start_line))
    }

    fn parse_object_block(
        &mut self,
        start_line: usize,
        frame: u32,
        header: &str,
        object_types: &HashMap<String, String>,
    ) -> Result<Vec<FrameAction>, String> {
        let object = header
            .strip_prefix('@')
            .and_then(|v| v.strip_suffix('{'))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("line {}: invalid object header", start_line))?
            .to_string();

        let node_type = object_types
            .get(&object)
            .ok_or_else(|| format!("line {}: unknown object `@{}`", start_line, object))?;
        let mut actions = Vec::new();
        while let Some((line_no, line)) = self.next_line() {
            let line = strip_comment(line).trim();
            if line.is_empty() {
                continue;
            }
            if line == "}" {
                return Ok(actions);
            }

            let Some((k, v)) = split_key_value(line) else {
                return Err(format!(
                    "line {}: expected `key = value` inside object block",
                    line_no
                ));
            };

            match k {
                "emit_signal" => actions.push(FrameAction::Event {
                    frame,
                    scope: AnimationEventScope::Object(object.clone().into()),
                    event: parse_emit_signal(v, line_no)?,
                }),
                "set_var" => actions.push(FrameAction::Event {
                    frame,
                    scope: AnimationEventScope::Object(object.clone().into()),
                    event: parse_set_var(v, line_no)?,
                }),
                "call_method" => actions.push(FrameAction::Event {
                    frame,
                    scope: AnimationEventScope::Object(object.clone().into()),
                    event: parse_call_method(v, line_no)?,
                }),
                _ => {
                    let value = parse_scene_value(v, line_no)?;
                    match resolve_node_field(node_type, k) {
                        Some(NodeField::Position2D) => {
                            let (x, y) = value
                                .as_vec2()
                                .ok_or_else(|| format!("line {}: `{}` expects vec2", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Position2D(Vector2::new(x, y)),
                            });
                        }
                        Some(NodeField::Rotation2D) => {
                            let rot = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Rotation2D(rot),
                            });
                        }
                        Some(NodeField::Scale2D) => {
                            let (x, y) = value
                                .as_vec2()
                                .ok_or_else(|| format!("line {}: `{}` expects vec2", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Scale2D(Vector2::new(x, y)),
                            });
                        }
                        Some(NodeField::Position3D) => {
                            let (x, y, z) = value
                                .as_vec3()
                                .ok_or_else(|| format!("line {}: `{}` expects vec3", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Position3D(Vector3::new(x, y, z)),
                            });
                        }
                        Some(NodeField::Rotation3D) => {
                            let (x, y, z, w) = value
                                .as_vec4()
                                .ok_or_else(|| format!("line {}: `{}` expects vec4", line_no, k))?;
                            let mut quat = Quaternion::new(x, y, z, w);
                            quat.normalize();
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Rotation3D(quat),
                            });
                        }
                        Some(NodeField::Scale3D) => {
                            let (x, y, z) = value
                                .as_vec3()
                                .ok_or_else(|| format!("line {}: `{}` expects vec3", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Scale3D(Vector3::new(x, y, z)),
                            });
                        }
                        Some(NodeField::Visible2D) | Some(NodeField::Visible3D) => {
                            let visible = value
                                .as_bool()
                                .ok_or_else(|| format!("line {}: `{}` expects bool", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Visible(visible),
                            });
                        }
                        Some(NodeField::Camera3DZoom) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DZoom(v),
                            });
                        }
                        Some(NodeField::Camera3DPerspectiveFovYDegrees) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DPerspectiveFovYDegrees(v),
                            });
                        }
                        Some(NodeField::Camera3DPerspectiveNear) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DPerspectiveNear(v),
                            });
                        }
                        Some(NodeField::Camera3DPerspectiveFar) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DPerspectiveFar(v),
                            });
                        }
                        Some(NodeField::Camera3DOrthographicSize) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DOrthographicSize(v),
                            });
                        }
                        Some(NodeField::Camera3DOrthographicNear) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DOrthographicNear(v),
                            });
                        }
                        Some(NodeField::Camera3DOrthographicFar) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DOrthographicFar(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumLeft) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumLeft(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumRight) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumRight(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumBottom) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumBottom(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumTop) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumTop(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumNear) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumNear(v),
                            });
                        }
                        Some(NodeField::Camera3DFrustumFar) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DFrustumFar(v),
                            });
                        }
                        Some(NodeField::Camera3DActive) => {
                            let v = value
                                .as_bool()
                                .ok_or_else(|| format!("line {}: `{}` expects bool", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::Camera3DActive(v),
                            });
                        }
                        Some(NodeField::AmbientLight3DColor)
                        | Some(NodeField::RayLight3DColor)
                        | Some(NodeField::PointLight3DColor)
                        | Some(NodeField::SpotLight3DColor) => {
                            let (x, y, z) = value
                                .as_vec3()
                                .ok_or_else(|| format!("line {}: `{}` expects vec3", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::LightColor([x, y, z]),
                            });
                        }
                        Some(NodeField::AmbientLight3DIntensity)
                        | Some(NodeField::RayLight3DIntensity)
                        | Some(NodeField::PointLight3DIntensity)
                        | Some(NodeField::SpotLight3DIntensity) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::LightIntensity(v),
                            });
                        }
                        Some(NodeField::AmbientLight3DActive)
                        | Some(NodeField::RayLight3DActive)
                        | Some(NodeField::PointLight3DActive)
                        | Some(NodeField::SpotLight3DActive) => {
                            let v = value
                                .as_bool()
                                .ok_or_else(|| format!("line {}: `{}` expects bool", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::LightActive(v),
                            });
                        }
                        Some(NodeField::PointLight3DRange) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::PointLightRange(v),
                            });
                        }
                        Some(NodeField::SpotLight3DRange) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::SpotLightRange(v),
                            });
                        }
                        Some(NodeField::SpotLight3DInnerAngleRadians) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::SpotLightInnerAngleRadians(v),
                            });
                        }
                        Some(NodeField::SpotLight3DOuterAngleRadians) => {
                            let v = value
                                .as_f32()
                                .ok_or_else(|| format!("line {}: `{}` expects f32", line_no, k))?;
                            actions.push(FrameAction::Field {
                                frame,
                                object: object.clone(),
                                field: ObjectFieldAction::SpotLightOuterAngleRadians(v),
                            });
                        }
                        Some(NodeField::ZIndex2D) => {
                            return Err(format!(
                                "line {}: `z_index` is valid for `{}` but not yet animatable",
                                line_no, node_type
                            ));
                        }
                        Some(_field) => {
                            return Err(format!(
                                "line {}: `{}` is valid for `{}` but not yet animatable in `.panim`",
                                line_no, k, node_type
                            ));
                        }
                        None => {
                            return Err(format!(
                                "line {}: unsupported object key `{}` for node type `{}`",
                                line_no, k, node_type
                            ));
                        }
                    }
                }
            }
        }

        Err(format!(
            "line {}: object block `{}` missing closing `}}`",
            start_line, object
        ))
    }

    fn next_line(&mut self) -> Option<(usize, &'a str)> {
        if self.index >= self.lines.len() {
            return None;
        }
        let line_no = self.index + 1;
        let line = self.lines[self.index];
        self.index += 1;
        Some((line_no, line))
    }
}

enum ObjectFieldAction {
    Position2D(Vector2),
    Rotation2D(f32),
    Scale2D(Vector2),
    Position3D(Vector3),
    Rotation3D(Quaternion),
    Scale3D(Vector3),
    Visible(bool),
    Camera3DZoom(f32),
    Camera3DPerspectiveFovYDegrees(f32),
    Camera3DPerspectiveNear(f32),
    Camera3DPerspectiveFar(f32),
    Camera3DOrthographicSize(f32),
    Camera3DOrthographicNear(f32),
    Camera3DOrthographicFar(f32),
    Camera3DFrustumLeft(f32),
    Camera3DFrustumRight(f32),
    Camera3DFrustumBottom(f32),
    Camera3DFrustumTop(f32),
    Camera3DFrustumNear(f32),
    Camera3DFrustumFar(f32),
    Camera3DActive(bool),
    LightColor([f32; 3]),
    LightIntensity(f32),
    LightActive(bool),
    PointLightRange(f32),
    SpotLightRange(f32),
    SpotLightInnerAngleRadians(f32),
    SpotLightOuterAngleRadians(f32),
}

enum FrameAction {
    Field {
        frame: u32,
        object: String,
        field: ObjectFieldAction,
    },
    Event {
        frame: u32,
        scope: AnimationEventScope,
        event: AnimationEvent,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TrackKind {
    Node2DTransform,
    Node2DVisible,
    Node3DTransform,
    Node3DVisible,
    Camera3DZoom,
    Camera3DPerspectiveFovYDegrees,
    Camera3DPerspectiveNear,
    Camera3DPerspectiveFar,
    Camera3DOrthographicSize,
    Camera3DOrthographicNear,
    Camera3DOrthographicFar,
    Camera3DFrustumLeft,
    Camera3DFrustumRight,
    Camera3DFrustumBottom,
    Camera3DFrustumTop,
    Camera3DFrustumNear,
    Camera3DFrustumFar,
    Camera3DActive,
    Light3DColor,
    Light3DIntensity,
    Light3DActive,
    PointLight3DRange,
    SpotLight3DRange,
    SpotLight3DInnerAngleRadians,
    SpotLight3DOuterAngleRadians,
}

#[derive(Clone, Debug)]
struct ObjectState2D {
    transform: Transform2D,
    visible: bool,
}

#[derive(Clone, Debug)]
struct ObjectState3D {
    transform: Transform3D,
    visible: bool,
}

fn build_tracks_and_events(
    mut actions: Vec<FrameAction>,
    object_types: &HashMap<String, String>,
) -> Result<(Vec<AnimationObjectTrack>, Vec<AnimationFrameEvent>), String> {
    let mut frame_events = Vec::<AnimationFrameEvent>::new();

    let mut fields = Vec::<(u32, String, ObjectFieldAction)>::new();
    for action in actions.drain(..) {
        match action {
            FrameAction::Field {
                frame,
                object,
                field,
            } => fields.push((frame, object, field)),
            FrameAction::Event {
                frame,
                scope,
                event,
            } => frame_events.push(AnimationFrameEvent {
                frame,
                scope,
                event,
            }),
        }
    }

    fields.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut state_2d = HashMap::<String, ObjectState2D>::new();
    let mut state_3d = HashMap::<String, ObjectState3D>::new();
    let mut tracks_map = BTreeMap::<(String, TrackKind), BTreeMap<u32, AnimationTrackValue>>::new();

    for (frame, object, field) in fields {
        match field {
            ObjectFieldAction::Position2D(v) => {
                let state = state_2d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState2D {
                        transform: Transform2D::new(Vector2::ZERO, 0.0, Vector2::ONE),
                        visible: true,
                    });
                state.transform.position = v;
                tracks_map
                    .entry((object, TrackKind::Node2DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform2D(state.transform));
            }
            ObjectFieldAction::Rotation2D(v) => {
                let state = state_2d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState2D {
                        transform: Transform2D::new(Vector2::ZERO, 0.0, Vector2::ONE),
                        visible: true,
                    });
                state.transform.rotation = v;
                tracks_map
                    .entry((object, TrackKind::Node2DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform2D(state.transform));
            }
            ObjectFieldAction::Scale2D(v) => {
                let state = state_2d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState2D {
                        transform: Transform2D::new(Vector2::ZERO, 0.0, Vector2::ONE),
                        visible: true,
                    });
                state.transform.scale = v;
                tracks_map
                    .entry((object, TrackKind::Node2DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform2D(state.transform));
            }
            ObjectFieldAction::Position3D(v) => {
                let state = state_3d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState3D {
                        transform: Transform3D::new(
                            Vector3::ZERO,
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        ),
                        visible: true,
                    });
                state.transform.position = v;
                tracks_map
                    .entry((object, TrackKind::Node3DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform3D(state.transform));
            }
            ObjectFieldAction::Rotation3D(v) => {
                let state = state_3d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState3D {
                        transform: Transform3D::new(
                            Vector3::ZERO,
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        ),
                        visible: true,
                    });
                state.transform.rotation = v;
                tracks_map
                    .entry((object, TrackKind::Node3DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform3D(state.transform));
            }
            ObjectFieldAction::Scale3D(v) => {
                let state = state_3d
                    .entry(object.clone())
                    .or_insert_with(|| ObjectState3D {
                        transform: Transform3D::new(
                            Vector3::ZERO,
                            Quaternion::IDENTITY,
                            Vector3::ONE,
                        ),
                        visible: true,
                    });
                state.transform.scale = v;
                tracks_map
                    .entry((object, TrackKind::Node3DTransform))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Transform3D(state.transform));
            }
            ObjectFieldAction::Visible(v) => {
                let Some(node_type) = object_types.get(object.as_str()) else {
                    continue;
                };
                match resolve_node_field(node_type, "position") {
                    Some(NodeField::Position2D) => {
                        let state =
                            state_2d
                                .entry(object.clone())
                                .or_insert_with(|| ObjectState2D {
                                    transform: Transform2D::new(Vector2::ZERO, 0.0, Vector2::ONE),
                                    visible: true,
                                });
                        state.visible = v;
                        tracks_map
                            .entry((object, TrackKind::Node2DVisible))
                            .or_default()
                            .insert(frame, AnimationTrackValue::Bool(v));
                    }
                    Some(NodeField::Position3D) => {
                        let state =
                            state_3d
                                .entry(object.clone())
                                .or_insert_with(|| ObjectState3D {
                                    transform: Transform3D::new(
                                        Vector3::ZERO,
                                        Quaternion::IDENTITY,
                                        Vector3::ONE,
                                    ),
                                    visible: true,
                                });
                        state.visible = v;
                        tracks_map
                            .entry((object, TrackKind::Node3DVisible))
                            .or_default()
                            .insert(frame, AnimationTrackValue::Bool(v));
                    }
                    _ => {}
                }
            }
            ObjectFieldAction::Camera3DZoom(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DZoom))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DPerspectiveFovYDegrees(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DPerspectiveFovYDegrees))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DPerspectiveNear(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DPerspectiveNear))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DPerspectiveFar(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DPerspectiveFar))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DOrthographicSize(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DOrthographicSize))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DOrthographicNear(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DOrthographicNear))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DOrthographicFar(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DOrthographicFar))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumLeft(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumLeft))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumRight(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumRight))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumBottom(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumBottom))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumTop(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumTop))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumNear(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumNear))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DFrustumFar(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DFrustumFar))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::Camera3DActive(v) => {
                tracks_map
                    .entry((object, TrackKind::Camera3DActive))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Bool(v));
            }
            ObjectFieldAction::LightColor(v) => {
                tracks_map
                    .entry((object, TrackKind::Light3DColor))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Vec3(v));
            }
            ObjectFieldAction::LightIntensity(v) => {
                tracks_map
                    .entry((object, TrackKind::Light3DIntensity))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::LightActive(v) => {
                tracks_map
                    .entry((object, TrackKind::Light3DActive))
                    .or_default()
                    .insert(frame, AnimationTrackValue::Bool(v));
            }
            ObjectFieldAction::PointLightRange(v) => {
                tracks_map
                    .entry((object, TrackKind::PointLight3DRange))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::SpotLightRange(v) => {
                tracks_map
                    .entry((object, TrackKind::SpotLight3DRange))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::SpotLightInnerAngleRadians(v) => {
                tracks_map
                    .entry((object, TrackKind::SpotLight3DInnerAngleRadians))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
            ObjectFieldAction::SpotLightOuterAngleRadians(v) => {
                tracks_map
                    .entry((object, TrackKind::SpotLight3DOuterAngleRadians))
                    .or_default()
                    .insert(frame, AnimationTrackValue::F32(v));
            }
        }
    }

    let mut object_tracks = Vec::<AnimationObjectTrack>::new();
    for ((object, kind), key_map) in tracks_map {
        let (channel, interpolation) = match kind {
            TrackKind::Node2DTransform => (
                AnimationChannel::Node2D(Node2DChannel::Transform),
                AnimationInterpolation::Step,
            ),
            TrackKind::Node2DVisible => (
                AnimationChannel::Node2D(Node2DChannel::Visible),
                AnimationInterpolation::Step,
            ),
            TrackKind::Node3DTransform => (
                AnimationChannel::Node3D(Node3DChannel::Transform),
                AnimationInterpolation::Step,
            ),
            TrackKind::Node3DVisible => (
                AnimationChannel::Node3D(Node3DChannel::Visible),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DZoom => (
                AnimationChannel::Camera3D(Camera3DChannel::Zoom),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DPerspectiveFovYDegrees => (
                AnimationChannel::Camera3D(Camera3DChannel::PerspectiveFovYDegrees),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DPerspectiveNear => (
                AnimationChannel::Camera3D(Camera3DChannel::PerspectiveNear),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DPerspectiveFar => (
                AnimationChannel::Camera3D(Camera3DChannel::PerspectiveFar),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DOrthographicSize => (
                AnimationChannel::Camera3D(Camera3DChannel::OrthographicSize),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DOrthographicNear => (
                AnimationChannel::Camera3D(Camera3DChannel::OrthographicNear),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DOrthographicFar => (
                AnimationChannel::Camera3D(Camera3DChannel::OrthographicFar),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumLeft => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumLeft),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumRight => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumRight),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumBottom => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumBottom),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumTop => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumTop),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumNear => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumNear),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DFrustumFar => (
                AnimationChannel::Camera3D(Camera3DChannel::FrustumFar),
                AnimationInterpolation::Step,
            ),
            TrackKind::Camera3DActive => (
                AnimationChannel::Camera3D(Camera3DChannel::Active),
                AnimationInterpolation::Step,
            ),
            TrackKind::Light3DColor => (
                AnimationChannel::Light3D(Light3DChannel::Color),
                AnimationInterpolation::Step,
            ),
            TrackKind::Light3DIntensity => (
                AnimationChannel::Light3D(Light3DChannel::Intensity),
                AnimationInterpolation::Step,
            ),
            TrackKind::Light3DActive => (
                AnimationChannel::Light3D(Light3DChannel::Active),
                AnimationInterpolation::Step,
            ),
            TrackKind::PointLight3DRange => (
                AnimationChannel::PointLight3D(PointLight3DChannel::Range),
                AnimationInterpolation::Step,
            ),
            TrackKind::SpotLight3DRange => (
                AnimationChannel::SpotLight3D(SpotLight3DChannel::Range),
                AnimationInterpolation::Step,
            ),
            TrackKind::SpotLight3DInnerAngleRadians => (
                AnimationChannel::SpotLight3D(SpotLight3DChannel::InnerAngleRadians),
                AnimationInterpolation::Step,
            ),
            TrackKind::SpotLight3DOuterAngleRadians => (
                AnimationChannel::SpotLight3D(SpotLight3DChannel::OuterAngleRadians),
                AnimationInterpolation::Step,
            ),
        };
        let mut keys = Vec::<AnimationObjectKey>::new();
        for (frame, value) in key_map {
            keys.push(AnimationObjectKey { frame, value });
        }
        object_tracks.push(AnimationObjectTrack {
            object: object.into(),
            channel,
            interpolation,
            keys: Cow::Owned(keys),
        });
    }

    object_tracks.sort_by(|a, b| a.object.as_ref().cmp(b.object.as_ref()));
    Ok((object_tracks, frame_events))
}

fn parse_emit_signal(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut params = Vec::<AnimationParam>::new();
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "params" => {
                params = parse_params(v, line_no)?;
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: emit_signal requires `name`", line_no))?;

    Ok(AnimationEvent::EmitSignal {
        name: name.into(),
        params: Cow::Owned(params),
    })
}

fn parse_set_var(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut set_value = None::<AnimationParam>;
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "value" => {
                set_value = Some(parse_param(v, line_no)?);
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: set_var requires `name`", line_no))?;
    let set_value =
        set_value.ok_or_else(|| format!("line {}: set_var requires `value`", line_no))?;

    Ok(AnimationEvent::SetVar {
        name: name.into(),
        value: set_value,
    })
}

fn parse_call_method(value: &str, line_no: usize) -> Result<AnimationEvent, String> {
    let value = parse_scene_value(value, line_no)?;
    let fields = as_object(&value).ok_or_else(|| format!("line {}: expected object", line_no))?;

    let mut name = None::<String>;
    let mut params = Vec::<AnimationParam>::new();
    for (k, v) in fields {
        match k.as_ref() {
            "name" => {
                name = as_text(v).map(|s| s.to_string());
            }
            "params" => {
                params = parse_params(v, line_no)?;
            }
            _ => {}
        }
    }
    let name = name.ok_or_else(|| format!("line {}: call_method requires `name`", line_no))?;

    Ok(AnimationEvent::CallMethod {
        name: name.into(),
        params: Cow::Owned(params),
    })
}

fn parse_params(value: &SceneValue, line_no: usize) -> Result<Vec<AnimationParam>, String> {
    let SceneValue::Array(items) = value else {
        return Err(format!("line {}: params must be an array", line_no));
    };

    let mut out = Vec::with_capacity(items.len());
    for item in items.iter() {
        out.push(parse_param(item, line_no)?);
    }
    Ok(out)
}

fn parse_param(value: &SceneValue, line_no: usize) -> Result<AnimationParam, String> {
    match value {
        SceneValue::Bool(v) => Ok(AnimationParam::Bool(*v)),
        SceneValue::I32(v) => Ok(AnimationParam::I32(*v)),
        SceneValue::F32(v) => Ok(AnimationParam::F32(*v)),
        SceneValue::Str(v) => Ok(AnimationParam::String(v.clone())),
        SceneValue::Key(v) => Ok(AnimationParam::String(v.0.clone())),
        SceneValue::Vec2 { x, y } => Ok(AnimationParam::Vec2([*x, *y])),
        SceneValue::Vec3 { x, y, z } => Ok(AnimationParam::Vec3([*x, *y, *z])),
        SceneValue::Vec4 { x, y, z, w } => Ok(AnimationParam::Vec4([*x, *y, *z, *w])),
        SceneValue::Object(fields) => {
            let mut position2 = None;
            let mut rotation2 = None;
            let mut scale2 = None;
            let mut position3 = None;
            let mut rotation3 = None;
            let mut scale3 = None;
            for (k, v) in fields.iter() {
                match k.as_ref() {
                    "position" => {
                        if let Some((x, y)) = v.as_vec2() {
                            position2 = Some(Vector2::new(x, y));
                        }
                        if let Some((x, y, z)) = v.as_vec3() {
                            position3 = Some(Vector3::new(x, y, z));
                        }
                    }
                    "rotation" => {
                        if let Some(r) = v.as_f32() {
                            rotation2 = Some(r);
                        }
                        if let Some((x, y, z, w)) = v.as_vec4() {
                            let mut q = Quaternion::new(x, y, z, w);
                            q.normalize();
                            rotation3 = Some(q);
                        }
                    }
                    "scale" => {
                        if let Some((x, y)) = v.as_vec2() {
                            scale2 = Some(Vector2::new(x, y));
                        }
                        if let Some((x, y, z)) = v.as_vec3() {
                            scale3 = Some(Vector3::new(x, y, z));
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(p), Some(r), Some(s)) = (position2, rotation2, scale2) {
                return Ok(AnimationParam::Transform2D(Transform2D::new(p, r, s)));
            }
            if let (Some(p), Some(r), Some(s)) = (position3, rotation3, scale3) {
                return Ok(AnimationParam::Transform3D(Transform3D::new(p, r, s)));
            }
            Err(format!("line {}: unsupported object param", line_no))
        }
        _ => Err(format!("line {}: unsupported param value", line_no)),
    }
}

fn parse_scene_value(value: &str, line_no: usize) -> Result<SceneValue, String> {
    std::panic::catch_unwind(|| SceneParser::new(value).parse_value_literal())
        .map_err(|_| format!("line {}: invalid value `{}`", line_no, value))
}

fn as_text(value: &SceneValue) -> Option<&str> {
    match value {
        SceneValue::Str(v) => Some(v.as_ref()),
        SceneValue::Key(v) => Some(v.as_ref()),
        _ => None,
    }
}

fn as_object(value: &SceneValue) -> Option<&[(Cow<'static, str>, SceneValue)]> {
    match value {
        SceneValue::Object(fields) => Some(fields.as_ref()),
        _ => None,
    }
}

fn parse_frame_header(line: &str) -> Option<u32> {
    let inner = line.strip_prefix("[Frame")?.strip_suffix(']')?;
    inner.trim().parse::<u32>().ok()
}

fn is_frame_footer(line: &str) -> bool {
    line.starts_with("[/Frame") && line.ends_with(']')
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn strip_comment(line: &str) -> &str {
    let mut cut = line.len();
    if let Some(i) = line.find("//") {
        cut = cut.min(i);
    }
    if let Some(i) = line.find('#') {
        cut = cut.min(i);
    }
    &line[..cut]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sparse_keyframes_and_events() {
        let src = r#"
[Animation]
name = "AttackA"
fps = 30
looping = false
[/Animation]

[Objects]
@Player = MeshInstance3D
[/Objects]

[Frame0]
@Player {
    position = (0,0,0)
    rotation = (0,0,0,1)
    scale = (1,1,1)
    visible = true
}
[/Frame0]

[Frame25]
@Player {
    call_method = { name="slash", params=[1.0] }
}
[/Frame25]
"#;

        let clip = parse_panim(src).expect("expected valid panim");
        assert_eq!(clip.name.as_ref(), "AttackA");
        assert_eq!(clip.fps, 30.0);
        assert!(!clip.looping);
        assert_eq!(clip.total_frames, 26);
        assert_eq!(clip.objects.len(), 1);
        assert_eq!(clip.object_tracks.len(), 2);
        assert_eq!(clip.frame_events.len(), 1);
        assert_eq!(clip.frame_events[0].frame, 25);
    }
}
