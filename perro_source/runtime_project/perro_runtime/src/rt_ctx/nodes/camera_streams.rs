use super::*;

impl Runtime {
    pub(super) fn mark_camera_stream_users_dirty(&mut self, camera: NodeID) {
        let users: Vec<_> = self
            .nodes
            .iter()
            .filter_map(|(node, scene_node)| match &scene_node.data {
                SceneNodeData::CameraStream2D(stream) if stream.stream.camera == camera => {
                    Some((node, false))
                }
                SceneNodeData::CameraStream3D(stream) if stream.stream.camera == camera => {
                    Some((node, false))
                }
                SceneNodeData::UiCameraStream(stream) if stream.stream.camera == camera => {
                    Some((node, true))
                }
                _ => None,
            })
            .collect();
        for (node, ui) in users {
            self.mark_needs_rerender(node);
            if ui {
                self.mark_ui_dirty(node, Self::UI_DIRTY_COMMANDS);
            }
        }
    }
}
