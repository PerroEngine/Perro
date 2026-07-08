use crate::{
    AnimationBoneSelector, AnimationBoneTarget, AnimationClip, AnimationObject,
    AnimationObjectTrack,
};
use perro_scene::{NodeField, NodeType, Skeleton3DField};
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimationRetargetReport {
    pub remapped_tracks: u32,
    pub kept_unmapped_tracks: u32,
    pub dropped_tracks: u32,
    pub unsupported_index_tracks: u32,
    pub unmapped_bones: Vec<Cow<'static, str>>,
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

pub fn retarget_skeleton3d_clip(
    clip: &AnimationClip,
    map: &AnimationRetargetMap,
) -> (AnimationClip, AnimationRetargetReport) {
    let aliases = map
        .bones
        .iter()
        .map(|entry| (entry.source.as_ref(), entry.target.as_ref()))
        .collect::<HashMap<_, _>>();
    let mut report = AnimationRetargetReport::default();
    let mut tracks = Vec::with_capacity(clip.object_tracks.len());

    for track in clip.object_tracks.iter() {
        if !is_source_skeleton_track(track, map.source_object.as_ref()) {
            tracks.push(track.clone());
            continue;
        }

        let Some(retargeted) = retarget_track(track, map, &aliases, &mut report) else {
            continue;
        };
        tracks.push(retargeted);
    }

    let mut objects = clip.objects.to_vec();
    ensure_target_object(&mut objects, map.target_object.as_ref());

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
    let mut source_object = None::<Cow<'static, str>>;
    let mut target_object = None::<Cow<'static, str>>;
    let mut keep_unmapped = true;
    let mut bones = Vec::<AnimationBoneRetarget>::new();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
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
                other => return Err(format!("line {line_no}: unknown retarget key `{other}`")),
            }
            continue;
        }
        return Err(format!("line {line_no}: expected key or bone mapping"));
    }

    Ok(AnimationRetargetMap {
        source_object: source_object
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing source_object".to_string())?,
        target_object: target_object
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "missing target_object".to_string())?,
        keep_unmapped,
        bones: Cow::Owned(bones),
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
    map: &AnimationRetargetMap,
    aliases: &HashMap<&str, &str>,
    report: &mut AnimationRetargetReport,
) -> Option<AnimationObjectTrack> {
    let mut out = track.clone();
    out.object = Cow::Owned(map.target_object.to_string());

    let bone_target = match &track.bone_target {
        Some(AnimationBoneTarget {
            selector: AnimationBoneSelector::Name(source),
        }) => {
            if let Some(target) = aliases.get(source.as_ref()) {
                report.remapped_tracks += 1;
                Some(AnimationBoneTarget {
                    selector: AnimationBoneSelector::Name((*target).to_string().into()),
                })
            } else if map.keep_unmapped {
                report.kept_unmapped_tracks += 1;
                report.unmapped_bones.push(Cow::Owned(source.to_string()));
                Some(AnimationBoneTarget {
                    selector: AnimationBoneSelector::Name(source.to_string().into()),
                })
            } else {
                report.dropped_tracks += 1;
                report.unmapped_bones.push(Cow::Owned(source.to_string()));
                None
            }
        }
        Some(AnimationBoneTarget {
            selector: AnimationBoneSelector::Index(_),
        }) => {
            report.unsupported_index_tracks += 1;
            if map.keep_unmapped {
                track.bone_target.clone()
            } else {
                report.dropped_tracks += 1;
                None
            }
        }
        None => None,
    }?;

    out.bone_target = Some(bone_target);
    Some(out)
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
