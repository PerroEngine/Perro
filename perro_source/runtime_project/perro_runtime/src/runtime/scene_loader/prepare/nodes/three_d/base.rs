fn build_node_3d(data: &SceneDefNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data(&mut node, data);
    node
}

fn build_camera_stream_3d(data: &SceneDefNodeData) -> CameraStream3D {
    let mut node = CameraStream3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_camera_stream_fields(&mut node.stream, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = [v.x.max(0.001), v.y.max(0.001)];
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    node
}

fn build_mesh_instance_3d(data: &SceneDefNodeData) -> MeshInstance3D {
    let mut node = MeshInstance3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_mesh_instance_3d_fields(&mut node, &data.fields);
    node
}

fn build_multi_mesh_instance_3d(data: &SceneDefNodeData) -> MultiMeshInstance3D {
    let mut node = MultiMeshInstance3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_multi_mesh_instance_3d_fields(&mut node, &data.fields);
    node
}

fn build_water_body_3d(data: &SceneDefNodeData) -> WaterBody3D {
    let mut node = WaterBody3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_water_body_fields(&mut node.water, "WaterBody3D", &data.fields);
    node
}

fn build_decal_3d(data: &SceneDefNodeData) -> Decal3D {
    let mut node = Decal3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_decal_3d_fields(&mut node, &data.fields);
    node
}

fn build_text_decal_3d(data: &SceneDefNodeData) -> TextDecal3D {
    let mut node = TextDecal3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_text_decal_3d_fields(&mut node, &data.fields);
    node
}

fn build_sprite_3d(data: &SceneDefNodeData) -> Sprite3D {
    let mut node = Sprite3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_sprite_3d_fields(&mut node, &data.fields);
    node
}

fn build_video_player_3d(data: &SceneDefNodeData) -> VideoPlayer3D {
    let mut node = VideoPlayer3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_video_player_fields(&mut node.video, &data.fields);
    apply_video_player_3d_fields(&mut node, &data.fields);
    node
}

fn build_label_3d(data: &SceneDefNodeData) -> Label3D {
    let mut node = Label3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_label_3d_fields(&mut node, &data.fields);
    node
}

fn apply_sprite_3d_fields(node: &mut Sprite3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        name if scene_key_in(name, TEXTURE_REGION_KEYS) => {
            if let Some((x, y, w, h)) = value.as_vec4()
                && w > 0.0
                && h > 0.0
            {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = Vector2::new(v.x.max(0.001), v.y.max(0.001));
            }
        }
        name if scene_key_in(name, FLIP_X_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_x = v;
            }
        }
        name if scene_key_in(name, FLIP_Y_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_y = v;
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.modulate.modulate = v;
            }
        }
        _ => {}
    });
}

fn apply_video_player_3d_fields(node: &mut VideoPlayer3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = Vector2::new(v.x.max(0.001), v.y.max(0.001));
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        name if scene_key_in(name, FLIP_X_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_x = v;
            }
        }
        name if scene_key_in(name, FLIP_Y_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_y = v;
            }
        }
        _ => {}
    });
}

fn apply_label_3d_fields(node: &mut Label3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_scene_text_literal(v));
            }
        }
        "size" => {
            if let Some(v) = as_vec2(value) {
                node.size = Vector2::new(v.x.max(0.001), v.y.max(0.001));
            }
        }
        name if scene_key_in(name, TEXT_COLOR_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "font_size" | "text_size" => {
            if let Some(v) = as_f32(value) {
                node.font_size = v.max(0.001);
            }
        }
        "h_align" | "text_h_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.h_align = v;
            }
        }
        "v_align" | "text_v_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.v_align = v;
            }
        }
        _ => {}
    });
}

fn apply_decal_3d_fields(node: &mut Decal3D, fields: &[SceneObjectField]) {
    // Texture paths are resolved to TextureIDs at merge time (see
    // decal_texture_sources on PendingNode); only scalars apply here.
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" | "extents" => {
            if let Some(v) = as_vec3(value) {
                node.size = Vector3::new(v.x.max(0.001), v.y.max(0.001), v.z.max(0.001));
            }
        }
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.modulate = v;
            }
        }
        "albedo_mix" => {
            if let Some(v) = as_f32(value) {
                node.surface.albedo_mix = v.clamp(0.0, 1.0);
            }
        }
        "emission_energy" => {
            if let Some(v) = as_f32(value) {
                node.surface.emission_energy = v.max(0.0);
            }
        }
        "normal_strength" => {
            if let Some(v) = as_f32(value) {
                node.surface.normal_strength = v.max(0.0);
            }
        }
        "normal_fade" => {
            if let Some(v) = as_f32(value) {
                node.surface.normal_fade = v.clamp(0.0, 1.0);
            }
        }
        "distance_fade_begin" => {
            if let Some(v) = as_f32(value) {
                node.distance_fade.begin = v.max(0.0);
            }
        }
        "distance_fade_length" => {
            if let Some(v) = as_f32(value) {
                node.distance_fade.length = v.max(0.001);
            }
        }
        "sort_priority" | "priority" => {
            if let Some(v) = as_i32(value) {
                node.sort_priority = v;
            }
        }
        "active" => {
            if let Some(v) = as_bool(value) {
                node.active = v;
            }
        }
        _ => {}
    });
}

fn apply_text_decal_3d_fields(node: &mut TextDecal3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "text" => {
            if let Some(v) = as_str(value) {
                node.text = Cow::Owned(decode_scene_text_literal(v));
            }
        }
        "size" | "extents" => {
            if let Some(v) = as_vec3(value) {
                node.size = Vector3::new(v.x.max(0.001), v.y.max(0.001), v.z.max(0.001));
            }
        }
        name if scene_key_in(name, TEXT_COLOR_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.color = v;
            }
        }
        "font_size" | "text_size" => {
            if let Some(v) = as_f32(value) {
                node.font_size = v.max(0.001);
            }
        }
        "h_align" | "text_h_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.h_align = v;
            }
        }
        "v_align" | "text_v_align" => {
            if let Some(v) = as_ui_text_align(value) {
                node.v_align = v;
            }
        }
        "texture_resolution" | "resolution" => {
            if let Some(v) = as_u32(value) {
                node.texture_resolution = v.clamp(16, 4096);
            }
        }
        "albedo_mix" => {
            if let Some(v) = as_f32(value) {
                node.surface.albedo_mix = v.clamp(0.0, 1.0);
            }
        }
        "emission_energy" => {
            if let Some(v) = as_f32(value) {
                node.surface.emission_energy = v.max(0.0);
            }
        }
        "normal_strength" => {
            if let Some(v) = as_f32(value) {
                node.surface.normal_strength = v.max(0.0);
            }
        }
        "normal_fade" => {
            if let Some(v) = as_f32(value) {
                node.surface.normal_fade = v.clamp(0.0, 1.0);
            }
        }
        "distance_fade_begin" => {
            if let Some(v) = as_f32(value) {
                node.distance_fade.begin = v.max(0.0);
            }
        }
        "distance_fade_length" => {
            if let Some(v) = as_f32(value) {
                node.distance_fade.length = v.max(0.001);
            }
        }
        "sort_priority" | "priority" => {
            if let Some(v) = as_i32(value) {
                node.sort_priority = v;
            }
        }
        "active" => {
            if let Some(v) = as_bool(value) {
                node.active = v;
            }
        }
        _ => {}
    });
}

fn build_skeleton_3d(data: &SceneDefNodeData) -> Skeleton3D {
    let mut node = Skeleton3D::default();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_skeleton_3d_fields(&mut node, &data.fields);
    node
}

fn build_bone_attachment_3d(data: &SceneDefNodeData) -> BoneAttachment3D {
    let mut node = BoneAttachment3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_bone_attachment_3d_fields(&mut node, &data.fields);
    node
}

fn build_ik_target_3d(data: &SceneDefNodeData) -> IKTarget3D {
    let mut node = IKTarget3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_ik_target_3d_fields(&mut node, &data.fields);
    node
}

fn build_physics_bone_chain_3d(data: &SceneDefNodeData) -> PhysicsBoneChain3D {
    let mut node = PhysicsBoneChain3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_physics_bone_chain_3d_fields(&mut node, &data.fields);
    node
}

fn build_bone_collider_3d(data: &SceneDefNodeData) -> BoneCollider3D {
    let mut node = BoneCollider3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_bone_collider_3d_fields(&mut node, &data.fields);
    node
}

fn apply_node_3d_data(target: &mut Node3D, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(target, base);
    }
    apply_node_3d_fields(target, &data.fields);
}

fn apply_node_3d_fields(node: &mut Node3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each_field(|field, value| {
        if matches!(field, SceneFieldName::RotationDeg) {
            if let Some(v) = as_vec3(value) {
                node.transform.rotation = quat_from_deg_xyz(v);
            }
            return;
        }

        match field {
            SceneFieldName::Position => {
                if let Some(v) = as_vec3(value) {
                    node.transform.position = v;
                }
            }
            SceneFieldName::Scale => {
                if let Some(v) = as_vec3(value) {
                    node.transform.scale = v;
                }
            }
            SceneFieldName::Rotation => {
                if let Some(v) = as_quat(value) {
                    node.transform.rotation = v;
                }
            }
            SceneFieldName::Visible => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            SceneFieldName::Modulate => {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.modulate = v;
                }
            }
            SceneFieldName::Custom(name) if name == "tint" => {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.modulate = v;
                }
            }
            SceneFieldName::SelfModulate => {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.self_modulate = v;
                }
            }
            SceneFieldName::Custom(name) if name == "self_tint" || name == "self_color" => {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.self_modulate = v;
                }
            }
            SceneFieldName::ChildrenModulate => {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.children_modulate = v;
                }
            }
            SceneFieldName::Custom(name)
                if name == "children_tint" || name == "child_tint" || name == "child_color" =>
            {
                if let Some(v) = as_scene_color(value) {
                    node.modulate.children_modulate = v;
                }
            }
            SceneFieldName::RenderLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_mesh_instance_3d_fields(node: &mut MeshInstance3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "surfaces" => {
            if let SceneValue::Array(items) = value {
                node.surfaces = parse_surface_bindings(items.as_ref());
            }
        }
        "meshlets" | "use_meshlets" => {
            if let Some(v) = as_bool(value) {
                node.meshlet_override = Some(v);
            }
        }
        name if scene_key_in(name, FLIP_X_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_x = v;
            }
        }
        "flip_y" | "mirror_y" => {
            if let Some(v) = as_bool(value) {
                node.flip_y = v;
            }
        }
        "flip_z" | "mirror_z" => {
            if let Some(v) = as_bool(value) {
                node.flip_z = v;
            }
        }
        "min_lod" | "lod_min" => {
            if let Some(v) = as_i32(value) {
                node.lod.min_lod = v.clamp(0, LODOptions::MAX as i32) as u8;
            }
        }
        "max_lod" | "lod_max" => {
            if let Some(v) = as_i32(value) {
                node.lod.max_lod = v.clamp(0, LODOptions::MAX as i32) as u8;
            }
        }
        "cast_shadows" | "casts_shadows" => {
            if let Some(v) = as_bool(value) {
                node.cast_shadows = v;
            }
        }
        "receive_shadows" | "receives_shadows" => {
            if let Some(v) = as_bool(value) {
                node.receive_shadows = v;
            }
        }
        "blend" | "mesh_blend" | "blending" => {
            apply_mesh_blend_fields(&mut node.blend, value);
        }
        "blend_shape_weights" | "shape_key_weights" | "morph_weights" => {
            node.blend_shape_weights = parse_blend_shape_weights(value);
        }
        "blend_enabled" => {
            if let Some(v) = as_bool(value) {
                node.blend.enabled = v;
            }
        }
        "blend_normals" => {
            if let Some(v) = as_bool(value) {
                node.blend.normal_blending = v;
            }
        }
        "blend_screen" => {
            if let Some(v) = as_bool(value) {
                node.blend.screen_blending = v;
            }
        }
        "blend_layers" => {
            if let Some(v) = as_bitmask(value) {
                node.blend.blend_layers = v;
            }
        }
        "blend_mask" => {
            if let Some(v) = as_bitmask(value) {
                node.blend.blend_mask = v;
            }
        }
        "blend_distance" | "blend_size" => {
            if let Some(v) = as_f32(value) {
                node.blend.distance = v.max(0.0);
            }
        }
        "blend_min_distance" | "blend_min_size" => {
            if let Some(v) = as_f32(value) {
                node.blend.min_distance = v.max(0.0);
            }
        }
        _ => {}
    });
}

fn apply_multi_mesh_instance_3d_fields(
    node: &mut MultiMeshInstance3D,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "surfaces" => {
            if let SceneValue::Array(items) = value {
                node.surfaces = parse_surface_bindings(items.as_ref());
            }
        }
        "instances" => {
            if let SceneValue::Array(items) = value {
                node.instances = parse_instance_posrot(items.as_ref());
            }
        }
        "instance_grid" | "grid_instances" => {
            if let Some(instances) = parse_instance_grid(value) {
                node.instances = instances;
            }
        }
        "instance_scale" => {
            if let Some(v) = as_f32(value) {
                node.instance_scale = v.max(0.0001);
            }
        }
        "blend_shape_weights" | "shape_key_weights" | "morph_weights" => {
            node.blend_shape_weights = parse_blend_shape_weights(value);
        }
        "meshlets" | "use_meshlets" => {
            if let Some(v) = as_bool(value) {
                node.meshlet_override = Some(v);
            }
        }
        name if scene_key_in(name, FLIP_X_KEYS) => {
            if let Some(v) = as_bool(value) {
                node.flip_x = v;
            }
        }
        "flip_y" | "mirror_y" => {
            if let Some(v) = as_bool(value) {
                node.flip_y = v;
            }
        }
        "flip_z" | "mirror_z" => {
            if let Some(v) = as_bool(value) {
                node.flip_z = v;
            }
        }
        "min_lod" | "lod_min" => {
            if let Some(v) = as_i32(value) {
                node.lod.min_lod = v.clamp(0, LODOptions::MAX as i32) as u8;
            }
        }
        "max_lod" | "lod_max" => {
            if let Some(v) = as_i32(value) {
                node.lod.max_lod = v.clamp(0, LODOptions::MAX as i32) as u8;
            }
        }
        "cast_shadows" | "casts_shadows" => {
            if let Some(v) = as_bool(value) {
                node.cast_shadows = v;
            }
        }
        "receive_shadows" | "receives_shadows" => {
            if let Some(v) = as_bool(value) {
                node.receive_shadows = v;
            }
        }
        "blend" | "mesh_blend" | "blending" => {
            apply_mesh_blend_fields(&mut node.blend, value);
        }
        "blend_enabled" => {
            if let Some(v) = as_bool(value) {
                node.blend.enabled = v;
            }
        }
        "blend_normals" => {
            if let Some(v) = as_bool(value) {
                node.blend.normal_blending = v;
            }
        }
        "blend_screen" => {
            if let Some(v) = as_bool(value) {
                node.blend.screen_blending = v;
            }
        }
        "blend_layers" => {
            if let Some(v) = as_bitmask(value) {
                node.blend.blend_layers = v;
            }
        }
        "blend_mask" => {
            if let Some(v) = as_bitmask(value) {
                node.blend.blend_mask = v;
            }
        }
        "blend_distance" | "blend_size" => {
            if let Some(v) = as_f32(value) {
                node.blend.distance = v.max(0.0);
            }
        }
        "blend_min_distance" | "blend_min_size" => {
            if let Some(v) = as_f32(value) {
                node.blend.min_distance = v.max(0.0);
            }
        }
        _ => {}
    });
}

fn apply_mesh_blend_fields(blend: &mut perro_nodes::MeshBlendOptions, value: &SceneValue) {
    match value {
        SceneValue::Bool(v) => {
            blend.enabled = *v;
        }
        SceneValue::Object(entries) => {
            for (key, value) in entries.iter() {
                match key.as_ref() {
                    "enabled" => {
                        if let Some(v) = as_bool(value) {
                            blend.enabled = v;
                        }
                    }
                    "normal_blending" | "blend_normals" => {
                        if let Some(v) = as_bool(value) {
                            blend.normal_blending = v;
                        }
                    }
                    "screen_blending" | "blend_screen" => {
                        if let Some(v) = as_bool(value) {
                            blend.screen_blending = v;
                        }
                    }
                    "layers" | "blend_layers" => {
                        if let Some(v) = as_bitmask(value) {
                            blend.blend_layers = v;
                        }
                    }
                    "mask" | "blend_mask" => {
                        if let Some(v) = as_bitmask(value) {
                            blend.blend_mask = v;
                        }
                    }
                    "distance" | "size" | "blend_distance" | "blend_size" => {
                        if let Some(v) = as_f32(value) {
                            blend.distance = v.max(0.0);
                        }
                    }
                    "min_distance" | "min_size" => {
                        if let Some(v) = as_f32(value) {
                            blend.min_distance = v.max(0.0);
                        }
                    }
                    "noise_factor" | "noise" => {
                        if let Some(v) = as_f32(value) {
                            blend.noise_factor = v.clamp(0.0, 1.0);
                        }
                    }
                    "noise_scale" | "noise_tile_size" => {
                        if let Some(v) = as_f32(value) {
                            blend.noise_scale = v.max(0.0001);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn apply_skeleton_3d_fields(_node: &mut Skeleton3D, _fields: &[SceneObjectField]) {}

fn apply_bone_attachment_3d_fields(node: &mut BoneAttachment3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "bone" | "bone_index" => {
            if let Some(v) = as_i32(value) {
                node.bone_index = v;
            }
        }
        _ => {}
    });
}

fn apply_ik_target_3d_fields(node: &mut IKTarget3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("IKTarget3D", name) {
            Some(NodeField::IKTarget3D(IKTarget3DField::BoneIndex)) => {
                if let Some(v) = as_i32(value) {
                    node.params.bone_index = v;
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::ChainLength)) => {
                if let Some(v) = as_i32(value) {
                    node.params.chain_length = v.max(0) as u32;
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::Iterations)) => {
                if let Some(v) = as_i32(value) {
                    node.params.iterations =
                        (v.max(0) as u32).min(perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS);
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::Tolerance)) => {
                if let Some(v) = as_f32(value) {
                    node.params.tolerance = v.max(0.0);
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::Weight)) => {
                if let Some(v) = as_f32(value) {
                    node.params.weight = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::MatchRotation)) => {
                if let Some(v) = as_bool(value) {
                    node.params.match_rotation = v;
                }
            }
            Some(NodeField::IKTarget3D(IKTarget3DField::Solver)) => {
                if let Some(v) = as_ik_target_3d_solver(value) {
                    node.params.solver = v;
                }
            }
            _ => {}
        }
    });
}

fn as_ik_target_3d_solver(value: &SceneValue) -> Option<IKTargetSolver> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "ccd" => Some(IKTargetSolver::CCD),
        "fabrik" => Some(IKTargetSolver::FABRIK),
        _ => None,
    }
}

fn apply_physics_bone_chain_3d_fields(node: &mut PhysicsBoneChain3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PhysicsBoneChain3D", name) {
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::BoneIndex)) => {
                if let Some(v) = as_i32(value) {
                    node.bone_index = v;
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::ChainLength)) => {
                if let Some(v) = as_i32(value) {
                    node.chain_length = v.max(0) as u32;
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Enabled)) => {
                if let Some(v) = as_bool(value) {
                    node.enabled = v;
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Gravity)) => {
                if let Some(v) = as_vec3(value) {
                    node.gravity = v;
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Damping)) => {
                if let Some(v) = as_f32(value) {
                    node.damping = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Stiffness)) => {
                if let Some(v) = as_f32(value) {
                    node.stiffness = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Radius)) => {
                if let Some(v) = as_f32(value) {
                    node.radius = v.max(0.0);
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Collisions)) => {
                if let Some(v) = as_bool(value) {
                    node.collisions = v;
                }
            }
            Some(NodeField::PhysicsBoneChain3D(PhysicsBoneChain3DField::Iterations)) => {
                if let Some(v) = as_i32(value) {
                    node.iterations =
                        (v.max(1) as u32).min(perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS);
                }
            }
            _ => {}
        }
    });
}

fn apply_bone_collider_3d_fields(node: &mut BoneCollider3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("BoneCollider3D", name)
            == Some(NodeField::BoneCollider3D(BoneCollider3DField::Enabled))
            && let Some(v) = as_bool(value)
        {
            node.enabled = v;
        }
    });
}

fn extract_mesh_source(data: &SceneDefNodeData) -> Option<String> {
    if !matches!(
        data.node_type,
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("MeshInstance3D", name)
            == Some(NodeField::MeshInstance3D(MeshInstance3DField::Mesh)))
        .then(|| as_asset_source(value))
        .flatten()
    })
}

fn extract_material_source(data: &SceneDefNodeData) -> Option<String> {
    if !matches!(
        data.node_type,
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("MeshInstance3D", name)
            == Some(NodeField::MeshInstance3D(MeshInstance3DField::Material)))
        .then(|| as_asset_source(value))
        .flatten()
    })
}

fn extract_material_inline(data: &SceneDefNodeData) -> Option<Material3D> {
    if !matches!(
        data.node_type,
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        if resolve_node_field("MeshInstance3D", name)
            != Some(NodeField::MeshInstance3D(MeshInstance3DField::Material))
        {
            return None;
        }
        match value {
            SceneValue::Object(entries) => material_schema::from_object(entries.as_ref()),
            _ => None,
        }
    })
}

fn extract_material_surfaces(data: &SceneDefNodeData) -> Vec<PendingSurfaceMaterial> {
    if !matches!(
        data.node_type,
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D
    ) {
        return Vec::new();
    }
    for (name, value) in data.fields.iter() {
        if name != "surfaces" {
            continue;
        }
        let SceneValue::Array(items) = value else {
            continue;
        };
        let mut out = Vec::new();
        for item in items.iter() {
            out.push(parse_surface_material(item));
        }
        return out;
    }

    let source = extract_material_source(data);
    let inline = extract_material_inline(data);
    if source.is_none() && inline.is_none() {
        Vec::new()
    } else {
        vec![PendingSurfaceMaterial { source, inline }]
    }
}

fn parse_surface_bindings(items: &[SceneValue]) -> Vec<MeshSurfaceBinding> {
    items.iter().map(parse_surface_binding).collect()
}

fn parse_surface_binding(value: &SceneValue) -> MeshSurfaceBinding {
    let mut binding = MeshSurfaceBinding::default();
    if let SceneValue::Object(entries) = value {
        for (key, value) in entries.iter() {
            match key.as_ref() {
                "modulate" => {
                    if let Some(color) = parse_color(value) {
                        binding.modulate = color;
                    }
                }
                "overrides" => {
                    if let Some(overrides) = parse_surface_overrides(value) {
                        binding.overrides = overrides;
                    }
                }
                _ => {}
            }
        }
    }
    binding
}

fn parse_surface_material(value: &SceneValue) -> PendingSurfaceMaterial {
    match value {
        SceneValue::Str(_) | SceneValue::Hashed(_) | SceneValue::Key(_) => PendingSurfaceMaterial {
            source: as_asset_source(value),
            inline: None,
        },
        SceneValue::Object(entries) => {
            let mut source = None;
            let mut inline = None;
            for (key, value) in entries.iter() {
                match key.as_ref() {
                    "material" => {
                        source = as_asset_source(value);
                        if source.is_none()
                            && let SceneValue::Object(obj) = value
                        {
                            inline = material_schema::from_object(obj.as_ref());
                        }
                    }
                    "source" => source = as_asset_source(value),
                    _ => {}
                }
            }
            PendingSurfaceMaterial { source, inline }
        }
        _ => PendingSurfaceMaterial {
            source: None,
            inline: None,
        },
    }
}

fn parse_surface_overrides(value: &SceneValue) -> Option<Vec<MaterialParamOverride>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items.iter() {
        let SceneValue::Object(entries) = item else {
            continue;
        };
        let mut name = None::<String>;
        let mut parsed = None::<MaterialParamOverrideValue>;
        for (key, value) in entries.iter() {
            match key.as_ref() {
                "name" => name = as_str(value).map(|v| v.to_string()),
                "value" => parsed = parse_override_value(value),
                _ => {}
            }
        }
        if let (Some(name), Some(value)) = (name, parsed) {
            out.push(MaterialParamOverride {
                name: std::borrow::Cow::Owned(name),
                value,
            });
        }
    }
    Some(out)
}

fn parse_override_value(value: &SceneValue) -> Option<MaterialParamOverrideValue> {
    value.as_const_param()
}

fn parse_color(value: &SceneValue) -> Option<perro_structs::Color> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some(perro_structs::Color::new(*x, *y, *z, *w)),
        SceneValue::Vec3 { x, y, z } => Some(perro_structs::Color::rgb(*x, *y, *z)),
        _ => None,
    }
}

fn parse_instance_posrot(items: &[SceneValue]) -> Vec<perro_nodes::MultiMeshInstancePose> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            SceneValue::Vec3 { x, y, z } => {
                out.push(perro_nodes::MultiMeshInstancePose::from_pos_rot(
                    perro_structs::Vector3::new(*x, *y, *z),
                    perro_structs::Quaternion::IDENTITY,
                ));
            }
            SceneValue::Object(entries) => {
                let mut pos = perro_structs::Vector3::ZERO;
                let mut scale = perro_structs::Vector3::ONE;
                let mut rot = perro_structs::Quaternion::IDENTITY;
                let mut rot_deg: Option<perro_structs::Vector3> = None;
                let mut blend_shape_weights = None;
                for (key, value) in entries.iter() {
                    match key.as_ref() {
                        "position" => {
                            if let Some(v) = as_vec3(value) {
                                pos = v;
                            }
                        }
                        "scale" => {
                            if let Some(v) = as_vec3(value) {
                                scale = v;
                            } else if let Some(v) = as_f32(value) {
                                scale = perro_structs::Vector3::new(v, v, v);
                            }
                        }
                        "rotation" => {
                            if let Some(v) = as_quat(value) {
                                rot = v;
                            }
                        }
                        "rotation_deg" => {
                            if let Some(v) = as_vec3(value) {
                                rot_deg = Some(v);
                            }
                        }
                        "blend_shape_weights" | "shape_key_weights" | "morph_weights" => {
                            blend_shape_weights = Some(parse_blend_shape_weights(value));
                        }
                        _ => {}
                    }
                }
                if let Some(deg) = rot_deg {
                    rot = quat_from_deg_xyz(deg);
                }
                out.push(perro_nodes::MultiMeshInstancePose {
                    transform: perro_structs::Transform3D::new(pos, rot, scale),
                    blend_shape_weights,
                });
            }
            _ => {}
        }
    }
    out
}

fn parse_instance_grid(value: &SceneValue) -> Option<Vec<perro_nodes::MultiMeshInstancePose>> {
    let SceneValue::Object(entries) = value else {
        return None;
    };

    let mut counts = (1_u32, 1_u32, 1_u32);
    let mut spacing = perro_structs::Vector3::new(1.0, 1.0, 1.0);
    let mut origin = perro_structs::Vector3::ZERO;
    let mut scale = perro_structs::Vector3::ONE;
    let mut scale_wave = 0.0_f32;
    let mut height_wave = 0.0_f32;
    let mut rotation_step_deg = perro_structs::Vector3::ZERO;

    for (key, value) in entries.iter() {
        match key.as_ref() {
            "counts" | "count" | "size" | "dims" => {
                if let Some(v) = as_vec3(value) {
                    counts = (
                        v.x.max(1.0) as u32,
                        v.y.max(1.0) as u32,
                        v.z.max(1.0) as u32,
                    );
                }
            }
            "spacing" => {
                if let Some(v) = as_vec3(value) {
                    spacing = v;
                }
            }
            "origin" => {
                if let Some(v) = as_vec3(value) {
                    origin = v;
                }
            }
            "scale" | "instance_scale" => {
                if let Some(v) = as_vec3(value) {
                    scale = v;
                } else if let Some(v) = as_f32(value) {
                    scale = perro_structs::Vector3::new(v, v, v);
                }
            }
            "scale_wave" | "wave_scale" => {
                if let Some(v) = as_f32(value) {
                    scale_wave = v;
                }
            }
            "height_wave" | "wave_height" => {
                if let Some(v) = as_f32(value) {
                    height_wave = v;
                }
            }
            "rotation_step_deg" | "rotation_step" => {
                if let Some(v) = as_vec3(value) {
                    rotation_step_deg = v;
                }
            }
            _ => {}
        }
    }

    let total = counts
        .0
        .saturating_mul(counts.1)
        .saturating_mul(counts.2)
        .min(100_000);
    let mut out = Vec::with_capacity(total as usize);

    for y in 0..counts.1 {
        for z in 0..counts.2 {
            for x in 0..counts.0 {
                let wave = if height_wave != 0.0 {
                    ((x as f32) * 0.63 + (z as f32) * 0.41).sin() * height_wave
                } else {
                    0.0
                };
                let pos = perro_structs::Vector3::new(
                    origin.x + x as f32 * spacing.x,
                    origin.y + y as f32 * spacing.y + wave,
                    origin.z + z as f32 * spacing.z,
                );
                let rot = quat_from_deg_xyz(perro_structs::Vector3::new(
                    x as f32 * rotation_step_deg.x,
                    (x + z + y) as f32 * rotation_step_deg.y,
                    z as f32 * rotation_step_deg.z,
                ));
                let scale_amount = if scale_wave != 0.0 {
                    (1.0 + ((x as f32) * 0.37 + (y as f32) * 0.19 + (z as f32) * 0.53).sin()
                        * scale_wave)
                        .max(0.0001)
                } else {
                    1.0
                };
                out.push(perro_nodes::MultiMeshInstancePose {
                    transform: perro_structs::Transform3D::new(pos, rot, scale * scale_amount),
                    blend_shape_weights: None,
                });
            }
        }
    }

    Some(out)
}

fn parse_blend_shape_weights(value: &SceneValue) -> Vec<f32> {
    let SceneValue::Array(items) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(as_f32)
        .map(|v| v.clamp(0.0, 1.0))
        .collect()
}

#[inline]
fn quat_from_deg_xyz(deg: perro_structs::Vector3) -> perro_structs::Quaternion {
    let to_rad = std::f32::consts::PI / 180.0;
    perro_structs::Quaternion::from_euler_xyz(deg.x * to_rad, deg.y * to_rad, deg.z * to_rad)
}

fn extract_model_source(data: &SceneDefNodeData) -> Option<String> {
    if !matches!(
        data.node_type,
        NodeType::MeshInstance3D | NodeType::MultiMeshInstance3D
    ) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("MeshInstance3D", name)
            == Some(NodeField::MeshInstance3D(MeshInstance3DField::Model)))
        .then(|| as_asset_source(value))
        .flatten()
    })
}

fn extract_skeleton_source(data: &SceneDefNodeData) -> Option<String> {
    if !matches!(data.node_type, NodeType::Skeleton2D | NodeType::Skeleton3D) {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        let resolved = resolve_node_field(data.type_name(), name);
        (resolved
            == Some(NodeField::Skeleton2D(
                perro_scene::Skeleton2DField::Skeleton,
            ))
            || resolved == Some(NodeField::Skeleton3D(Skeleton3DField::Skeleton)))
        .then(|| as_asset_source(value))
        .flatten()
    })
}

fn extract_mesh_skeleton_target(data: &SceneDefNodeData) -> Result<Option<String>, String> {
    if data.node_type != NodeType::MeshInstance3D {
        return Ok(None);
    }
    for (name, value) in data.fields.iter() {
        if resolve_node_field("MeshInstance3D", name)
            == Some(NodeField::MeshInstance3D(MeshInstance3DField::Skeleton))
        {
            return as_node_ref_source(value, "MeshInstance3D.skeleton");
        }
    }
    Ok(None)
}

fn extract_bone_attachment_skeleton_target(
    data: &SceneDefNodeData,
) -> Result<Option<String>, String> {
    if !matches!(
        data.node_type,
        NodeType::BoneAttachment2D | NodeType::BoneAttachment3D
    ) {
        return Ok(None);
    }
    for (name, value) in data.fields.iter() {
        let resolved = resolve_node_field(data.type_name(), name);
        if resolved == Some(NodeField::BoneAttachment2D(BoneAttachment2DField::Skeleton))
            || resolved == Some(NodeField::BoneAttachment3D(BoneAttachment3DField::Skeleton))
        {
            return as_node_ref_source(value, &format!("{}.skeleton", data.type_name()));
        }
    }
    Ok(None)
}

fn extract_ik_target_skeleton_target(data: &SceneDefNodeData) -> Result<Option<String>, String> {
    if !matches!(data.node_type, NodeType::IKTarget2D | NodeType::IKTarget3D) {
        return Ok(None);
    }
    for (name, value) in data.fields.iter() {
        let resolved = resolve_node_field(data.type_name(), name);
        if resolved == Some(NodeField::IKTarget2D(IKTarget2DField::Skeleton))
            || resolved == Some(NodeField::IKTarget3D(IKTarget3DField::Skeleton))
        {
            return as_node_ref_source(value, &format!("{}.skeleton", data.type_name()));
        }
    }
    Ok(None)
}

fn extract_physics_bone_chain_skeleton_target(
    data: &SceneDefNodeData,
) -> Result<Option<String>, String> {
    if !matches!(
        data.node_type,
        NodeType::PhysicsBoneChain2D | NodeType::PhysicsBoneChain3D
    ) {
        return Ok(None);
    }
    for (name, value) in data.fields.iter() {
        let resolved = resolve_node_field(data.type_name(), name);
        if resolved
            == Some(NodeField::PhysicsBoneChain2D(
                PhysicsBoneChain2DField::Skeleton,
            ))
            || resolved
                == Some(NodeField::PhysicsBoneChain3D(
                    PhysicsBoneChain3DField::Skeleton,
                ))
        {
            return as_node_ref_source(value, &format!("{}.skeleton", data.type_name()));
        }
    }
    Ok(None)
}

fn as_node_ref_source(value: &SceneValue, field: &str) -> Result<Option<String>, String> {
    match value {
        SceneValue::Key(v) => Ok(Some(v.to_string())),
        SceneValue::Str(_) => Err(format!("{field} must be a node ref like @SkeletonNode")),
        _ => Ok(None),
    }
}
