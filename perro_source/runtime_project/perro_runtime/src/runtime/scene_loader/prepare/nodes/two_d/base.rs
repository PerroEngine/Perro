fn build_node_2d(data: &SceneDefNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
    node
}

fn build_camera_stream_2d(data: &SceneDefNodeData) -> CameraStream2D {
    let mut node = CameraStream2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_camera_stream_fields(&mut node.stream, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        "tint" | "color" | "modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    node
}

fn build_sprite_2d(data: &SceneDefNodeData) -> Sprite2D {
    let mut node = Sprite2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_button_2d(data: &SceneDefNodeData) -> Button2D {
    let mut node = Button2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_button_2d_fields(&mut node, &data.fields);
    node
}

fn build_image_button_2d(data: &SceneDefNodeData) -> ImageButton2D {
    let mut node = ImageButton2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_image_button_2d_fields(&mut node, &data.fields);
    node
}

fn build_nine_slice_2d(data: &SceneDefNodeData) -> NineSlice2D {
    let mut node = NineSlice2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_nine_slice_2d_fields(&mut node, &data.fields);
    node
}

fn build_animated_sprite_2d(data: &SceneDefNodeData) -> AnimatedSprite2D {
    let mut node = AnimatedSprite2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_animated_sprite_2d_fields(&mut node, &data.fields);
    node
}

fn build_particle_emitter_2d(data: &SceneDefNodeData) -> ParticleEmitter2D {
    let mut node = ParticleEmitter2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_particle_emitter_2d_fields(&mut node, &data.fields);
    node
}

fn build_water_body_2d(data: &SceneDefNodeData) -> WaterBody2D {
    let mut node = WaterBody2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_water_body_fields(&mut node.water, "WaterBody2D", &data.fields);
    node
}

fn build_ambient_light_2d(data: &SceneDefNodeData) -> AmbientLight2D {
    let mut node = AmbientLight2D::new();
    apply_ambient_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_ray_light_2d(data: &SceneDefNodeData) -> RayLight2D {
    let mut node = RayLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_ray_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_point_light_2d(data: &SceneDefNodeData) -> PointLight2D {
    let mut node = PointLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_point_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_spot_light_2d(data: &SceneDefNodeData) -> SpotLight2D {
    let mut node = SpotLight2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_spot_light_2d_fields(&mut node, &data.fields);
    node
}

fn build_tilemap_2d(data: &SceneDefNodeData) -> TileMap2D {
    let mut node = TileMap2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_tilemap_2d_fields(&mut node, &data.fields);
    node
}

fn build_skeleton_2d(data: &SceneDefNodeData) -> Skeleton2D {
    let mut node = Skeleton2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_skeleton_2d_fields(&mut node, &data.fields);
    node
}

fn build_bone_attachment_2d(data: &SceneDefNodeData) -> BoneAttachment2D {
    let mut node = BoneAttachment2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_bone_attachment_2d_fields(&mut node, &data.fields);
    node
}

fn build_ik_target_2d(data: &SceneDefNodeData) -> IKTarget2D {
    let mut node = IKTarget2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_ik_target_2d_fields(&mut node, &data.fields);
    node
}

fn build_physics_bone_chain_2d(data: &SceneDefNodeData) -> PhysicsBoneChain2D {
    let mut node = PhysicsBoneChain2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_physics_bone_chain_2d_fields(&mut node, &data.fields);
    node
}

fn build_bone_collider_2d(data: &SceneDefNodeData) -> BoneCollider2D {
    let mut node = BoneCollider2D::new();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_bone_collider_2d_fields(&mut node, &data.fields);
    node
}

fn apply_node_2d_data(target: &mut Node2D, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(target, base);
    }
    apply_node_2d_fields(target, &data.fields);
}

fn apply_skeleton_2d_fields(_node: &mut Skeleton2D, _fields: &[SceneObjectField]) {}

fn apply_bone_attachment_2d_fields(node: &mut BoneAttachment2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("BoneAttachment2D", name)
            == Some(NodeField::BoneAttachment2D(
                BoneAttachment2DField::BoneIndex,
            ))
            && let Some(v) = as_i32(value)
        {
            node.bone_index = v;
        }
    });
}

fn apply_ik_target_2d_fields(node: &mut IKTarget2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("IKTarget2D", name) {
            Some(NodeField::IKTarget2D(IKTarget2DField::BoneIndex)) => {
                if let Some(v) = as_i32(value) {
                    node.params.bone_index = v;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::ChainLength)) => {
                if let Some(v) = as_i32(value) {
                    node.params.chain_length = v.max(0) as u32;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Iterations)) => {
                if let Some(v) = as_i32(value) {
                    node.params.iterations = v.max(0) as u32;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Tolerance)) => {
                if let Some(v) = value.as_f32() {
                    node.params.tolerance = v.max(0.0);
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Weight)) => {
                if let Some(v) = value.as_f32() {
                    node.params.weight = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::MatchRotation)) => {
                if let Some(v) = value.as_bool() {
                    node.params.match_rotation = v;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Solver)) => {
                if let Some(v) = as_ik_target_2d_solver(value) {
                    node.params.solver = v;
                }
            }
            _ => {}
        }
    });
}

fn as_ik_target_2d_solver(value: &SceneValue) -> Option<IKTargetSolver> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "ccd" => Some(IKTargetSolver::CCD),
        "fabrik" => Some(IKTargetSolver::FABRIK),
        _ => None,
    }
}

fn apply_physics_bone_chain_2d_fields(node: &mut PhysicsBoneChain2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PhysicsBoneChain2D", name) {
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::BoneIndex)) => {
                if let Some(v) = as_i32(value) {
                    node.bone_index = v;
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::ChainLength)) => {
                if let Some(v) = as_i32(value) {
                    node.chain_length = v.max(0) as u32;
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Enabled)) => {
                if let Some(v) = value.as_bool() {
                    node.enabled = v;
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Gravity)) => {
                if let Some((x, y)) = value.as_vec2() {
                    node.gravity = Vector2::new(x, y);
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Damping)) => {
                if let Some(v) = value.as_f32() {
                    node.damping = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Stiffness)) => {
                if let Some(v) = value.as_f32() {
                    node.stiffness = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Radius)) => {
                if let Some(v) = value.as_f32() {
                    node.radius = v.max(0.0);
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Collisions)) => {
                if let Some(v) = value.as_bool() {
                    node.collisions = v;
                }
            }
            Some(NodeField::PhysicsBoneChain2D(PhysicsBoneChain2DField::Iterations)) => {
                if let Some(v) = as_i32(value) {
                    node.iterations = v.max(1) as u32;
                }
            }
            _ => {}
        }
    });
}

fn apply_bone_collider_2d_fields(node: &mut BoneCollider2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("BoneCollider2D", name)
            == Some(NodeField::BoneCollider2D(BoneCollider2DField::Enabled))
            && let Some(v) = value.as_bool()
        {
            node.enabled = v;
        }
    });
}

fn apply_node_2d_fields(node: &mut Node2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each_field(|field, value| {
        if matches!(field, SceneFieldName::RotationDeg) {
            if let Some(v) = value.as_f32() {
                node.transform.rotation = v.to_radians();
            }
            return;
        }

        match field {
            SceneFieldName::Position => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.position = Vector2 { x, y };
                }
            }
            SceneFieldName::Scale => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.scale = Vector2 { x, y };
                }
            }
            SceneFieldName::Rotation => {
                if let Some(v) = value.as_f32() {
                    node.transform.rotation = v;
                }
            }
            SceneFieldName::ZIndex => {
                if let Some(v) = value.as_i32() {
                    node.z_index = v;
                }
            }
            SceneFieldName::Visible => {
                if let Some(v) = value.as_bool() {
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

fn apply_particle_emitter_2d_fields(node: &mut ParticleEmitter2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("ParticleEmitter2D", name) {
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Looping)) => {
                if let Some(v) = value.as_bool() {
                    node.looping = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Prewarm)) => {
                if let Some(v) = value.as_bool() {
                    node.prewarm = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::SpawnRate)) => {
                if let Some(v) = value.as_f32() {
                    node.spawn_rate = v.max(0.0);
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Seed)) => {
                if let Some(v) = as_i32(value) {
                    node.seed = v.max(0) as u32;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Params)) => {
                if let Some(v) = as_particle_params(value) {
                    node.params = v;
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::Profile)) => {
                if let Some(path) = as_str(value) {
                    node.profile = path.to_string();
                } else if let SceneValue::Object(entries) = value {
                    node.profile = inline_pparticle(entries);
                }
            }
            Some(NodeField::ParticleEmitter2D(ParticleEmitter2DField::SimMode)) => {
                if let Some(v) = as_particle_sim_mode_2d(value) {
                    node.sim_mode = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_tilemap_2d_fields(node: &mut TileMap2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("TileMap2D", name) {
            Some(NodeField::TileMap2D(TileMap2DField::Tileset)) => {
                if let Some(v) = as_str(value) {
                    node.tileset = v.to_string();
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Width)) => {
                if let Some(v) = as_u32(value) {
                    node.width = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Height)) => {
                if let Some(v) = as_u32(value) {
                    node.height = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::EmptyTile)) => {
                if let Some(v) = as_i32(value) {
                    node.empty_tile = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::Tiles)) => {
                if let SceneValue::Array(items) = value {
                    node.tiles = items.iter().filter_map(as_i32).collect();
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionEnabled)) => {
                if let Some(v) = as_bool(value) {
                    node.collision_enabled = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_layers = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionMask)) => {
                if let Some(v) = as_bitmask(value) {
                    node.collision_mask = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_ambient_light_2d_fields(node: &mut AmbientLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_light_2d_common("AmbientLight2D", name, value, |field| match field {
            Light2DField::Color => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Light2DField::Intensity => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Light2DField::CastShadows => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Light2DField::Active => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Light2DField::RenderLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
        });
    });
}

fn apply_ray_light_2d_fields(node: &mut RayLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        apply_light_2d_common("RayLight2D", name, value, |field| match field {
            Light2DField::Color => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Light2DField::Intensity => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Light2DField::CastShadows => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Light2DField::Active => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Light2DField::RenderLayers => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
        });
        if resolve_node_field("RayLight2D", name)
            == Some(NodeField::RayLight2D(RayLight2DField::Visible))
            && let Some(v) = value.as_bool()
        {
            node.visible = v;
        }
    });
}

fn apply_point_light_2d_fields(node: &mut PointLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("PointLight2D", name) {
            Some(NodeField::Light2D(Light2DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Some(NodeField::Light2D(Light2DField::Intensity)) => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Some(NodeField::PointLight2D(PointLight2DField::Range)) => {
                if let Some(v) = value.as_f32() {
                    node.range = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::CastShadows)) => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_spot_light_2d_fields(node: &mut SpotLight2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("SpotLight2D", name) {
            Some(NodeField::Light2D(Light2DField::Color)) => {
                if let Some(v) = as_vec3(value) {
                    node.color = [v.x, v.y, v.z];
                } else if let Some((x, y, z, _)) = value.as_vec4() {
                    node.color = [x, y, z];
                }
            }
            Some(NodeField::Light2D(Light2DField::Intensity)) => {
                if let Some(v) = value.as_f32() {
                    node.intensity = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::Range)) => {
                if let Some(v) = value.as_f32() {
                    node.range = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::InnerAngleRadians)) => {
                if let Some(v) = value.as_f32() {
                    node.inner_angle_radians = v;
                }
            }
            Some(NodeField::SpotLight2D(SpotLight2DField::OuterAngleRadians)) => {
                if let Some(v) = value.as_f32() {
                    node.outer_angle_radians = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::CastShadows)) => {
                if let Some(v) = value.as_bool() {
                    node.cast_shadows = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::Active)) => {
                if let Some(v) = value.as_bool() {
                    node.active = v;
                }
            }
            Some(NodeField::Light2D(Light2DField::RenderLayers)) => {
                if let Some(v) = as_bitmask(value) {
                    node.render_layers = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_light_2d_common(
    node_type: &str,
    name: &str,
    value: &SceneValue,
    mut apply: impl FnMut(Light2DField),
) {
    if let Some(NodeField::Light2D(field)) = resolve_node_field(node_type, name) {
        let _ = value;
        apply(field);
    }
}

fn as_particle_sim_mode_2d(value: &SceneValue) -> Option<ParticleEmitterSimMode2D> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "default" => Some(ParticleEmitterSimMode2D::Default),
        "cpu" => Some(ParticleEmitterSimMode2D::Cpu),
        _ => None,
    }
}

fn apply_sprite_2d_fields(node: &mut Sprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each_field(|field, value| {
        match field {
            SceneFieldName::TextureRegion => {
                if let Some((x, y, w, h)) = value.as_vec4()
                    && w > 0.0
                    && h > 0.0
                {
                    node.texture_region = Some([x, y, w, h]);
                }
            }
            SceneFieldName::FlipX => {
                if let Some(v) = value.as_bool() {
                    node.flip_x = v;
                }
            }
            SceneFieldName::FlipY => {
                if let Some(v) = value.as_bool() {
                    node.flip_y = v;
                }
            }
            _ => {}
        }
    });
}

struct Button2DCommonFields<'a> {
    input_mask: &'a mut perro_ui::UiInputMask,
    mouse_filter: &'a mut UiMouseFilter,
    cursor_icon: &'a mut perro_ui::CursorIcon,
    input_enabled: &'a mut bool,
    disabled: &'a mut bool,
    clicked_signals: &'a mut Vec<perro_ids::SignalID>,
    hover_signals: &'a mut Vec<perro_ids::SignalID>,
    hover_exit_signals: &'a mut Vec<perro_ids::SignalID>,
    pressed_signals: &'a mut Vec<perro_ids::SignalID>,
    released_signals: &'a mut Vec<perro_ids::SignalID>,
    web: &'a mut Option<perro_ui::UiButtonWebAction>,
}

fn apply_button_2d_common(target: Button2DCommonFields<'_>, fields: &[SceneObjectField]) {
    apply_ui_input_mask_fields(target.input_mask, fields);
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "input_enabled" => {
            if let Some(v) = as_bool(value) {
                *target.input_enabled = v;
            }
        }
        "disabled" => {
            if let Some(v) = as_bool(value) {
                *target.disabled = v;
            }
        }
        "mouse_filter" => {
            if let Some(v) = as_ui_mouse_filter(value) {
                *target.mouse_filter = v;
            }
        }
        "cursor_icon" | "hover_cursor_icon" => {
            if let Some(v) = as_cursor_icon(value) {
                *target.cursor_icon = v;
            }
        }
        "hover_signals" | "hovered_signals" | "hover_enter_signals" => {
            *target.hover_signals = as_signal_ids(value);
        }
        "hover_exit_signals" | "unhover_signals" => {
            *target.hover_exit_signals = as_signal_ids(value);
        }
        "pressed_signals" | "press_signals" => {
            *target.pressed_signals = as_signal_ids(value);
        }
        "released_signals" | "release_signals" => {
            *target.released_signals = as_signal_ids(value);
        }
        "clicked_signals" | "click_signals" => {
            *target.clicked_signals = as_signal_ids(value);
        }
        "web" => {
            *target.web = parse_ui_button_web_action(value);
        }
        _ => {}
    });
}

fn apply_button_2d_fields(node: &mut Button2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if matches!(name, "size")
            && let Some((x, y)) = value.as_vec2()
        {
            node.size = Vector2::new(x.max(0.0), y.max(0.0));
        }
    });
    apply_button_2d_common(
        Button2DCommonFields {
            input_mask: &mut node.input_mask,
            mouse_filter: &mut node.mouse_filter,
            cursor_icon: &mut node.cursor_icon,
            input_enabled: &mut node.input_enabled,
            disabled: &mut node.disabled,
            clicked_signals: &mut node.clicked_signals,
            hover_signals: &mut node.hover_signals,
            hover_exit_signals: &mut node.hover_exit_signals,
            pressed_signals: &mut node.pressed_signals,
            released_signals: &mut node.released_signals,
            web: &mut node.web,
        },
        fields,
    );
    apply_ui_style_fields(&mut node.style, fields, "");
    node.hover_style = node.style.clone();
    node.pressed_style = node.style.clone();
    apply_ui_style_fields(&mut node.hover_style, fields, "hover_");
    apply_ui_style_fields(&mut node.pressed_style, fields, "pressed_");
}

fn apply_image_button_2d_fields(node: &mut ImageButton2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some((x, y)) = value.as_vec2() {
                node.size = Vector2::new(x.max(0.0), y.max(0.0));
            }
        }
        "texture_region" | "region" | "atlas_region" => {
            if let Some((x, y, w, h)) = value.as_vec4()
                && w > 0.0
                && h > 0.0
            {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        "tint" | "color" | "modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    apply_button_2d_common(
        Button2DCommonFields {
            input_mask: &mut node.input_mask,
            mouse_filter: &mut node.mouse_filter,
            cursor_icon: &mut node.cursor_icon,
            input_enabled: &mut node.input_enabled,
            disabled: &mut node.disabled,
            clicked_signals: &mut node.clicked_signals,
            hover_signals: &mut node.hover_signals,
            hover_exit_signals: &mut node.hover_exit_signals,
            pressed_signals: &mut node.pressed_signals,
            released_signals: &mut node.released_signals,
            web: &mut node.web,
        },
        fields,
    );
    node.hover_tint = node.tint;
    node.pressed_tint = node.tint;
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "hover_tint" | "hover_color" | "hover_modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.hover_tint = v;
            }
        }
        "pressed_tint" | "pressed_color" | "pressed_modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.pressed_tint = v;
            }
        }
        _ => {}
    });
}

fn apply_nine_slice_2d_fields(node: &mut NineSlice2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| match name {
        "size" => {
            if let Some((x, y)) = value.as_vec2() {
                node.size = Vector2::new(x.max(0.0), y.max(0.0));
            }
        }
        "texture_region" | "region" | "atlas_region" => {
            if let Some((x, y, w, h)) = value.as_vec4() && w > 0.0 && h > 0.0 {
                node.texture_region = Some([x, y, w, h]);
            }
        }
        "margins" | "slice" | "slices" => {
            if let Some(v) = as_margins_4(value) {
                node.margins = v;
            }
        }
        "tint" | "color" | "modulate" => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
}

fn as_margins_4(value: &SceneValue) -> Option<[f32; 4]> {
    if let Some((x, y, z, w)) = value.as_vec4() {
        return Some([x.max(0.0), y.max(0.0), z.max(0.0), w.max(0.0)]);
    }
    if let Some((x, y)) = value.as_vec2() {
        return Some([x.max(0.0), y.max(0.0), x.max(0.0), y.max(0.0)]);
    }
    value.as_f32().map(|v| [v.max(0.0); 4])
}

fn apply_animated_sprite_2d_fields(node: &mut AnimatedSprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AnimatedSprite2D", name) {
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Animations)) => {
                if let Some(animations) = parse_animated_sprite_list(value) {
                    node.animations = animations;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipX)) => {
                if let Some(v) = value.as_bool() {
                    node.flip_x = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FlipY)) => {
                if let Some(v) = value.as_bool() {
                    node.flip_y = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::CurrentAnimation)) => {
                if let Some(v) = as_str(value) {
                    node.current_animation = std::borrow::Cow::Owned(v.to_string());
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::CurrentFrame)) => {
                if let Some(v) = as_i32(value) {
                    node.current_frame = u32::try_from(v.max(0)).unwrap_or(0);
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::FpsScale)) => {
                if let Some(v) = value.as_f32() {
                    node.fps_scale = v.max(0.0);
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Playing)) => {
                if let Some(v) = value.as_bool() {
                    node.playing = v;
                }
            }
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Looping)) => {
                if let Some(v) = value.as_bool() {
                    node.looping = v;
                }
            }
            _ => {}
        }
    });
    if node.current_animation_data().is_none() {
        node.animations.push(AnimatedSprite::default());
    }
    let max_frame = node
        .current_animation_data()
        .map(|animation| animation.frame_count.max(1).saturating_sub(1))
        .unwrap_or(0);
    node.current_frame = node.current_frame.min(max_frame);
}

fn parse_animated_sprite_list(value: &SceneValue) -> Option<Vec<AnimatedSprite>> {
    let SceneValue::Array(items) = value else {
        return None;
    };
    let mut out = Vec::new();
    for item in items.iter() {
        if let Some(animation) = parse_animated_sprite(item) {
            out.push(animation);
        }
    }
    (!out.is_empty()).then_some(out)
}

fn parse_animated_sprite(value: &SceneValue) -> Option<AnimatedSprite> {
    let SceneValue::Object(fields) = value else {
        return None;
    };

    let mut animation = AnimatedSprite::default();
    for (name, value) in fields.iter() {
        let key = name
            .as_ref()
            .trim()
            .trim_start_matches(',')
            .trim_end_matches(',')
            .trim();
        match key {
            "name" => {
                if let Some(v) = as_str(value) {
                    animation.name = std::borrow::Cow::Owned(v.to_string());
                }
            }
            "start" | "offset" | "origin" => {
                if let Some((x, y)) = value.as_vec2() {
                    animation.start = [x, y];
                }
            }
            "atlas_region" | "texture_region" | "region" => {
                if let Some((x, y, _, _)) = value.as_vec4() {
                    animation.start = [x, y];
                }
            }
            "frame_size" | "cell_size" => {
                if let Some((w, h)) = value.as_vec2()
                    && w > 0.0
                    && h > 0.0
                {
                    animation.frame_size = [w, h];
                }
            }
            "frame_count" | "frames" => {
                if let Some(v) = as_i32(value) {
                    animation.frame_count = u32::try_from(v.max(1)).unwrap_or(1);
                }
            }
            "columns" | "cols" => {
                if let Some(v) = as_i32(value) {
                    animation.columns = u32::try_from(v.max(1)).unwrap_or(1);
                }
            }
            "fps" => {
                if let Some(v) = value.as_f32() {
                    animation.fps = v.max(0.0);
                }
            }
            _ => {}
        }
    }
    animation.frame_count = animation.frame_count.max(1);
    Some(animation)
}
