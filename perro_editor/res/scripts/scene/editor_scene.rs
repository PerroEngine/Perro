use perro_api::prelude::perro_scene;
use perro_api::scene::SceneDoc;

pub fn has_3d(doc: &SceneDoc) -> bool {
    doc.scene
        .nodes
        .iter()
        .any(|node| node.data.node_type.is_a(perro_scene::NodeType::Node3D))
}

pub fn has_2d(doc: &SceneDoc) -> bool {
    doc.scene
        .nodes
        .iter()
        .any(|node| node.data.node_type.is_a(perro_scene::NodeType::Node2D))
}

pub fn has_type(doc: &SceneDoc, ty: perro_scene::NodeType) -> bool {
    doc.scene.nodes.iter().any(|node| node.data.node_type == ty)
}

pub fn root_viewport_mode(doc: &SceneDoc) -> &'static str {
    let Some(root_key) = doc.scene.root else {
        return "UI";
    };
    let Some(root) = doc.scene.nodes.iter().find(|node| node.key == root_key) else {
        return "UI";
    };
    let node_type = root.data.node_type;
    if node_type.is_a(perro_scene::NodeType::Node3D) {
        "3D"
    } else if node_type.is_a(perro_scene::NodeType::Node2D) {
        "2D"
    } else {
        "UI"
    }
}
