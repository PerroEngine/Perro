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

fn build_skeleton_3d(data: &SceneDefNodeData) -> Skeleton3D {
    let mut node = Skeleton3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_skeleton_3d_fields(&mut node, &data.fields);
    node
}

fn build_terrain_instance_3d(data: &SceneDefNodeData) -> TerrainInstance3D {
    let mut node = TerrainInstance3D::new();
    if let Some(base) = data.base_ref() {
        apply_node_3d_data(&mut node, base);
    }
    apply_node_3d_fields(&mut node, &data.fields);
    apply_terrain_instance_3d_fields(&mut node, &data.fields);
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

fn apply_skeleton_3d_fields(_node: &mut Skeleton3D, _fields: &[SceneObjectField]) {}

fn apply_terrain_instance_3d_fields(node: &mut TerrainInstance3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("TerrainInstance3D", name) {
            Some(NodeField::TerrainInstance3D(TerrainInstance3DField::Terrain)) => {
                node.terrain_source = as_asset_source(value).map(std::borrow::Cow::Owned);
            }
            Some(NodeField::TerrainInstance3D(
                TerrainInstance3DField::ShowDebugVertices,
            )) => {
                if let Some(v) = as_bool(value) {
                    node.show_debug_vertices = v;
                }
            }
            Some(NodeField::TerrainInstance3D(
                TerrainInstance3DField::ShowDebugEdges,
            )) => {
                if let Some(v) = as_bool(value) {
                    node.show_debug_edges = v;
                }
            }
            _ => {}
        }
    });
}

fn extract_mesh_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
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
    if data.ty != "MeshInstance3D" {
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
    if data.ty != "MeshInstance3D" {
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
    if data.ty != "MeshInstance3D" {
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
        SceneValue::Str(_) | SceneValue::Key(_) => PendingSurfaceMaterial {
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
    match value {
        SceneValue::Bool(v) => Some(MaterialParamOverrideValue::Bool(*v)),
        SceneValue::I32(v) => Some(MaterialParamOverrideValue::I32(*v)),
        SceneValue::F32(v) => Some(MaterialParamOverrideValue::F32(*v)),
        SceneValue::Vec2 { x, y } => Some(MaterialParamOverrideValue::Vec2([*x, *y])),
        SceneValue::Vec3 { x, y, z } => Some(MaterialParamOverrideValue::Vec3([*x, *y, *z])),
        SceneValue::Vec4 { x, y, z, w } => {
            Some(MaterialParamOverrideValue::Vec4([*x, *y, *z, *w]))
        }
        _ => None,
    }
}

fn parse_color(value: &SceneValue) -> Option<[f32; 4]> {
    match value {
        SceneValue::Vec4 { x, y, z, w } => Some([*x, *y, *z, *w]),
        SceneValue::Vec3 { x, y, z } => Some([*x, *y, *z, 1.0]),
        _ => None,
    }
}

fn extract_model_source(data: &SceneDefNodeData) -> Option<String> {
    if data.ty != "MeshInstance3D" {
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
