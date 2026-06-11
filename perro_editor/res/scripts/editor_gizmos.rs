use perro_api::scene::SceneDoc;

#[derive(Clone, Debug)]
pub struct GizmoView {
    pub selected: bool,
    pub camera_2d: bool,
    pub camera_3d: bool,
    pub outline_size: (f32, f32),
}

impl Default for GizmoView {
    fn default() -> Self {
        Self {
            selected: false,
            camera_2d: false,
            camera_3d: false,
            outline_size: (0.28, 0.24),
        }
    }
}

pub fn gizmo_view(doc: &SceneDoc, selected_key: Option<u32>) -> GizmoView {
    let Some(key) = selected_key else {
        return GizmoView::default();
    };
    let Some(node) = doc.scene.nodes.iter().find(|node| node.key.as_u32() == key) else {
        return GizmoView::default();
    };
    let type_name = node.data.type_name();
    let mut out = GizmoView {
        selected: true,
        camera_2d: type_name == "Camera2D",
        camera_3d: type_name == "Camera3D",
        outline_size: (0.28, 0.24),
    };
    if type_name == "Camera2D" {
        out.outline_size = (0.52, 0.38);
    } else if type_name == "Camera3D" {
        out.outline_size = (0.18, 0.14);
    } else if type_name.starts_with("Ui") {
        out.outline_size = (0.22, 0.14);
    } else if type_name.ends_with("3D") {
        out.outline_size = (0.18, 0.18);
    } else if type_name.ends_with("2D") {
        out.outline_size = (0.24, 0.18);
    }
    out
}
