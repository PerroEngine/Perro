fn build_node_2d(data: &SceneDefNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
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
                    node.bone_index = v;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::ChainLength)) => {
                if let Some(v) = as_i32(value) {
                    node.chain_length = v.max(0) as u32;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Iterations)) => {
                if let Some(v) = as_i32(value) {
                    node.iterations = v.max(0) as u32;
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Tolerance)) => {
                if let Some(v) = value.as_f32() {
                    node.tolerance = v.max(0.0);
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::Weight)) => {
                if let Some(v) = value.as_f32() {
                    node.weight = v.clamp(0.0, 1.0);
                }
            }
            Some(NodeField::IKTarget2D(IKTarget2DField::MatchRotation)) => {
                if let Some(v) = value.as_bool() {
                    node.match_rotation = v;
                }
            }
            _ => {}
        }
    });
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
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name == "rotation_deg" {
            if let Some(v) = value.as_f32() {
                node.transform.rotation = v.to_radians();
            }
            return;
        }

        match resolve_node_field("Node2D", name) {
            Some(NodeField::Node2D(Node2DField::Position)) => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.position = Vector2 { x, y };
                }
            }
            Some(NodeField::Node2D(Node2DField::Scale)) => {
                if let Some((x, y)) = value.as_vec2() {
                    node.transform.scale = Vector2 { x, y };
                }
            }
            Some(NodeField::Node2D(Node2DField::Rotation)) => {
                if let Some(v) = value.as_f32() {
                    node.transform.rotation = v;
                }
            }
            Some(NodeField::Node2D(Node2DField::ZIndex)) => {
                if let Some(v) = value.as_i32() {
                    node.z_index = v;
                }
            }
            Some(NodeField::Node2D(Node2DField::Visible)) => {
                if let Some(v) = value.as_bool() {
                    node.visible = v;
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
            Some(NodeField::TileMap2D(TileMap2DField::CollisionLayer)) => {
                if let Some(v) = as_u32(value) {
                    node.collision_layer = v;
                }
            }
            Some(NodeField::TileMap2D(TileMap2DField::CollisionMask)) => {
                if let Some(v) = as_u32(value) {
                    node.collision_mask = v;
                }
            }
            _ => {}
        }
    });
}

fn as_particle_sim_mode_2d(value: &SceneValue) -> Option<ParticleEmitterSimMode2D> {
    match as_str(value)?.trim().to_ascii_lowercase().as_str() {
        "default" => Some(ParticleEmitterSimMode2D::Default),
        "cpu" => Some(ParticleEmitterSimMode2D::Cpu),
        _ => None,
    }
}

fn apply_sprite_2d_fields(node: &mut Sprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if resolve_node_field("Sprite2D", name)
            == Some(NodeField::Sprite2D(Sprite2DField::TextureRegion))
            && let Some((x, y, w, h)) = value.as_vec4()
            && w > 0.0
            && h > 0.0
        {
            node.texture_region = Some([x, y, w, h]);
        }
    });
}

fn apply_animated_sprite_2d_fields(node: &mut AnimatedSprite2D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("AnimatedSprite2D", name) {
            Some(NodeField::AnimatedSprite2D(AnimatedSprite2DField::Animations)) => {
                if let Some(animations) = parse_animated_sprite_list(value) {
                    node.animations = animations;
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
