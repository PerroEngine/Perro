fn build_node_3d(data: &SceneDefNodeData) -> Node3D {
    let mut node = Node3D::new();
    apply_node_3d_data(&mut node, data);
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

fn build_skeleton_3d(data: &SceneDefNodeData) -> Skeleton3D {
    let mut node = Skeleton3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_skeleton_3d_fields(&mut node, &data.fields);
    node
}

fn apply_node_3d_data(target: &mut Node3D, data: &SceneDefNodeData) {
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(target, base);
    }
    apply_node_3d_fields(target, &data.fields);
}

fn apply_node_3d_fields(node: &mut Node3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("Node3D", name) {
            Some(NodeField::Node3D(Node3DField::Position)) => {
                if let Some(v) = as_vec3(value) {
                    node.transform.position = v;
                }
            }
            Some(NodeField::Node3D(Node3DField::Scale)) => {
                if let Some(v) = as_vec3(value) {
                    node.transform.scale = v;
                }
            }
            Some(NodeField::Node3D(Node3DField::Rotation)) => {
                if let Some(v) = as_quat(value) {
                    node.transform.rotation = v;
                }
            }
            Some(NodeField::Node3D(Node3DField::Visible)) => {
                if let Some(v) = as_bool(value) {
                    node.visible = v;
                }
            }
            _ => {}
        }
    });
}

fn apply_mesh_instance_3d_fields(node: &mut MeshInstance3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        if name != "surfaces" {
            return;
        }
        if let SceneValue::Array(items) = value {
            node.surfaces = parse_surface_bindings(items.as_ref());
        }
    });
}

fn apply_multi_mesh_instance_3d_fields(
    node: &mut MultiMeshInstance3D,
    fields: &[SceneObjectField],
) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match name {
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
            "instance_scale" => {
                if let Some(v) = as_f32(value) {
                    node.instance_scale = v.max(0.0001);
                }
            }
            _ => {}
        }
    });
}

fn apply_skeleton_3d_fields(_node: &mut Skeleton3D, _fields: &[SceneObjectField]) {}

fn extract_mesh_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" && data.ty != "MultiMeshInstance3D" {
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
    if data.ty != "MeshInstance3D" && data.ty != "MultiMeshInstance3D" {
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
    if data.ty != "MeshInstance3D" && data.ty != "MultiMeshInstance3D" {
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
    if data.ty != "MeshInstance3D" && data.ty != "MultiMeshInstance3D" {
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

fn parse_color(value: &SceneValue) -> Option<[f32; 4]> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        SceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn parse_instance_posrot(items: &[SceneValue]) -> Vec<(perro_structs::Vector3, perro_structs::Quaternion)> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            SceneValue::Vec3 { x, y, z } => {
                out.push((
                    perro_structs::Vector3::new(*x, *y, *z),
                    perro_structs::Quaternion::IDENTITY,
                ));
            }
            SceneValue::Object(entries) => {
                let mut pos = perro_structs::Vector3::ZERO;
                let mut rot = perro_structs::Quaternion::IDENTITY;
                let mut rot_deg: Option<perro_structs::Vector3> = None;
                for (key, value) in entries.iter() {
                    match key.as_ref() {
                        "position" => {
                            if let Some(v) = as_vec3(value) {
                                pos = v;
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
                        _ => {}
                    }
                }
                if let Some(deg) = rot_deg {
                    rot = quat_from_deg_xyz(deg);
                }
                out.push((pos, rot));
            }
            _ => {}
        }
    }
    out
}

#[inline]
fn quat_from_deg_xyz(deg: perro_structs::Vector3) -> perro_structs::Quaternion {
    let to_rad = std::f32::consts::PI / 180.0;
    perro_structs::Quaternion::from_euler_xyz(deg.x * to_rad, deg.y * to_rad, deg.z * to_rad)
}

fn extract_model_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" && data.ty != "MultiMeshInstance3D" {
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
    if data.ty != "Skeleton3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("Skeleton3D", name)
            == Some(NodeField::Skeleton3D(Skeleton3DField::Skeleton)))
            .then(|| as_asset_source(value))
            .flatten()
    })
}

fn extract_mesh_skeleton_target(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
        return None;
    }
    data.fields.iter().find_map(|(name, value)| {
        (resolve_node_field("MeshInstance3D", name)
            == Some(NodeField::MeshInstance3D(MeshInstance3DField::Skeleton)))
            .then(|| as_asset_source(value))
            .flatten()
    })
}
