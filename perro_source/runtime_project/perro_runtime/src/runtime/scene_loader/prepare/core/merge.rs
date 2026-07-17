use super::*;

pub(super) struct PrepareSceneCtx<'a> {
    pub(super) prepared_nodes: &'a mut Vec<PendingNode>,
    pub(super) scripts: &'a mut Vec<PendingScript>,
    pub(super) next_key: &'a mut u32,
    pub(super) include_stack: &'a mut HashSet<String>,
    pub(super) load_scene: &'a dyn Fn(&str) -> Result<Arc<Scene>, String>,
    pub(super) static_ui_style_lookup: Option<StaticUiStyleLookup>,
    pub(super) scratch: ScenePrepareScratch,
}

#[derive(Default)]
pub(super) struct ScenePrepareScratch {
    pub(super) fields: Vec<SceneObjectField>,
}

pub(super) fn expand_import_children_into_host(
    host_key: u32,
    path: &str,
    import_scene: &Scene,
    import_root: &SceneKey,
    ctx: &mut PrepareSceneCtx<'_>,
) -> Result<(), String> {
    let mut map = HashMap::<SceneKey, u32>::new();
    map.insert(*import_root, host_key);
    for node in import_scene.nodes.as_ref() {
        if node.key == *import_root {
            continue;
        }
        let next = *ctx.next_key;
        *ctx.next_key = ctx.next_key.saturating_add(1);
        map.insert(node.key, next);
    }

    for node in import_scene.nodes.as_ref() {
        if node.key == *import_root {
            continue;
        }
        let remapped_key = map.get(&node.key).copied().ok_or_else(|| {
            format!(
                "missing remap key for `{}` in root_of `{path}`",
                import_scene.key_name_or_id(node.key)
            )
        })?;
        push_entry_prepared(import_scene, node, Some(remapped_key), &map, ctx)?;
    }
    Ok(())
}

pub(super) fn merge_root_host_entry(
    host: &SceneDefNodeEntry,
    base_root: &SceneDefNodeEntry,
) -> SceneDefNodeEntry {
    let mut merged = host.clone();
    merged.name = host.name.clone().or_else(|| base_root.name.clone());
    if host.tags.is_empty() {
        merged.tags = base_root.tags.clone();
    }
    if host.children.is_empty() {
        merged.children = base_root.children.clone();
    }
    merged.parent = host.parent.or(base_root.parent);
    if host.clear_script {
        merged.script = None;
    } else if host.script.is_some() {
        merged.script = host.script.clone();
    } else {
        merged.script = base_root.script.clone();
    }
    merged.clear_script = false;
    merged.script_vars = merge_scene_object_fields(&base_root.script_vars, &host.script_vars);
    merged.data = if host.has_data_override {
        merge_scene_node_data(&base_root.data, &host.data)
    } else {
        base_root.data.clone()
    };
    merged.has_data_override = true;
    merged
}

pub(super) fn merge_scene_node_data(base: &SceneDefNodeData, local: &SceneDefNodeData) -> SceneDefNodeData {
    if base.node_type != local.node_type {
        return local.clone();
    }

    let base_fields = flatten_scene_node_fields(base);
    let local_fields = flatten_scene_node_fields(local);
    let merged_fields = merge_scene_object_fields(&base_fields, &local_fields);
    SceneDefNodeData {
        node_type: local.node_type,
        fields: merged_fields,
        base: None,
    }
}

pub(super) fn flatten_scene_node_fields(data: &SceneDefNodeData) -> Vec<SceneObjectField> {
    let mut out = Vec::new();
    flatten_scene_node_fields_into(data, &mut out);
    out
}

pub(super) fn flatten_scene_node_fields_into(data: &SceneDefNodeData, out: &mut Vec<SceneObjectField>) {
    if let Some(base) = data.base_ref() {
        flatten_scene_node_fields_into(base, out);
    }
    out.extend(data.fields.iter().cloned());
}

pub(super) fn scratch_flatten_scene_node_fields<'a>(
    data: &SceneDefNodeData,
    scratch: &'a mut ScenePrepareScratch,
) -> &'a [SceneObjectField] {
    scratch.fields.clear();
    flatten_scene_node_fields_into(data, &mut scratch.fields);
    scratch.fields.as_slice()
}

pub(super) fn merge_scene_object_fields(
    base: &[SceneObjectField],
    local: &[SceneObjectField],
) -> Cow<'static, [SceneObjectField]> {
    let mut merged: BTreeMap<SceneFieldName, SceneValue> = BTreeMap::new();
    for (name, value) in base {
        merged.insert(name.clone(), value.clone());
    }
    for (name, value) in local {
        if is_unset_marker(value) {
            merged.remove(name);
            continue;
        }

        let key = name.clone();
        let next_value = if let Some(prev) = merged.get(&key) {
            merge_scene_values(prev, value)
        } else {
            value.clone()
        };
        merged.insert(key, next_value);
    }

    Cow::Owned(merged.into_iter().collect())
}

pub(super) fn merge_scene_values(base: &SceneValue, local: &SceneValue) -> SceneValue {
    match (base, local) {
        (SceneValue::Object(base_fields), SceneValue::Object(local_fields)) => {
            SceneValue::Object(merge_scene_object_fields(base_fields, local_fields))
        }
        _ => local.clone(),
    }
}

pub(super) fn is_unset_marker(value: &SceneValue) -> bool {
    matches!(value, SceneValue::Key(key) if key.as_ref() == "__unset__")
        || matches!(value, SceneValue::Str(text) if text.as_ref() == "__unset__")
}

pub(super) fn remap_key(key: SceneKey, key_map: &HashMap<SceneKey, u32>) -> u32 {
    key_map.get(&key).copied().unwrap_or_else(|| key.as_u32())
}

pub(super) fn scene_key_by_name(scene: &Scene, name: &str) -> Option<SceneKey> {
    if let Some(raw) = name.strip_prefix('#') {
        return raw.parse::<u32>().ok().map(SceneKey::new);
    }
    let name = name.strip_prefix('@').unwrap_or(name);
    scene
        .key_names
        .iter()
        .position(|key_name| key_name.as_ref() == name)
        .and_then(|idx| u32::try_from(idx).ok())
        .map(SceneKey::new)
}

pub(super) fn remap_scene_value_keys(
    value: &SceneValue,
    scene: &Scene,
    key_map: &HashMap<SceneKey, u32>,
) -> SceneValue {
    match value {
        SceneValue::Bool(v) => SceneValue::Bool(*v),
        SceneValue::I32(v) => SceneValue::I32(*v),
        SceneValue::F32(v) => SceneValue::F32(*v),
        SceneValue::Vec2 { x, y } => SceneValue::Vec2 { x: *x, y: *y },
        SceneValue::Vec3 { x, y, z } => SceneValue::Vec3 {
            x: *x,
            y: *y,
            z: *z,
        },
        SceneValue::Vec4 { x, y, z, w } => SceneValue::Vec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        },
        SceneValue::IVec2 { x, y } => SceneValue::IVec2 { x: *x, y: *y },
        SceneValue::IVec3 { x, y, z } => SceneValue::IVec3 {
            x: *x,
            y: *y,
            z: *z,
        },
        SceneValue::IVec4 { x, y, z, w } => SceneValue::IVec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        },
        SceneValue::UVec2 { x, y } => SceneValue::UVec2 { x: *x, y: *y },
        SceneValue::UVec3 { x, y, z } => SceneValue::UVec3 {
            x: *x,
            y: *y,
            z: *z,
        },
        SceneValue::UVec4 { x, y, z, w } => SceneValue::UVec4 {
            x: *x,
            y: *y,
            z: *z,
            w: *w,
        },
        SceneValue::Str(v) => SceneValue::Str(v.clone()),
        SceneValue::Hashed(v) => SceneValue::Hashed(*v),
        SceneValue::Key(v) => scene_key_by_name(scene, v.as_ref())
            .map(|key| SceneValue::Key(format!("#{}", remap_key(key, key_map)).into()))
            .unwrap_or_else(|| SceneValue::Key(v.clone())),
        SceneValue::Object(fields) => SceneValue::Object(Cow::Owned(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), remap_scene_value_keys(v, scene, key_map)))
                .collect(),
        )),
        SceneValue::Array(items) => SceneValue::Array(Cow::Owned(
            items
                .iter()
                .map(|v| remap_scene_value_keys(v, scene, key_map))
                .collect(),
        )),
    }
}
pub(super) fn scene_node_from_entry(
    entry: &SceneDefNodeEntry,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
    scratch: &mut ScenePrepareScratch,
) -> Result<SceneNodeExtraction, String> {
    let mut node = SceneNode::new(scene_node_data_from(&entry.data, static_ui_style_lookup)?);
    if let Some(name) = &entry.name {
        node.name = name.clone();
    }
    if !entry.tags.is_empty() {
        let tags = entry
            .tags
            .iter()
            .map(|tag| perro_ids::NodeTag::new(tag.clone()))
            .collect::<Vec<_>>();
        node.set_tags(Some(tags));
    }
    let texture_source = extract_texture_source(&entry.data);
    let animation_source = extract_animation_source(&entry.data);
    let animation_tree_source = extract_animation_tree_source(&entry.data);
    let animation_tree_animations = extract_animation_tree_animations(&entry.data);
    let mesh_source_explicit = extract_mesh_source(&entry.data);
    let material_surfaces_explicit = extract_material_surfaces(&entry.data);
    let skeleton_source = extract_skeleton_source(&entry.data);
    let mesh_skeleton_target = extract_mesh_skeleton_target(&entry.data)?;
    let bone_attachment_skeleton_target = extract_bone_attachment_skeleton_target(&entry.data)?;
    let ik_target_skeleton_target = extract_ik_target_skeleton_target(&entry.data)?;
    let physics_bone_chain_skeleton_target =
        extract_physics_bone_chain_skeleton_target(&entry.data)?;
    let camera_stream_target = extract_camera_stream_target(&entry.data);
    let joint_body_targets = extract_joint_body_targets(&entry.data, scratch);
    let animation_bindings = extract_animation_scene_bindings(&entry.data);
    let locale_text_bindings = extract_locale_text_bindings(&entry.data, scratch);
    let model_source = extract_model_source(&entry.data);
    let (mesh_source, material_surfaces) = if let Some(model) = model_source.as_ref() {
        (
            Some(format!("{model}:mesh[0]")),
            vec![PendingSurfaceMaterial {
                source: Some(format!("{model}:mat[0]")),
                inline: None,
            }],
        )
    } else {
        (mesh_source_explicit, material_surfaces_explicit)
    };
    Ok((
        node,
        animation_source,
        animation_tree_source,
        animation_tree_animations,
        texture_source,
        mesh_source,
        material_surfaces,
        skeleton_source,
        mesh_skeleton_target,
        bone_attachment_skeleton_target,
        ik_target_skeleton_target,
        physics_bone_chain_skeleton_target,
        camera_stream_target,
        joint_body_targets,
        animation_bindings,
        locale_text_bindings,
    ))
}

pub(super) fn extract_camera_stream_target(data: &SceneDefNodeData) -> Option<String> {
    if !matches!(
        data.node_type,
        NodeType::CameraStream2D | NodeType::CameraStream3D | NodeType::UiCameraStream
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if !matches!(
            name.as_ref(),
            "camera" | "camera_id" | "source_camera" | "source" | "webcam"
        ) {
            return None;
        }
        match value {
            SceneValue::Key(v) => Some(v.to_string()),
            _ => None,
        }
    })
}

pub(super) fn extract_locale_text_bindings(
    data: &SceneDefNodeData,
    scratch: &mut ScenePrepareScratch,
) -> Vec<PendingLocaleTextBinding> {
    let mut out = Vec::new();
    match data.node_type {
        NodeType::UiLabel | NodeType::Label2D | NodeType::Label3D => {
            let fields = scratch_flatten_scene_node_fields(data, scratch);
            push_locale_text_binding(
                &mut out,
                fields,
                "text",
                crate::runtime::state::LocaleTextField::LabelText,
            );
        }
        NodeType::UiTextBox | NodeType::UiTextBlock => {
            let fields = scratch_flatten_scene_node_fields(data, scratch);
            push_locale_text_binding(
                &mut out,
                fields,
                "text",
                crate::runtime::state::LocaleTextField::TextEditText,
            );
            push_locale_text_binding(
                &mut out,
                fields,
                "placeholder",
                crate::runtime::state::LocaleTextField::TextEditPlaceholder,
            );
            push_locale_text_binding(
                &mut out,
                fields,
                "hint",
                crate::runtime::state::LocaleTextField::TextEditPlaceholder,
            );
        }
        _ => {}
    }
    out
}

pub(super) fn push_locale_text_binding(
    out: &mut Vec<PendingLocaleTextBinding>,
    fields: &[SceneObjectField],
    field_name: &str,
    field: crate::runtime::state::LocaleTextField,
) {
    for (name, value) in fields {
        if name.as_ref() != field_name {
            continue;
        }
        out.retain(|binding| binding.field != field);
        let Some(raw) = as_str(value) else {
            continue;
        };
        let Some(key) = parse_locale_text_key(raw) else {
            continue;
        };
        out.push(PendingLocaleTextBinding {
            key: key.to_string(),
            key_hash: string_to_u64(key),
            field,
        });
    }
}

pub(super) fn extract_joint_body_targets(
    data: &SceneDefNodeData,
    scratch: &mut ScenePrepareScratch,
) -> Vec<(PendingJointBodyField, String)> {
    let mut out = Vec::new();
    let Some((body_a_field, body_b_field)) = joint_body_fields_for(data.node_type) else {
        return out;
    };
    let fields = scratch_flatten_scene_node_fields(data, scratch);
    for (name, value) in fields {
        let resolved = resolve_scene_node_field(data.type_name(), name);
        let field = if resolved == Some(body_a_field) {
            Some(PendingJointBodyField::BodyA)
        } else if resolved == Some(body_b_field) {
            Some(PendingJointBodyField::BodyB)
        } else {
            None
        };
        if let Some(field) = field
            && let Some(target) = as_str(value)
        {
            out.push((field, target.to_string()));
        }
    }
    out
}

pub(super) fn joint_body_fields_for(ty: NodeType) -> Option<(NodeField, NodeField)> {
    match ty {
        NodeType::PinJoint2D => Some((
            NodeField::PinJoint2D(Joint2DField::BodyA),
            NodeField::PinJoint2D(Joint2DField::BodyB),
        )),
        NodeType::DistanceJoint2D => Some((
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyA)),
            NodeField::DistanceJoint2D(DistanceJoint2DField::Common(Joint2DField::BodyB)),
        )),
        NodeType::FixedJoint2D => Some((
            NodeField::FixedJoint2D(Joint2DField::BodyA),
            NodeField::FixedJoint2D(Joint2DField::BodyB),
        )),
        NodeType::BallJoint3D => Some((
            NodeField::BallJoint3D(Joint3DField::BodyA),
            NodeField::BallJoint3D(Joint3DField::BodyB),
        )),
        NodeType::HingeJoint3D => Some((
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyA)),
            NodeField::HingeJoint3D(HingeJoint3DField::Common(Joint3DField::BodyB)),
        )),
        NodeType::FixedJoint3D => Some((
            NodeField::FixedJoint3D(Joint3DField::BodyA),
            NodeField::FixedJoint3D(Joint3DField::BodyB),
        )),
        _ => None,
    }
}

pub(super) fn scene_node_data_from(
    data: &SceneDefNodeData,
    static_ui_style_lookup: Option<StaticUiStyleLookup>,
) -> Result<SceneNodeData, String> {
    match data.node_type {
        NodeType::Node => Ok(SceneNodeData::Node),
        NodeType::Webcam => Ok(SceneNodeData::Webcam(build_webcam(data))),
        NodeType::Node2D => Ok(SceneNodeData::Node2D(build_node_2d(data))),
        NodeType::CameraStream2D => {
            Ok(SceneNodeData::CameraStream2D(build_camera_stream_2d(data)))
        }
        NodeType::Button2D => Ok(SceneNodeData::Button2D(Box::new(build_button_2d(data)))),
        NodeType::ImageButton2D => Ok(SceneNodeData::ImageButton2D(Box::new(build_image_button_2d(data)))),
        NodeType::NineSliceButton2D => Ok(SceneNodeData::NineSliceButton2D(Box::new(build_nine_slice_button_2d(data)))),
        NodeType::Sprite2D => Ok(SceneNodeData::Sprite2D(build_sprite_2d(data))),
        NodeType::VideoPlayer2D => Ok(SceneNodeData::VideoPlayer2D(build_video_player_2d(data))),
        NodeType::Label2D => Ok(SceneNodeData::Label2D(build_label_2d(data))),
        NodeType::NineSlice2D => Ok(SceneNodeData::NineSlice2D(build_nine_slice_2d(data))),
        NodeType::AnimatedSprite2D => Ok(SceneNodeData::AnimatedSprite2D(build_animated_sprite_2d(
            data,
        ))),
        NodeType::ParticleEmitter2D => Ok(SceneNodeData::ParticleEmitter2D(build_particle_emitter_2d(
            data,
        ))),
        NodeType::AmbientLight2D => Ok(SceneNodeData::AmbientLight2D(build_ambient_light_2d(data))),
        NodeType::RayLight2D => Ok(SceneNodeData::RayLight2D(build_ray_light_2d(data))),
        NodeType::PointLight2D => Ok(SceneNodeData::PointLight2D(build_point_light_2d(data))),
        NodeType::SpotLight2D => Ok(SceneNodeData::SpotLight2D(build_spot_light_2d(data))),
        NodeType::TileMap2D => Ok(SceneNodeData::TileMap2D(build_tilemap_2d(data))),
        NodeType::WaterBody2D => Ok(SceneNodeData::WaterBody2D(Box::new(build_water_body_2d(data)))),
        NodeType::Skeleton2D => Ok(SceneNodeData::Skeleton2D(build_skeleton_2d(data))),
        NodeType::BoneAttachment2D => Ok(SceneNodeData::BoneAttachment2D(build_bone_attachment_2d(
            data,
        ))),
        NodeType::IKTarget2D => Ok(SceneNodeData::IKTarget2D(build_ik_target_2d(data))),
        NodeType::PhysicsBoneChain2D => Ok(SceneNodeData::PhysicsBoneChain2D(
            Box::new(build_physics_bone_chain_2d(data)),
        )),
        NodeType::BoneCollider2D => Ok(SceneNodeData::BoneCollider2D(build_bone_collider_2d(data))),
        NodeType::Camera2D => Ok(SceneNodeData::Camera2D(build_camera_2d(data))),
        NodeType::CollisionShape2D => Ok(SceneNodeData::CollisionShape2D(build_collision_shape_2d(
            data,
        ))),
        NodeType::StaticBody2D => Ok(SceneNodeData::StaticBody2D(build_static_body_2d(data))),
        NodeType::Area2D => Ok(SceneNodeData::Area2D(build_area_2d(data))),
        NodeType::RigidBody2D => Ok(SceneNodeData::RigidBody2D(build_rigid_body_2d(data))),
        NodeType::CharacterBody2D => Ok(SceneNodeData::CharacterBody2D(build_character_body_2d(
            data,
        ))),
        NodeType::PhysicsForceEmitter2D => Ok(SceneNodeData::PhysicsForceEmitter2D(
            build_physics_force_emitter_2d(data),
        )),
        NodeType::PinJoint2D => Ok(SceneNodeData::PinJoint2D(build_pin_joint_2d(data))),
        NodeType::DistanceJoint2D => Ok(SceneNodeData::DistanceJoint2D(build_distance_joint_2d(
            data,
        ))),
        NodeType::FixedJoint2D => Ok(SceneNodeData::FixedJoint2D(build_fixed_joint_2d(data))),
        NodeType::AudioMask2D => Ok(SceneNodeData::AudioMask2D(build_audio_mask_2d(data))),
        NodeType::AudioEffectZone2D => Ok(SceneNodeData::AudioEffectZone2D(
            build_audio_effect_zone_2d(data),
        )),
        NodeType::AudioPortal2D => Ok(SceneNodeData::AudioPortal2D(build_audio_portal_2d(data))),
        NodeType::Node3D => Ok(SceneNodeData::Node3D(build_node_3d(data))),
        NodeType::CameraStream3D => {
            Ok(SceneNodeData::CameraStream3D(build_camera_stream_3d(data)))
        }
        NodeType::MeshInstance3D => Ok(SceneNodeData::MeshInstance3D(build_mesh_instance_3d(data))),
        NodeType::MultiMeshInstance3D => Ok(SceneNodeData::MultiMeshInstance3D(
            build_multi_mesh_instance_3d(data),
        )),
        NodeType::Sprite3D => Ok(SceneNodeData::Sprite3D(build_sprite_3d(data))),
        NodeType::VideoPlayer3D => Ok(SceneNodeData::VideoPlayer3D(build_video_player_3d(data))),
        NodeType::Label3D => Ok(SceneNodeData::Label3D(build_label_3d(data))),
        NodeType::CollisionShape3D => Ok(SceneNodeData::CollisionShape3D(build_collision_shape_3d(
            data,
        ))),
        NodeType::StaticBody3D => Ok(SceneNodeData::StaticBody3D(build_static_body_3d(data))),
        NodeType::Area3D => Ok(SceneNodeData::Area3D(build_area_3d(data))),
        NodeType::RigidBody3D => Ok(SceneNodeData::RigidBody3D(build_rigid_body_3d(data))),
        NodeType::CharacterBody3D => Ok(SceneNodeData::CharacterBody3D(build_character_body_3d(
            data,
        ))),
        NodeType::PhysicsForceEmitter3D => Ok(SceneNodeData::PhysicsForceEmitter3D(
            build_physics_force_emitter_3d(data),
        )),
        NodeType::BallJoint3D => Ok(SceneNodeData::BallJoint3D(build_ball_joint_3d(data))),
        NodeType::HingeJoint3D => Ok(SceneNodeData::HingeJoint3D(build_hinge_joint_3d(data))),
        NodeType::FixedJoint3D => Ok(SceneNodeData::FixedJoint3D(build_fixed_joint_3d(data))),
        NodeType::AudioMask3D => Ok(SceneNodeData::AudioMask3D(build_audio_mask_3d(data))),
        NodeType::AudioEffectZone3D => Ok(SceneNodeData::AudioEffectZone3D(
            build_audio_effect_zone_3d(data),
        )),
        NodeType::AudioPortal3D => Ok(SceneNodeData::AudioPortal3D(build_audio_portal_3d(data))),
        NodeType::Skeleton3D => Ok(SceneNodeData::Skeleton3D(build_skeleton_3d(data))),
        NodeType::BoneAttachment3D => Ok(SceneNodeData::BoneAttachment3D(build_bone_attachment_3d(
            data,
        ))),
        NodeType::IKTarget3D => Ok(SceneNodeData::IKTarget3D(build_ik_target_3d(data))),
        NodeType::PhysicsBoneChain3D => Ok(SceneNodeData::PhysicsBoneChain3D(
            Box::new(build_physics_bone_chain_3d(data)),
        )),
        NodeType::BoneCollider3D => Ok(SceneNodeData::BoneCollider3D(build_bone_collider_3d(data))),
        NodeType::Camera3D => Ok(SceneNodeData::Camera3D(build_camera_3d(data))),
        NodeType::ParticleEmitter3D => Ok(SceneNodeData::ParticleEmitter3D(build_particle_emitter_3d(
            data,
        ))),
        NodeType::WaterBody3D => Ok(SceneNodeData::WaterBody3D(Box::new(build_water_body_3d(data)))),
        NodeType::Decal3D => Ok(SceneNodeData::Decal3D(build_decal_3d(data))),
        NodeType::AnimationPlayer => Ok(SceneNodeData::AnimationPlayer(build_animation_player(data))),
        NodeType::AnimationTree => Ok(SceneNodeData::AnimationTree(build_animation_tree(data))),
        NodeType::AmbientLight3D => Ok(SceneNodeData::AmbientLight3D(build_ambient_light_3d(data))),
        NodeType::Sky3D => Ok(SceneNodeData::Sky3D(build_sky_3d(data))),
        NodeType::RayLight3D => Ok(SceneNodeData::RayLight3D(build_ray_light_3d(data))),
        NodeType::PointLight3D => Ok(SceneNodeData::PointLight3D(build_point_light_3d(data))),
        NodeType::SpotLight3D => Ok(SceneNodeData::SpotLight3D(build_spot_light_3d(data))),
        NodeType::UiNode => Ok(SceneNodeData::UiNode(build_ui_node(data))),
        NodeType::UiPanel => Ok(SceneNodeData::UiPanel(Box::new(build_ui_panel(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiProgressBar => Ok(SceneNodeData::UiProgressBar(Box::new(build_ui_progress_bar(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiButton => Ok(SceneNodeData::UiButton(Box::new(build_ui_button(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiDropdown => Ok(SceneNodeData::UiDropdown(Box::new(build_ui_dropdown(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiShape => Ok(SceneNodeData::UiShape(build_ui_shape(data))),
        NodeType::UiCheckbox => Ok(SceneNodeData::UiCheckbox(Box::new(build_ui_checkbox(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiColorPicker => Ok(SceneNodeData::UiColorPicker(Box::new(build_ui_color_picker(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiCameraStream => Ok(SceneNodeData::UiCameraStream(Box::new(build_ui_camera_stream(data)))),
        NodeType::UiViewport => Ok(SceneNodeData::UiViewport(Box::new(build_ui_viewport(data)))),
        NodeType::UiImage => Ok(SceneNodeData::UiImage(Box::new(build_ui_image(data)))),
        NodeType::UiVideoPlayer => Ok(SceneNodeData::UiVideoPlayer(Box::new(build_ui_video_player(data)))),
        NodeType::UiImageButton => Ok(SceneNodeData::UiImageButton(Box::new(build_ui_image_button(data)))),
        NodeType::UiNineSliceButton => Ok(SceneNodeData::UiNineSliceButton(Box::new(build_ui_nine_slice_button(data)))),
        NodeType::UiNineSlice => Ok(SceneNodeData::UiNineSlice(Box::new(build_ui_nine_slice(data)))),
        NodeType::UiAnimatedImage => Ok(SceneNodeData::UiAnimatedImage(Box::new(build_ui_animated_image(
            data,
        )))),
        NodeType::UiLabel => Ok(SceneNodeData::UiLabel(Box::new(build_ui_label(data)))),
        NodeType::UiTextBox => Ok(SceneNodeData::UiTextBox(Box::new(build_ui_text_box(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiTextBlock => Ok(SceneNodeData::UiTextBlock(Box::new(build_ui_text_block(
            data,
            static_ui_style_lookup,
        )))),
        NodeType::UiScrollContainer => Ok(SceneNodeData::UiScrollContainer(
            Box::new(build_ui_scroll_container(data)),
        )),
        NodeType::UiLayout => Ok(SceneNodeData::UiLayout(build_ui_layout(data))),
        NodeType::UiHLayout => Ok(SceneNodeData::UiHLayout(build_ui_hlayout(data))),
        NodeType::UiVLayout => Ok(SceneNodeData::UiVLayout(build_ui_vlayout(data))),
        NodeType::UiGrid => Ok(SceneNodeData::UiGrid(build_ui_grid(data))),
        NodeType::UiTreeList => Ok(SceneNodeData::UiTreeList(Box::new(build_ui_tree_list(data)))),
    }
}

pub(super) fn apply_camera_stream_fields(stream: &mut CameraStream, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "camera" | "camera_id" | "source_camera" | "source" | "webcam" => {
            if let Some(v) = as_node_id(value) {
                stream.camera = v;
            }
        }
        "resolution" => {
            if let Some(v) = as_vec2(value) {
                stream.resolution = UVector2::new(v.x.max(1.0) as u32, v.y.max(1.0) as u32);
            }
        }
        "width" => {
            if let Some(v) = as_u32(value) {
                stream.resolution.x = v.max(1);
            }
        }
        "height" => {
            if let Some(v) = as_u32(value) {
                stream.resolution.y = v.max(1);
            }
        }
        "aspect_ratio" | "ratio" => {
            if let Some(v) = as_f32(value) {
                stream.aspect_ratio = v.max(0.0);
            }
        }
        "aspect_mode" | "scale_mode" | "image_scale" => {
            if let Some(v) = as_str(value) {
                stream.aspect_mode = match v {
                    "stretch" | "fill" => UiImageScaleMode::Stretch,
                    "cover" | "crop" => UiImageScaleMode::Cover,
                    _ => UiImageScaleMode::Fit,
                };
            }
        }
        "post_processing" => {
            if let Some(v) = as_post_processing(value) {
                stream.post_processing = v;
            }
        }
        "enabled" | "active" => {
            if let Some(v) = as_bool(value) {
                stream.enabled = v;
            }
        }
        _ => {}
    });
    stream.resolution.x = stream.resolution.x.clamp(1, 8192);
    stream.resolution.y = stream.resolution.y.clamp(1, 8192);
}

pub(super) fn apply_video_player_fields(node: &mut VideoPlayer, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "source" | "path" | "video" | "stream" => {
            if let Some(v) = as_str(value) {
                node.source = Cow::Owned(v.to_string());
            }
        }
        "playing" | "play" | "autoplay" => {
            if let Some(v) = as_bool(value) {
                node.playing = v;
            }
        }
        "paused" => {
            if let Some(v) = as_bool(value) {
                node.playing = !v;
            }
        }
        "looping" | "loop" => {
            if let Some(v) = as_bool(value) {
                node.looping = v;
            }
        }
        "fps_scale" | "speed" | "playback_speed" => {
            if let Some(v) = as_f32(value) {
                node.fps_scale = v.max(0.0);
            }
        }
        "volume" => {
            if let Some(v) = as_f32(value) {
                node.volume = v.max(0.0);
            }
        }
        _ => {}
    });
}

pub(super) fn build_webcam(data: &SceneDefNodeData) -> Webcam {
    let mut node = Webcam::default();
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "slot" | "device" | "device_id" | "name" => {
            if let Some(v) = as_str(value) {
                node.config.device = v.to_string().into();
            }
        }
        "resolution" => {
            if let Some(v) = as_vec2(value) {
                node.config.width = (v.x.max(1.0) as u32).clamp(1, 8192);
                node.config.height = (v.y.max(1.0) as u32).clamp(1, 8192);
            }
        }
        "width" => {
            if let Some(v) = as_u32(value) {
                node.config.width = v.clamp(1, 8192);
            }
        }
        "height" => {
            if let Some(v) = as_u32(value) {
                node.config.height = v.clamp(1, 8192);
            }
        }
        "fps" | "frame_rate" => {
            if let Some(v) = as_u32(value) {
                node.config.fps = v.clamp(1, 240);
            }
        }
        "mirror" => {
            if let Some(v) = as_bool(value) {
                node.config.mirror = v;
            }
        }
        "cpu_frames" | "cpu_frame" | "readback" => {
            if let Some(v) = as_bool(value) {
                node.config.cpu_frames = v;
            }
        }
        "enabled" | "active" => {
            if let Some(v) = as_bool(value) {
                node.enabled = v;
            }
        }
        _ => {}
    });
    node
}
