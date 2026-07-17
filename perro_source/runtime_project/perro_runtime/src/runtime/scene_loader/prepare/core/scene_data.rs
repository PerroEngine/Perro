use super::*;

pub(crate) struct PreparedScene {
    pub(in super::super) root_key: Option<u32>,
    pub(in super::super) nodes: Vec<PendingNode>,
    pub(in super::super) scripts: Vec<PendingScript>,
}

impl Clone for PreparedScene {
    fn clone(&self) -> Self {
        let keep_key_names = !self.scripts.is_empty();
        Self {
            root_key: self.root_key,
            nodes: self
                .nodes
                .iter()
                .map(|node| node.clone_for_spawn(keep_key_names))
                .collect(),
            scripts: self.scripts.clone(),
        }
    }
}

#[derive(Clone)]
pub(in super::super) struct PendingScript {
    pub(in super::super) node_key: u32,
    #[cfg(test)]
    pub(in super::super) node_key_name: String,
    pub(in super::super) script_path_hash: u64,
    pub(in super::super) script_mount: Option<String>,
    pub(in super::super) scene_injected_vars: Vec<(String, SceneValue)>,
}

pub(in super::super) struct PendingNode {
    pub(in super::super) key: u32,
    pub(in super::super) key_name: String,
    pub(in super::super) parent_key: Option<u32>,
    pub(in super::super) node: SceneNode,
    pub(in super::super) animation_source: Option<String>,
    pub(in super::super) animation_tree_source: Option<String>,
    pub(in super::super) animation_tree_animations: Vec<PendingAnimationTreeAnimation>,
    pub(in super::super) texture_source: Option<String>,
    // Decal3D [albedo, normal, emission] paths; resolved to TextureIDs at merge.
    pub(in super::super) decal_texture_sources: [Option<String>; 3],
    pub(in super::super) mesh_source: Option<String>,
    pub(in super::super) material_surfaces: Vec<PendingSurfaceMaterial>,
    pub(in super::super) skeleton_source: Option<String>,
    pub(in super::super) bone_pose_overrides: Vec<PendingBonePoseOverride>,
    pub(in super::super) mesh_skeleton_target: Option<u32>,
    pub(in super::super) bone_attachment_skeleton_target: Option<u32>,
    pub(in super::super) ik_target_skeleton_target: Option<u32>,
    pub(in super::super) physics_bone_chain_skeleton_target: Option<u32>,
    pub(in super::super) camera_stream_target: Option<u32>,
    pub(in super::super) joint_body_links: Vec<PendingJointBodyLink>,
    pub(in super::super) animation_bindings: Vec<(String, u32)>,
    pub(in super::super) locale_text_bindings: Vec<PendingLocaleTextBinding>,
}

impl PendingNode {
    fn clone_for_spawn(&self, keep_key_name: bool) -> Self {
        Self {
            key: self.key,
            key_name: if keep_key_name {
                self.key_name.clone()
            } else {
                String::new()
            },
            parent_key: self.parent_key,
            node: self.node.clone(),
            animation_source: self.animation_source.clone(),
            animation_tree_source: self.animation_tree_source.clone(),
            animation_tree_animations: self.animation_tree_animations.clone(),
            texture_source: self.texture_source.clone(),
            decal_texture_sources: self.decal_texture_sources.clone(),
            mesh_source: self.mesh_source.clone(),
            material_surfaces: self.material_surfaces.clone(),
            skeleton_source: self.skeleton_source.clone(),
            bone_pose_overrides: self.bone_pose_overrides.clone(),
            mesh_skeleton_target: self.mesh_skeleton_target,
            bone_attachment_skeleton_target: self.bone_attachment_skeleton_target,
            ik_target_skeleton_target: self.ik_target_skeleton_target,
            physics_bone_chain_skeleton_target: self.physics_bone_chain_skeleton_target,
            camera_stream_target: self.camera_stream_target,
            joint_body_links: self.joint_body_links.clone(),
            animation_bindings: self.animation_bindings.clone(),
            locale_text_bindings: self.locale_text_bindings.clone(),
        }
    }
}

/// Per-bone pose override authored on a Skeleton2D/Skeleton3D scene node:
///
/// ```text
/// bones = {
///     Spine = { position = (0, 1.2, 0), rotation = (0, 0, 0, 1) },
///     Head = { rotation_deg = (0, 30, 0) }
/// }
/// ```
///
/// Only authored components override; the rest of the pose keeps the value
/// loaded from the rig asset. Applied to `pose` (never `rest`) after the
/// skeleton's bones load, so animation tracks can still take over animated
/// bones while un-animated bones keep their scene override.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PendingBonePoseOverride {
    pub(crate) bone: String,
    pub(crate) position_2d: Option<Vector2>,
    pub(crate) rotation_2d: Option<f32>,
    pub(crate) scale_2d: Option<Vector2>,
    pub(crate) position_3d: Option<Vector3>,
    pub(crate) rotation_3d: Option<Quaternion>,
    pub(crate) scale_3d: Option<Vector3>,
}

pub(crate) fn apply_bone_pose_overrides_2d(
    skeleton: &mut Skeleton2D,
    overrides: &[PendingBonePoseOverride],
) {
    for entry in overrides {
        let Some(bone) = skeleton
            .bones
            .iter_mut()
            .find(|bone| bone.name.as_ref() == entry.bone)
        else {
            continue;
        };
        if let Some(position) = entry.position_2d {
            bone.pose.position = position;
        }
        if let Some(rotation) = entry.rotation_2d {
            bone.pose.rotation = rotation;
        }
        if let Some(scale) = entry.scale_2d {
            bone.pose.scale = scale;
        }
    }
}

pub(crate) fn apply_bone_pose_overrides_3d(
    skeleton: &mut Skeleton3D,
    overrides: &[PendingBonePoseOverride],
) {
    for entry in overrides {
        let Some(bone) = skeleton
            .bones
            .iter_mut()
            .find(|bone| bone.name.as_ref() == entry.bone)
        else {
            continue;
        };
        if let Some(position) = entry.position_3d {
            bone.pose.position = position;
        }
        if let Some(rotation) = entry.rotation_3d {
            bone.pose.rotation = rotation;
        }
        if let Some(scale) = entry.scale_3d {
            bone.pose.scale = scale;
        }
    }
}

pub(super) fn extract_bone_pose_overrides(data: &SceneDefNodeData) -> Vec<PendingBonePoseOverride> {
    let two_d = match data.node_type {
        NodeType::Skeleton2D => true,
        NodeType::Skeleton3D => false,
        _ => return Vec::new(),
    };
    let Some(SceneValue::Object(bones)) = data
        .fields
        .iter()
        .find(|(name, _)| name.as_ref() == "bones")
        .map(|(_, value)| value)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (bone_name, bone_value) in bones.iter() {
        let SceneValue::Object(fields) = bone_value else {
            continue;
        };
        let mut entry = PendingBonePoseOverride {
            bone: bone_name.as_ref().to_string(),
            ..PendingBonePoseOverride::default()
        };
        let mut rot_deg_3d: Option<Vector3> = None;
        for (name, value) in fields.iter() {
            match name.as_ref() {
                "position" => {
                    if two_d {
                        entry.position_2d = as_vec2(value);
                    } else {
                        entry.position_3d = as_vec3(value);
                    }
                }
                "rotation" => {
                    if two_d {
                        entry.rotation_2d = as_f32(value);
                    } else {
                        entry.rotation_3d = bone_as_quat(value);
                    }
                }
                "rotation_deg" => {
                    if two_d {
                        entry.rotation_2d = as_f32(value).map(f32::to_radians);
                    } else {
                        rot_deg_3d = as_vec3(value);
                    }
                }
                "scale" => {
                    if two_d {
                        entry.scale_2d = as_vec2(value);
                    } else {
                        entry.scale_3d = as_vec3(value);
                    }
                }
                _ => {}
            }
        }
        if entry.rotation_3d.is_none()
            && let Some(deg) = rot_deg_3d
        {
            entry.rotation_3d = Some(Quaternion::from_euler_xyz(
                deg.x.to_radians(),
                deg.y.to_radians(),
                deg.z.to_radians(),
            ));
        }
        if entry.position_2d.is_some()
            || entry.rotation_2d.is_some()
            || entry.scale_2d.is_some()
            || entry.position_3d.is_some()
            || entry.rotation_3d.is_some()
            || entry.scale_3d.is_some()
        {
            out.push(entry);
        }
    }
    out
}

pub(super) fn bone_as_quat(value: &SceneValue) -> Option<Quaternion> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(Quaternion::new(*x, *y, *z, *w)),
        // Euler radians shorthand, mirroring Node3D rotation acceptance.
        SceneValue::Vec3 { x, y, z } => Some(Quaternion::from_euler_xyz(*x, *y, *z)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum PendingJointBodyField {
    BodyA,
    BodyB,
}

#[derive(Clone)]
pub(in super::super) struct PendingJointBodyLink {
    pub(in super::super) field: PendingJointBodyField,
    pub(in super::super) target_key: u32,
}

#[derive(Clone, Debug)]
pub(in super::super) struct PendingLocaleTextBinding {
    pub(in super::super) field: crate::runtime::state::LocaleTextField,
    pub(in super::super) key: String,
    pub(in super::super) key_hash: u64,
}

#[derive(Clone)]
pub(in super::super) struct PendingAnimationTreeAnimation {
    pub(in super::super) source: String,
    pub(in super::super) bindings: Vec<(String, u32)>,
    pub(in super::super) speed: f32,
    pub(in super::super) paused: bool,
    pub(in super::super) playback_type: perro_nodes::AnimationPlaybackType,
}

#[derive(Clone)]
pub(in super::super) struct PendingSurfaceMaterial {
    pub(in super::super) source: Option<String>,
    pub(in super::super) inline: Option<Material3D>,
}

pub(super) type AnimationSceneBindings = Vec<(String, String)>;
pub(super) type AnimationTreeAnimationEntry = (
    String,
    AnimationSceneBindings,
    f32,
    bool,
    perro_nodes::AnimationPlaybackType,
);
pub(super) type AnimationTreeAnimationEntries = Vec<AnimationTreeAnimationEntry>;

pub(super) type SceneNodeExtraction = (
    SceneNode,
    Option<String>,
    Option<String>,
    AnimationTreeAnimationEntries,
    Option<String>,
    Option<String>,
    Vec<PendingSurfaceMaterial>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<(PendingJointBodyField, String)>,
    Vec<(String, String)>,
    Vec<PendingLocaleTextBinding>,
);
