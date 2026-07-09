use crate::{parse_flag_value, resolve_local_path};
use perro_animation::{
    ANIMATION_TRANSFORM_MASK_POSITION, ANIMATION_TRANSFORM_MASK_ROTATION,
    ANIMATION_TRANSFORM_MASK_SCALE, AnimationBoneSelector, AnimationClip, AnimationKeyMode,
    AnimationTrackValue,
};
use perro_scene::{Node2DField, Node3DField, NodeField, Skeleton2DField, Skeleton3DField};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct TrackTarget {
    object: String,
    prop: String,
}

#[derive(Default)]
struct FrameBlock {
    tracks: BTreeMap<TrackTarget, String>,
}

pub(crate) fn gltf_to_panim_command(args: &[String], cwd: &Path) -> Result<(), String> {
    let Some(raw_input) = parse_flag_value(args, "--input")
        .or_else(|| parse_flag_value(args, "--in"))
        .or_else(|| args.get(2).filter(|arg| !arg.starts_with("--")).cloned())
    else {
        return Err("missing input path".to_string());
    };
    let Some(raw_output) =
        parse_flag_value(args, "--output").or_else(|| parse_flag_value(args, "--out"))
    else {
        return Err("missing required flag `--output`".to_string());
    };

    let input_path = resolve_local_path(&raw_input, cwd);
    let output_path = resolve_local_path(&raw_output, cwd);
    let fps = parse_flag_value(args, "--fps")
        .map(|raw| {
            raw.parse::<f32>()
                .map_err(|_| format!("invalid --fps `{raw}`"))
        })
        .transpose()?
        .unwrap_or(60.0);
    if !fps.is_finite() || fps <= 0.0 {
        return Err("--fps must be a positive finite number".to_string());
    }

    let clip_selector = parse_flag_value(args, "--clip");
    let skeleton_object = parse_flag_value(args, "--skeleton")
        .map(|name| sanitize_ident(&name))
        .unwrap_or_else(|| "Rig".to_string());

    let mut panim = convert_gltf_animation_to_panim(
        &input_path,
        fps,
        clip_selector.as_deref(),
        &skeleton_object,
    )?;
    if let Some(raw_map) =
        parse_flag_value(args, "--retarget-map").or_else(|| parse_flag_value(args, "--retarget"))
    {
        let map_path = resolve_local_path(&raw_map, cwd);
        let map_text = std::fs::read_to_string(&map_path)
            .map_err(|err| format!("failed to read {}: {err}", map_path.display()))?;
        let map = perro_animation::parse_pretarget(&map_text)?;
        let clip = perro_animation::parse_panim(&panim)?;
        let (retargeted, report) = perro_animation::retarget_skeleton3d_clip(&clip, &map);
        if report.remapped_tracks == 0 && report.kept_unmapped_tracks == 0 {
            return Err("retarget map matched no Skeleton3D bone tracks".to_string());
        }
        panim = render_clip_to_panim(&retargeted)?;
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    std::fs::write(&output_path, panim)
        .map_err(|err| format!("failed to write {}: {err}", output_path.display()))?;
    println!("created animation at {}", output_path.display());
    Ok(())
}

fn convert_gltf_animation_to_panim(
    input_path: &Path,
    fps: f32,
    clip_selector: Option<&str>,
    skeleton_object: &str,
) -> Result<String, String> {
    let (doc, buffers, _images) = gltf::import(input_path)
        .map_err(|err| format!("failed to import glTF `{}`: {err}", input_path.display()))?;
    let animation = select_animation(&doc, clip_selector)?;
    let animation_name = animation
        .name()
        .map(sanitize_display)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("Animation{}", animation.index()));

    let joint_nodes = collect_joint_nodes(&doc);
    let node_names = doc
        .nodes()
        .map(|node| {
            (
                node.index(),
                node.name()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("Node{}", node.index())),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut frames = BTreeMap::<u32, FrameBlock>::new();
    let mut objects = BTreeMap::<String, String>::new();
    let mut used_object_names = BTreeSet::<String>::from([skeleton_object.to_string()]);
    let mut object_name_by_node = HashMap::<usize, String>::new();

    for channel in animation.channels() {
        let target = channel.target();
        if target.property() == gltf::animation::Property::MorphTargetWeights {
            continue;
        }
        let node = target.node();
        let node_index = node.index();
        let node_name = node_names
            .get(&node_index)
            .cloned()
            .unwrap_or_else(|| format!("Node{node_index}"));
        let (object, prop) = if joint_nodes.contains(&node_index) {
            objects.insert(skeleton_object.to_string(), "Skeleton3D".to_string());
            (
                skeleton_object.to_string(),
                format!(
                    "bone[\"{}\"].{}",
                    escape_str(&node_name),
                    target_property_name(&target)
                ),
            )
        } else {
            let object = object_name_by_node
                .entry(node_index)
                .or_insert_with(|| unique_ident(&node_name, &mut used_object_names))
                .clone();
            objects.insert(object.clone(), "Node3D".to_string());
            (object, target_property_name(&target).to_string())
        };

        let reader = channel.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
        let sampler = channel.sampler();
        let value_step = match sampler.interpolation() {
            gltf::animation::Interpolation::CubicSpline => 3,
            gltf::animation::Interpolation::Linear | gltf::animation::Interpolation::Step => 1,
        };
        let value_offset = if value_step == 3 { 1 } else { 0 };
        let inputs = reader
            .read_inputs()
            .ok_or_else(|| format!("animation channel on `{node_name}` lacks input times"))?
            .collect::<Vec<f32>>();
        if inputs.is_empty() {
            continue;
        }

        match reader
            .read_outputs()
            .ok_or_else(|| format!("animation channel on `{node_name}` lacks output values"))?
        {
            gltf::animation::util::ReadOutputs::Translations(values) => {
                let values = values.collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    let Some(value) = values.get(index * value_step + value_offset).copied() else {
                        continue;
                    };
                    insert_track(&mut frames, time, fps, &object, &prop, vec3_value(value));
                }
            }
            gltf::animation::util::ReadOutputs::Rotations(values) => {
                let values = values.into_f32().collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    let Some(value) = values.get(index * value_step + value_offset).copied() else {
                        continue;
                    };
                    insert_track(&mut frames, time, fps, &object, &prop, quat_value(value));
                }
            }
            gltf::animation::util::ReadOutputs::Scales(values) => {
                let values = values.collect::<Vec<_>>();
                for (index, time) in inputs.iter().copied().enumerate() {
                    let Some(value) = values.get(index * value_step + value_offset).copied() else {
                        continue;
                    };
                    insert_track(&mut frames, time, fps, &object, &prop, vec3_value(value));
                }
            }
            gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => {}
        }
    }

    if frames.is_empty() {
        return Err(format!(
            "animation `{animation_name}` contains no translation/rotation/scale tracks"
        ));
    }

    render_panim(&animation_name, fps, &objects, &frames)
}

fn select_animation<'a>(
    doc: &'a gltf::Document,
    clip_selector: Option<&str>,
) -> Result<gltf::Animation<'a>, String> {
    let selector = clip_selector.unwrap_or("0").trim();
    if let Ok(index) = selector.parse::<usize>() {
        return doc
            .animations()
            .nth(index)
            .ok_or_else(|| format!("animation index {index} not found"));
    }
    doc.animations()
        .find(|animation| animation.name() == Some(selector))
        .ok_or_else(|| format!("animation `{selector}` not found"))
}

fn collect_joint_nodes(doc: &gltf::Document) -> HashSet<usize> {
    let mut joints = HashSet::new();
    for skin in doc.skins() {
        for joint in skin.joints() {
            joints.insert(joint.index());
        }
    }
    joints
}

fn target_property_name(target: &gltf::animation::Target) -> &'static str {
    match target.property() {
        gltf::animation::Property::Translation => "position",
        gltf::animation::Property::Rotation => "rotation",
        gltf::animation::Property::Scale => "scale",
        gltf::animation::Property::MorphTargetWeights => unreachable!(),
    }
}

fn insert_track(
    frames: &mut BTreeMap<u32, FrameBlock>,
    time: f32,
    fps: f32,
    object: &str,
    prop: &str,
    value: String,
) {
    if !time.is_finite() {
        return;
    }
    let frame = (time * fps).round().max(0.0) as u32;
    frames.entry(frame).or_default().tracks.insert(
        TrackTarget {
            object: object.to_string(),
            prop: prop.to_string(),
        },
        value,
    );
}

fn render_panim(
    animation_name: &str,
    fps: f32,
    objects: &BTreeMap<String, String>,
    frames: &BTreeMap<u32, FrameBlock>,
) -> Result<String, String> {
    let mut out = String::new();
    let _ = writeln!(out, "[Animation]");
    let _ = writeln!(out, "name = \"{}\"", escape_str(animation_name));
    let _ = writeln!(out, "fps = {}", fmt_f32(fps));
    let _ = writeln!(out, "default_interp = \"interpolate\"");
    let _ = writeln!(out, "default_ease = \"linear\"");
    let _ = writeln!(out, "[/Animation]\n");
    let _ = writeln!(out, "[Objects]");
    for (object, node_type) in objects {
        let _ = writeln!(out, "{object} = {node_type}");
    }
    let _ = writeln!(out, "[/Objects]\n");

    for (frame, block) in frames {
        let _ = writeln!(out, "[Frame{frame}]");
        let mut props_by_object = BTreeMap::<&str, Vec<(&str, &str)>>::new();
        for (target, value) in &block.tracks {
            props_by_object
                .entry(&target.object)
                .or_default()
                .push((&target.prop, value));
        }
        for (object, props) in props_by_object {
            let _ = writeln!(out, "@{object} {{");
            for (prop, value) in props {
                let _ = writeln!(out, "    {prop} = {value}");
            }
            let _ = writeln!(out, "}}");
        }
        let _ = writeln!(out, "[/Frame{frame}]\n");
    }

    Ok(out)
}

fn render_clip_to_panim(clip: &AnimationClip) -> Result<String, String> {
    let mut frames = BTreeMap::<u32, FrameBlock>::new();
    let mut objects = BTreeMap::<String, String>::new();
    for object in clip.objects.iter() {
        objects.insert(
            object.name.to_string(),
            object.node_type.as_str().to_string(),
        );
    }
    for track in clip.object_tracks.iter() {
        for key in track.keys.iter() {
            if key.mode != AnimationKeyMode::Closed {
                continue;
            }
            for (prop, value) in track_key_values(track, &key.value)? {
                frames.entry(key.frame).or_default().tracks.insert(
                    TrackTarget {
                        object: track.object.to_string(),
                        prop,
                    },
                    value,
                );
            }
        }
    }
    render_panim(clip.name.as_ref(), clip.fps, &objects, &frames)
}

fn track_key_values(
    track: &perro_animation::AnimationObjectTrack,
    value: &AnimationTrackValue,
) -> Result<Vec<(String, String)>, String> {
    if let Some(target) = &track.bone_target {
        let bone = match &target.selector {
            AnimationBoneSelector::Name(name) => format!("bone[\"{}\"]", escape_str(name)),
            AnimationBoneSelector::Index(index) => format!("bone[{index}]"),
        };
        return transform_key_values(&bone, track.transform3d_mask, value);
    }

    match &track.field {
        NodeField::Node3D(Node3DField::Position)
        | NodeField::Skeleton3D(Skeleton3DField::Skeleton) => {
            transform_key_values("", track.transform3d_mask, value)
        }
        NodeField::Node2D(Node2DField::Position)
        | NodeField::Skeleton2D(Skeleton2DField::Skeleton) => {
            transform2d_key_values("", track.transform2d_mask, value)
        }
        NodeField::Node3D(Node3DField::Visible) | NodeField::Node2D(Node2DField::Visible) => {
            match value {
                AnimationTrackValue::Bool(value) => {
                    Ok(vec![("visible".to_string(), value.to_string())])
                }
                _ => Err("visible track needs bool value".to_string()),
            }
        }
        _ => Ok(Vec::new()),
    }
}

fn transform_key_values(
    prefix: &str,
    mask: u8,
    value: &AnimationTrackValue,
) -> Result<Vec<(String, String)>, String> {
    let AnimationTrackValue::Transform3D(transform) = value else {
        return Err("3D transform track needs Transform3D value".to_string());
    };
    let prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}.")
    };
    let mask = if mask == 0 {
        ANIMATION_TRANSFORM_MASK_POSITION
            | ANIMATION_TRANSFORM_MASK_ROTATION
            | ANIMATION_TRANSFORM_MASK_SCALE
    } else {
        mask
    };
    let mut out = Vec::new();
    if mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        out.push((
            format!("{prefix}position"),
            vec3_value(transform.position.to_array()),
        ));
    }
    if mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        out.push((
            format!("{prefix}rotation"),
            quat_value([
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            ]),
        ));
    }
    if mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        out.push((
            format!("{prefix}scale"),
            vec3_value(transform.scale.to_array()),
        ));
    }
    Ok(out)
}

fn transform2d_key_values(
    prefix: &str,
    mask: u8,
    value: &AnimationTrackValue,
) -> Result<Vec<(String, String)>, String> {
    let AnimationTrackValue::Transform2D(transform) = value else {
        return Err("2D transform track needs Transform2D value".to_string());
    };
    let prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}.")
    };
    let mask = if mask == 0 {
        ANIMATION_TRANSFORM_MASK_POSITION
            | ANIMATION_TRANSFORM_MASK_ROTATION
            | ANIMATION_TRANSFORM_MASK_SCALE
    } else {
        mask
    };
    let mut out = Vec::new();
    if mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
        out.push((
            format!("{prefix}position"),
            format!(
                "({}, {})",
                fmt_f32(transform.position.x),
                fmt_f32(transform.position.y)
            ),
        ));
    }
    if mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
        out.push((format!("{prefix}rotation"), fmt_f32(transform.rotation)));
    }
    if mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
        out.push((
            format!("{prefix}scale"),
            format!(
                "({}, {})",
                fmt_f32(transform.scale.x),
                fmt_f32(transform.scale.y)
            ),
        ));
    }
    Ok(out)
}

fn sanitize_display(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

fn unique_ident(raw: &str, used: &mut BTreeSet<String>) -> String {
    let base = sanitize_ident(raw);
    if used.insert(base.clone()) {
        return base;
    }
    let mut index = 1usize;
    loop {
        let candidate = format!("{base}_{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn sanitize_ident(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.trim().chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if c.is_whitespace() || c == '-' || c == '.' {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("Object");
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn vec3_value(v: [f32; 3]) -> String {
    format!("({}, {}, {})", fmt_f32(v[0]), fmt_f32(v[1]), fmt_f32(v[2]))
}

fn quat_value(v: [f32; 4]) -> String {
    format!(
        "({}, {}, {}, {})",
        fmt_f32(v[0]),
        fmt_f32(v[1]),
        fmt_f32(v[2]),
        fmt_f32(v[3])
    )
}

fn fmt_f32(value: f32) -> String {
    if value == 0.0 {
        return "0.0".to_string();
    }
    let mut out = format!("{value:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.push('0');
    }
    out
}

fn escape_str(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}
