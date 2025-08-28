use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wgpu::RenderPass;

use crate::{ast::FurAnchor, graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH}, ui_element::{BaseElement, UIElement}, ui_elements::ui_panel::UIPanel, ui_node::Ui, Color, Graphics, Transform2D, Vector2};


pub fn update_global_transforms(
    elements: &mut IndexMap<String, UIElement>,
    current_id: &str,
    parent_global: &Transform2D,
) {
    // First, figure out parent size without holding a mutable borrow
    let parent_size = {
        let parent_id = elements
            .get(current_id)
            .and_then(|el| el.get_parent().cloned());

        if let Some(parent_id) = parent_id {
            if let Some(parent) = elements.get(&parent_id) {
                *parent.get_size()
            } else {
                Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT)
            }
        } else {
            Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT)
        }
    };

    // Now borrow mutably
    if let Some(element) = elements.get_mut(current_id) {
        let mut local = element.get_transform().clone();

        let child_size = *element.get_size();
        let pivot = *element.get_pivot();

let (anchor_x, anchor_y) = match element.get_anchor() {
    // Corners
    FurAnchor::TopLeft => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::TopRight => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::BottomLeft => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),
    FurAnchor::BottomRight => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),

    // Edges
    FurAnchor::Top => (
        0.0,
        parent_size.y * 0.5 - child_size.y * (1.0 - pivot.y),
    ),
    FurAnchor::Bottom => (
        0.0,
        -parent_size.y * 0.5 + child_size.y * pivot.y,
    ),
    FurAnchor::Left => (
        -parent_size.x * 0.5 + child_size.x * pivot.x,
        0.0,
    ),
    FurAnchor::Right => (
        parent_size.x * 0.5 - child_size.x * (1.0 - pivot.x),
        0.0,
    ),

    // Center
    FurAnchor::Center => (
        0.0,
        0.0,
    ),
};

        // Apply anchor offset + user translation
        local.position.x = anchor_x + local.position.x;
        local.position.y = anchor_y + local.position.y;

        // --- Combine with parent transform ---
        let mut global = Transform2D::default();

        global.scale.x = parent_global.scale.x * local.scale.x;
        global.scale.y = parent_global.scale.y * local.scale.y;

        global.position.x =
            parent_global.position.x + (local.position.x * parent_global.scale.x);
        global.position.y =
            parent_global.position.y + (local.position.y * parent_global.scale.y);

        global.rotation = parent_global.rotation + local.rotation;

        element.set_global_transform(global.clone());

        // Recurse into children
        for child_id in element.get_children().to_vec() {
            update_global_transforms(elements, &child_id, &global);
        }
    }
}
pub fn update_ui_layout(ui_node: &mut Ui) {
    for root_id in &ui_node.root_ids {
        update_global_transforms(&mut ui_node.elements, root_id.as_str(), &Transform2D::default());
    }
}

pub fn render_ui(ui_node: &Ui, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
    for (_, element) in &ui_node.elements {
        if !element.get_visible() {
            continue;
        }
        match element {
            UIElement::Panel(panel) => render_panel(panel, gfx, pass),
        }
    }
}

fn render_panel(panel: &UIPanel, gfx: &mut Graphics, pass: &mut RenderPass<'_>) {
    // Extract props info
    let background_color = panel
        .props
        .background_color
        .clone()
        .unwrap_or(Color::new(0, 0, 0, 0));
    let corner_radius = panel.props.corner_radius;
    let border_color = panel.props.border_color.clone();
    let border_thickness = panel.props.border_thickness;

    // Step 1: Draw background rectangle
    gfx.draw_rect(
        pass,
        panel.global_transform.clone(),
        panel.size.clone(),
        panel.pivot,
        background_color,
        Some(corner_radius),
    );

    // Step 2: Optional border (only if thickness > 0 and color is set)
    if border_thickness > 0.0 {
        if let Some(border_color) = border_color {
            gfx.draw_border(
                pass,
                panel.global_transform.clone(),
                panel.size.clone(),
                panel.pivot,
                border_color,
                border_thickness,
                Some(corner_radius),
            );
        }
    }
}