use crate::{
    ANIMATION_TRANSFORM_MASK_POSITION, ANIMATION_TRANSFORM_MASK_ROTATION,
    ANIMATION_TRANSFORM_MASK_SCALE, AnimationBoneSelector, AnimationBoneTarget, AnimationClip,
    AnimationObject, AnimationObjectTrack, AnimationTrackValue,
};
use perro_scene::{NodeField, NodeType, Skeleton3DField};
use perro_structs::{Quaternion, Transform3D, Vector3};
use std::{borrow::Cow, collections::HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationBoneRetarget {
    pub source: Cow<'static, str>,
    pub target: Cow<'static, str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimationRetargetMap {
    pub source_object: Cow<'static, str>,
    pub target_object: Cow<'static, str>,
    pub keep_unmapped: bool,
    pub bones: Cow<'static, [AnimationBoneRetarget]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnimationTranslationPolicy {
    #[default]
    All,
    RootOnly,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationBoneRestPose {
    pub bone: Cow<'static, str>,
    pub transform: Transform3D,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationRetargetProfile {
    pub map: AnimationRetargetMap,
    pub translation_policy: AnimationTranslationPolicy,
    pub root_bone: Option<Cow<'static, str>>,
    pub source_rest: Cow<'static, [AnimationBoneRestPose]>,
    pub target_rest: Cow<'static, [AnimationBoneRestPose]>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimationRetargetReport {
    pub remapped_tracks: u32,
    pub kept_unmapped_tracks: u32,
    pub dropped_tracks: u32,
    pub unsupported_index_tracks: u32,
    pub unmapped_bones: Vec<Cow<'static, str>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimationRetargetSolveReport {
    pub map: AnimationRetargetReport,
    pub aligned_tracks: u32,
    pub translation_locked_tracks: u32,
    pub translation_dropped_tracks: u32,
    pub missing_rest_bones: Vec<Cow<'static, str>>,
}

impl Default for AnimationRetargetMap {
    fn default() -> Self {
        Self {
            source_object: Cow::Borrowed(""),
            target_object: Cow::Borrowed(""),
            keep_unmapped: true,
            bones: Cow::Borrowed(&[]),
        }
    }
}

impl Default for AnimationRetargetProfile {
    fn default() -> Self {
        Self {
            map: AnimationRetargetMap::default(),
            translation_policy: AnimationTranslationPolicy::All,
            root_bone: None,
            source_rest: Cow::Borrowed(&[]),
            target_rest: Cow::Borrowed(&[]),
        }
    }
}

pub fn retarget_skeleton3d_clip(
    clip: &AnimationClip,
    map: &AnimationRetargetMap,
) -> (AnimationClip, AnimationRetargetReport) {
    let profile = AnimationRetargetProfile {
        map: map.clone(),
        ..AnimationRetargetProfile::default()
    };
    let (clip, report) = retarget_skeleton3d_clip_with_profile(clip, &profile);
    (clip, report.map)
}

pub fn retarget_skeleton3d_clip_with_profile(
    clip: &AnimationClip,
    profile: &AnimationRetargetProfile,
) -> (AnimationClip, AnimationRetargetSolveReport) {
    let aliases = profile
        .map
        .bones
        .iter()
        .map(|entry| (entry.source.as_ref(), entry.target.as_ref()))
        .collect::<HashMap<_, _>>();
    let source_rest = rest_pose_lookup(profile.source_rest.as_ref());
    let target_rest = rest_pose_lookup(profile.target_rest.as_ref());
    let mut report = AnimationRetargetSolveReport::default();
    let mut tracks = Vec::with_capacity(clip.object_tracks.len());

    for track in clip.object_tracks.iter() {
        if !is_source_skeleton_track(track, profile.map.source_object.as_ref()) {
            tracks.push(track.clone());
            continue;
        }

        let Some(retargeted) = retarget_track(
            track,
            profile,
            &aliases,
            &source_rest,
            &target_rest,
            &mut report,
        ) else {
            continue;
        };
        tracks.push(retargeted);
    }

    let mut objects = clip.objects.to_vec();
    ensure_target_object(&mut objects, profile.map.target_object.as_ref());

    report.missing_rest_bones.sort_unstable();
    report.missing_rest_bones.dedup();

    (
        AnimationClip {
            name: clip.name.clone(),
            fps: clip.fps,
            total_frames: clip.total_frames,
            objects: Cow::Owned(objects),
            object_tracks: Cow::Owned(tracks),
            frame_events: Cow::Owned(clip.frame_events.to_vec()),
        },
        report,
    )
}

pub fn parse_pretarget(source: &str) -> Result<AnimationRetargetMap, String> {
    parse_pretarget_profile(source).map(|profile| profile.map)
}

pub fn parse_pretarget_profile(source: &str) -> Result<AnimationRetargetProfile, String> {
    let mut source_object = None::<Cow<'static, str>>;
    let mut target_object = None::<Cow<'static, str>>;
    let mut keep_unmapped = true;
    let mut translation_policy = AnimationTranslationPolicy::All;
    let mut root_bone = None::<Cow<'static, str>>;
    let mut bones = Vec::<AnimationBoneRetarget>::new();
    let mut source_rest = Vec::<AnimationBoneRestPose>::new();
    let mut target_rest = Vec::<AnimationBoneRestPose>::new();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = parse_rest_line(line, "source_rest", line_no)? {
            source_rest.push(rest);
            continue;
        }
        if let Some(rest) = parse_rest_line(line, "target_rest", line_no)? {
            target_rest.push(rest);
            continue;
        }
        let mapping = line.strip_prefix("bone ").unwrap_or(line);
        if let Some((source, target)) = mapping.split_once("=>") {
            let source = parse_text(source);
            let target = parse_text(target);
            if source.is_empty() || target.is_empty() {
                return Err(format!("line {line_no}: bone mapping cannot be empty"));
            }
            bones.push(AnimationBoneRetarget {
                source: source.into(),
                target: target.into(),
            });
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "source" | "source_object" => source_object = Some(parse_text(value).into()),
                "target" | "target_object" => target_object = Some(parse_text(value).into()),
                "keep_unmapped" => keep_unmapped = parse_bool(value, line_no)?,
                "translation" | "translation_policy" => {
                    translation_policy = parse_translation_policy(value, line_no)?
                }
                "root" | "root_bone" => root_bone = Some(parse_text(value).into()),
                other => return Err(format!("line {line_no}: unknown retarget key `{other}`")),
            }
            continue;
        }
        return Err(format!("line {line_no}: expected key or bone mapping"));
    }

    if translation_policy == AnimationTranslationPolicy::RootOnly
        && root_bone.as_ref().is_none_or(|bone| bone.is_empty())
    {
        return Err("translation=root_only needs root_bone".to_string());
    }

    Ok(AnimationRetargetProfile {
        map: AnimationRetargetMap {
            source_object: source_object
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "missing source_object".to_string())?,
            target_object: target_object
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "missing target_object".to_string())?,
            keep_unmapped,
            bones: Cow::Owned(bones),
        },
        translation_policy,
        root_bone,
        source_rest: Cow::Owned(source_rest),
        target_rest: Cow::Owned(target_rest),
    })
}

fn is_source_skeleton_track(track: &AnimationObjectTrack, source_object: &str) -> bool {
    track.object.as_ref() == source_object
        && matches!(
            &track.field,
            NodeField::Skeleton3D(Skeleton3DField::Skeleton)
        )
        && track.bone_target.is_some()
}

fn retarget_track(
    track: &AnimationObjectTrack,
    profile: &AnimationRetargetProfile,
    aliases: &HashMap<&str, &str>,
    source_rest: &HashMap<&str, Transform3D>,
    target_rest: &HashMap<&str, Transform3D>,
    report: &mut AnimationRetargetSolveReport,
) -> Option<AnimationObjectTrack> {
    let mut out = track.clone();
    out.object = Cow::Owned(profile.map.target_object.to_string());

    let (bone_target, source_name, target_name) = match &track.bone_target {
        Some(AnimationBoneTarget {
            selector: AnimationBoneSelector::Name(source),
        }) => {
            if let Some(target) = aliases.get(source.as_ref()) {
                report.map.remapped_tracks += 1;
                (
                    AnimationBoneTarget {
                        selector: AnimationBoneSelector::Name((*target).to_string().into()),
                    },
                    source.as_ref(),
                    *target,
                )
            } else if profile.map.keep_unmapped {
                report.map.kept_unmapped_tracks += 1;
                report
                    .map
                    .unmapped_bones
                    .push(Cow::Owned(source.to_string()));
                (
                    AnimationBoneTarget {
                        selector: AnimationBoneSelector::Name(source.to_string().into()),
                    },
                    source.as_ref(),
                    source.as_ref(),
                )
            } else {
                report.map.dropped_tracks += 1;
                report
                    .map
                    .unmapped_bones
                    .push(Cow::Owned(source.to_string()));
                return None;
            }
        }
        Some(AnimationBoneTarget {
            selector: AnimationBoneSelector::Index(_),
        }) => {
            report.map.unsupported_index_tracks += 1;
            if profile.map.keep_unmapped {
                out.bone_target = track.bone_target.clone();
                return Some(out);
            }
            report.map.dropped_tracks += 1;
            return None;
        }
        None => return None,
    };

    out.bone_target = Some(bone_target);
    align_track_rest_pose(
        &mut out,
        source_name,
        target_name,
        profile,
        source_rest,
        target_rest,
        report,
    )?;
    Some(out)
}

fn align_track_rest_pose(
    track: &mut AnimationObjectTrack,
    source_name: &str,
    target_name: &str,
    profile: &AnimationRetargetProfile,
    source_rest: &HashMap<&str, Transform3D>,
    target_rest: &HashMap<&str, Transform3D>,
    report: &mut AnimationRetargetSolveReport,
) -> Option<()> {
    let source_pose = source_rest.get(source_name).copied();
    let target_pose = target_rest.get(target_name).copied();
    if source_pose.is_some() != target_pose.is_some() {
        report
            .missing_rest_bones
            .push(Cow::Owned(format!("{source_name}=>{target_name}")));
    }

    if let (Some(source_pose), Some(target_pose)) = (source_pose, target_pose) {
        let mask = normalized_transform3d_mask(track.transform3d_mask);
        for key in track.keys.to_mut() {
            let AnimationTrackValue::Transform3D(animated) = &mut key.value else {
                continue;
            };
            if mask & ANIMATION_TRANSFORM_MASK_POSITION != 0 {
                animated.position = align_position(animated.position, source_pose, target_pose);
            }
            if mask & ANIMATION_TRANSFORM_MASK_ROTATION != 0 {
                animated.rotation = animated.rotation.normalized();
            }
            if mask & ANIMATION_TRANSFORM_MASK_SCALE != 0 {
                animated.scale = align_scale(animated.scale, source_pose.scale, target_pose.scale);
            }
        }
        report.aligned_tracks += 1;
    }

    let keep_translation = match profile.translation_policy {
        AnimationTranslationPolicy::All => true,
        AnimationTranslationPolicy::RootOnly => profile
            .root_bone
            .as_deref()
            .is_some_and(|root| root == source_name),
        AnimationTranslationPolicy::None => false,
    };
    if keep_translation {
        return Some(());
    }

    let mask = normalized_transform3d_mask(track.transform3d_mask);
    if mask & ANIMATION_TRANSFORM_MASK_POSITION == 0 {
        return Some(());
    }
    let remaining = mask & !ANIMATION_TRANSFORM_MASK_POSITION;
    if remaining == 0 {
        report.translation_dropped_tracks += 1;
        return None;
    }
    track.transform3d_mask = remaining;
    let rest_position = target_pose.unwrap_or(Transform3D::IDENTITY).position;
    for key in track.keys.to_mut() {
        if let AnimationTrackValue::Transform3D(transform) = &mut key.value {
            transform.position = rest_position;
        }
    }
    report.translation_locked_tracks += 1;
    Some(())
}

fn normalized_transform3d_mask(mask: u8) -> u8 {
    if mask == 0 {
        ANIMATION_TRANSFORM_MASK_POSITION
            | ANIMATION_TRANSFORM_MASK_ROTATION
            | ANIMATION_TRANSFORM_MASK_SCALE
    } else {
        mask
    }
}

fn align_position(position: Vector3, source: Transform3D, target: Transform3D) -> Vector3 {
    let offset = position - source.position;
    let unrotated = source.rotation.inverse().rotate_vector3(offset);
    let source_local = Vector3::new(
        safe_ratio(unrotated.x, source.scale.x),
        safe_ratio(unrotated.y, source.scale.y),
        safe_ratio(unrotated.z, source.scale.z),
    );
    let target_local = Vector3::new(
        source_local.x * target.scale.x,
        source_local.y * target.scale.y,
        source_local.z * target.scale.z,
    );
    target.position + target.rotation.rotate_vector3(target_local)
}

fn align_scale(scale: Vector3, source: Vector3, target: Vector3) -> Vector3 {
    Vector3::new(
        safe_ratio(scale.x, source.x) * target.x,
        safe_ratio(scale.y, source.y) * target.y,
        safe_ratio(scale.z, source.z) * target.z,
    )
}

fn safe_ratio(value: f32, divisor: f32) -> f32 {
    if divisor.abs() <= 1.0e-8 {
        value
    } else {
        value / divisor
    }
}

fn rest_pose_lookup(poses: &[AnimationBoneRestPose]) -> HashMap<&str, Transform3D> {
    poses
        .iter()
        .map(|pose| (pose.bone.as_ref(), pose.transform))
        .collect()
}

fn ensure_target_object(objects: &mut Vec<AnimationObject>, target_object: &str) {
    if objects
        .iter()
        .any(|object| object.name.as_ref() == target_object)
    {
        return;
    }
    objects.push(AnimationObject {
        name: Cow::Owned(target_object.to_string()),
        node_type: NodeType::Skeleton3D,
    });
}

fn parse_rest_line(
    line: &str,
    prefix: &str,
    line_no: usize,
) -> Result<Option<AnimationBoneRestPose>, String> {
    let Some(rest) = line.strip_prefix(prefix) else {
        return Ok(None);
    };
    if !rest.starts_with(char::is_whitespace) {
        return Ok(None);
    }
    let Some((bone, value)) = rest.trim().split_once('=') else {
        return Err(format!("line {line_no}: {prefix} needs `bone = pose`"));
    };
    let bone = parse_text(bone);
    if bone.is_empty() {
        return Err(format!("line {line_no}: {prefix} bone cannot be empty"));
    }
    Ok(Some(AnimationBoneRestPose {
        bone: bone.into(),
        transform: parse_rest_transform(value, line_no)?,
    }))
}

fn parse_rest_transform(value: &str, line_no: usize) -> Result<Transform3D, String> {
    let parts = value.split('|').map(str::trim).collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return Err(format!(
            "line {line_no}: rest pose needs `position | rotation_quat | scale`"
        ));
    }
    let position = parse_f32_list::<3>(parts[0], line_no, "position")?;
    let rotation = parse_f32_list::<4>(parts[1], line_no, "rotation")?;
    let scale = if parts.len() == 3 {
        parse_f32_list::<3>(parts[2], line_no, "scale")?
    } else {
        [1.0; 3]
    };
    let rotation = Quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]);
    let length_sq = rotation.x * rotation.x
        + rotation.y * rotation.y
        + rotation.z * rotation.z
        + rotation.w * rotation.w;
    if !length_sq.is_finite() || length_sq <= 1.0e-8 {
        return Err(format!(
            "line {line_no}: rest rotation needs finite non-zero quat"
        ));
    }
    Ok(Transform3D::new(
        Vector3::from(position),
        rotation.normalized(),
        Vector3::from(scale),
    ))
}

fn parse_f32_list<const N: usize>(
    value: &str,
    line_no: usize,
    label: &str,
) -> Result<[f32; N], String> {
    let value = value
        .trim()
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or(value.trim());
    let values = value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .map_err(|_| format!("line {line_no}: invalid {label} num `{}`", part.trim()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|values: Vec<f32>| {
        format!(
            "line {line_no}: {label} needs {N} nums, got {}",
            values.len()
        )
    })
}

fn parse_translation_policy(
    value: &str,
    line_no: usize,
) -> Result<AnimationTranslationPolicy, String> {
    match parse_text(value).to_ascii_lowercase().as_str() {
        "all" | "keep" => Ok(AnimationTranslationPolicy::All),
        "root" | "root_only" => Ok(AnimationTranslationPolicy::RootOnly),
        "none" | "drop" => Ok(AnimationTranslationPolicy::None),
        other => Err(format!(
            "line {line_no}: invalid translation policy `{other}`"
        )),
    }
}

fn parse_text(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn parse_bool(value: &str, line_no: usize) -> Result<bool, String> {
    match parse_text(value).as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        other => Err(format!("line {line_no}: invalid bool `{other}`")),
    }
}
