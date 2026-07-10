fn build_node_2d(data: &SceneDefNodeData) -> Node2D {
    let mut node = Node2D::new();
    apply_node_2d_data(&mut node, data);
    node
}

fn build_camera_stream_2d(data: &SceneDefNodeData) -> CameraStream2D {
    let mut node = CameraStream2D::default();
    if let Some(base) = data.base_ref() {
        apply_node_2d_data(&mut node, base);
    }
    apply_node_2d_fields(&mut node, &data.fields);
    apply_camera_stream_fields(&mut node.stream, &data.fields);
    SceneFieldIterRef::new(&data.fields).for_each(|name, value| match name {
        name if scene_key_in(name, COLOR_MODULATE_KEYS) => {
            if let Some(v) = as_scene_color(value) {
                node.tint = v;
            }
        }
        _ => {}
    });
    node
}

fn build_skeleton_2d(data: &SceneDefNodeData) -> Skeleton2D {
    let mut node = Skeleton2D::default();
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
                    node.params.iterations =
                        (v.max(0) as u32).min(perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS);
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
                    node.iterations =
                        (v.max(1) as u32).min(perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS);
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
            SceneFieldName::Custom(name) if name == "top_level" => {
                if let Some(v) = value.as_bool() {
                    node.top_level = v;
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
