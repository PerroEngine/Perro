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

fn apply_mesh_instance_3d_fields(_node: &mut MeshInstance3D, _fields: &[SceneObjectField]) {}

fn apply_skeleton_3d_fields(_node: &mut Skeleton3D, _fields: &[SceneObjectField]) {}

fn apply_terrain_instance_3d_fields(node: &mut TerrainInstance3D, fields: &[SceneObjectField]) {
    SceneFieldIterRef::new(fields).for_each(|name, value| {
        match resolve_node_field("TerrainInstance3D", name) {
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
