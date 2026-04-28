use super::Runtime;
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_nodes::SceneNodeData;
use perro_render_bridge::{RenderCommand, UiCommand, UiRectState};
use perro_structs::Vector2;
use perro_ui::{ComputedUiRect, UiRoot, UiStyle};
use std::borrow::Cow;

impl Runtime {
    pub fn extract_render_ui_commands(&mut self) {
        let viewport = self.input.viewport_size();
        let root_rect = ComputedUiRect::new(Vector2::ZERO, viewport);
        let mut computed = AHashMap::<NodeID, ComputedUiRect>::default();
        let node_ids: Vec<NodeID> = self.nodes.iter().map(|(id, _)| id).collect();

        self.queue_render_command(RenderCommand::Ui(UiCommand::Clear));
        for node in node_ids.iter().copied() {
            self.compute_ui_rect(node, root_rect, &mut computed);
        }

        for node in node_ids {
            let Some(rect) = computed.get(&node).copied() else {
                continue;
            };
            let effective_visible = self.is_effectively_visible(node);
            let Some(command) = self.nodes.get(node).and_then(|scene_node| {
                ui_command_from_node(node, &scene_node.data, rect, effective_visible)
            }) else {
                continue;
            };
            self.queue_render_command(RenderCommand::Ui(command));
        }
    }

    fn compute_ui_rect(
        &self,
        node: NodeID,
        root_rect: ComputedUiRect,
        computed: &mut AHashMap<NodeID, ComputedUiRect>,
    ) -> Option<ComputedUiRect> {
        if let Some(rect) = computed.get(&node).copied() {
            return Some(rect);
        }

        let scene_node = self.nodes.get(node)?;
        let ui_root = ui_root_from_data(&scene_node.data)?;
        let parent_rect = if scene_node.parent.is_nil() {
            root_rect
        } else {
            self.compute_ui_rect(scene_node.parent, root_rect, computed)
                .unwrap_or(root_rect)
        };
        let rect = ui_root.layout.compute_rect(parent_rect);
        computed.insert(node, rect);
        Some(rect)
    }
}

fn ui_root_from_data(data: &SceneNodeData) -> Option<&UiRoot> {
    match data {
        SceneNodeData::UiRoot(root) => Some(root),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiLabel(node) => Some(&node.base),
        SceneNodeData::UiHBox(node) => Some(&node.inner.base),
        SceneNodeData::UiVBox(node) => Some(&node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&node.base),
        _ => None,
    }
}

fn ui_command_from_node(
    node: NodeID,
    data: &SceneNodeData,
    rect: ComputedUiRect,
    effective_visible: bool,
) -> Option<UiCommand> {
    if !effective_visible {
        return None;
    }

    let rect = UiRectState {
        center: [rect.center.x, rect.center.y],
        size: [rect.size.x, rect.size.y],
        z_index: ui_root_from_data(data)?.layout.z_index,
    };
    match data {
        SceneNodeData::UiPanel(panel) => Some(panel_command(node, rect, &panel.style)),
        SceneNodeData::UiButton(button) => {
            let style = if button.disabled {
                button.style.clone()
            } else {
                button.style.clone()
            };
            Some(UiCommand::UpsertButton {
                node,
                rect,
                text: Cow::Owned(button.text.to_string()),
                text_color: button.text_color.to_rgba(),
                fill: style.fill.to_rgba(),
                stroke: style.stroke.to_rgba(),
                stroke_width: style.stroke_width,
                corner_radius: style.corner_radius,
                disabled: button.disabled,
            })
        }
        SceneNodeData::UiLabel(label) => Some(UiCommand::UpsertLabel {
            node,
            rect,
            text: Cow::Owned(label.text.to_string()),
            color: label.color.to_rgba(),
            font_size: label.font_size,
        }),
        _ => None,
    }
}

fn panel_command(node: NodeID, rect: UiRectState, style: &UiStyle) -> UiCommand {
    UiCommand::UpsertPanel {
        node,
        rect,
        fill: style.fill.to_rgba(),
        stroke: style.stroke.to_rgba(),
        stroke_width: style.stroke_width,
        corner_radius: style.corner_radius,
    }
}
