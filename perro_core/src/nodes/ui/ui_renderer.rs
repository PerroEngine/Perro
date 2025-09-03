use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wgpu::RenderPass;

use crate::{
    ast::FurAnchor, 
    graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH}, 
    ui_element::{BaseElement, BaseUIElement, UIElement}, 
    ui_elements::ui_panel::UIPanel, 
    ui_node::Ui, 
    Color, Graphics, Transform2D, Vector2
};
pub fn update_global_transforms(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
) {
    // First, figure out parent size and z without holding a mutable borrow
    let (parent_size, parent_z) = {
        let parent_id = elements
            .get(current_id)
            .and_then(|el| el.get_parent());

        if let Some(parent_id) = parent_id {
            if let Some(parent) = elements.get(&parent_id) {
                (*parent.get_size(), parent.get_z_index())
            } else {
                (Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT), 0)
            }
        } else {
            (Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT), 0)
        }
    };

    // Now borrow mutably
    if let Some(element) = elements.get_mut(current_id) {
        let parent_size_for_percentages = parent_size;

        let style_map = element.get_style_map().clone(); // clone to break the borrow
        for (key, pct) in style_map.iter() {
            let fraction = *pct / 100.0;

            match key.as_str() {
                // Size
                "size.x" => element.set_size(Vector2::new(parent_size_for_percentages.x * fraction, element.get_size().y)),
                "size.y" => element.set_size(Vector2::new(element.get_size().x, parent_size_for_percentages.y * fraction)),

                // Translation (position)
                "transform.position.x" => element.get_transform_mut().position.x = parent_size_for_percentages.x * fraction,
                "transform.position.y" => element.get_transform_mut().position.y = parent_size_for_percentages.y * fraction,

                // Scale (relative to parent scale, not size)
                "transform.scale.x" => {
                    let parent_scale_x = parent_global.scale.x;
                    element.get_transform_mut().scale.x = 1.0 * fraction * parent_scale_x;
                },
                "transform.scale.y" => {
                    let parent_scale_y = parent_global.scale.y;
                    element.get_transform_mut().scale.y = 1.0 * fraction * parent_scale_y;
                },

                // Padding
                "padding.left" => element.get_padding_mut().left = parent_size_for_percentages.x * fraction,
                "padding.right" => element.get_padding_mut().right = parent_size_for_percentages.x * fraction,
                "padding.top" => element.get_padding_mut().top = parent_size_for_percentages.y * fraction,
                "padding.bottom" => element.get_padding_mut().bottom = parent_size_for_percentages.y * fraction,

                _ => {}
            }
        }

        // Local transform
        let mut local = element.get_transform().clone();
        let local_z = element.get_z_index();
        let child_size = *element.get_size();
        let pivot = *element.get_pivot();

        // The key insight: 
        // - pivot (0.5, 0) means the transform position is at the bottom-center of the element
        // - pivot (0.5, 1) means the transform position is at the top-center of the element
        // - We need to calculate where to put the transform position so the element appears in the right place
let (anchor_x, anchor_y) = match element.get_anchor() {
    // Corners
    FurAnchor::TopLeft => {
        let target_x = -parent_size.x * 0.5; // Left edge
        let target_y = parent_size.y * 0.5;  // Top edge
        let offset_x = target_x + child_size.x * pivot.x;
        let offset_y = target_y - child_size.y * (1.0 - pivot.y); // pivot.y=1.0 means top
        (offset_x, offset_y)
    },
    FurAnchor::TopRight => {
        let target_x = parent_size.x * 0.5;  // Right edge
        let target_y = parent_size.y * 0.5;  // Top edge
        let offset_x = target_x - child_size.x * (1.0 - pivot.x);
        let offset_y = target_y - child_size.y * (1.0 - pivot.y);
        (offset_x, offset_y)
    },
    FurAnchor::BottomLeft => {
        let target_x = -parent_size.x * 0.5; // Left edge
        let target_y = -parent_size.y * 0.5; // Bottom edge
        let offset_x = target_x + child_size.x * pivot.x;
        let offset_y = target_y + child_size.y * pivot.y; // pivot.y=0.0 means bottom
        (offset_x, offset_y)
    },
    FurAnchor::BottomRight => {
        let target_x = parent_size.x * 0.5;  // Right edge
        let target_y = -parent_size.y * 0.5; // Bottom edge
        let offset_x = target_x - child_size.x * (1.0 - pivot.x);
        let offset_y = target_y + child_size.y * pivot.y;
        (offset_x, offset_y)
    },

    // Edges
    FurAnchor::Top => {
        let target_y = parent_size.y * 0.5;  // Top edge
        let offset_y = target_y - child_size.y * (1.0 - pivot.y);
        (0.0, offset_y)
    },
    FurAnchor::Bottom => {
        let target_y = -parent_size.y * 0.5; // Bottom edge
        let offset_y = target_y + child_size.y * pivot.y;
        (0.0, offset_y)
    },
    FurAnchor::Left => {
        let target_x = -parent_size.x * 0.5; // Left edge
        let offset_x = target_x + child_size.x * pivot.x;
        (offset_x, 0.0)
    },
    FurAnchor::Right => {
        let target_x = parent_size.x * 0.5;  // Right edge
        let offset_x = target_x - child_size.x * (1.0 - pivot.x);
        (offset_x, 0.0)
    },

    // Center - no offset needed
    FurAnchor::Center => (0.0, 0.0),
};
        // Apply anchor offset + user translation
        local.position.x += anchor_x;
        local.position.y += anchor_y;

        // --- Combine with parent transform ---
        let mut global = Transform2D::default();
        global.scale.x = parent_global.scale.x * local.scale.x;
        global.scale.y = parent_global.scale.y * local.scale.y;
        global.position.x = parent_global.position.x + (local.position.x * parent_global.scale.x);
        global.position.y = parent_global.position.y + (local.position.y * parent_global.scale.y);
        global.rotation = parent_global.rotation + local.rotation;

        element.set_global_transform(global.clone());

        // Set inherited z-index: local z + parent z
        let global_z = local_z + parent_z + 2;
        element.set_z_index(global_z);

        println!("Updating {:?} -> {:?}", current_id, element.get_global_transform().position);

        // Recurse into children (pass the current element's global transform)
        for child_id in element.get_children().to_vec() {
            update_global_transforms(elements, &child_id, &global);
        }
    }
}

pub fn update_ui_layout(ui_node: &mut Ui) {
    for root_id in &ui_node.root_ids {
        update_global_transforms(&mut ui_node.elements, root_id, &Transform2D::default());
    }
}

pub fn render_ui(ui_node: &mut Ui, gfx: &mut Graphics) {
    update_ui_layout(ui_node); // now works
    for (_, element) in &ui_node.elements {
        if !element.get_visible() {
            continue;
        }
        match element {
            UIElement::Panel(panel) => render_panel(panel, gfx),
            UIElement::BoxContainer(_) => { /* no-op */ },
            UIElement::GridContainer(_) => { /* no-op */ },
        }
    }
}

fn render_panel(panel: &UIPanel, gfx: &mut Graphics) {
    let background_color = panel.props.background_color.clone().unwrap_or(Color::new(0, 0, 0, 0));
    let corner_radius = panel.props.corner_radius;
    let border_color = panel.props.border_color.clone();
    let border_thickness = panel.props.border_thickness;
    let z_index = panel.base.z_index;
    let bg_id = panel.id;
    let border_id = Uuid::new_v5(&bg_id, b"border");

    gfx.draw_rect(
        bg_id,
        panel.base.global_transform.clone(),
        panel.base.size,
        panel.base.pivot,
        background_color,
        Some(corner_radius),
        0.0,
        false,
        z_index, // Pass z-index
    );

    if border_thickness > 0.0 {
        if let Some(border_color) = border_color {
            gfx.draw_rect(
                border_id,
                panel.base.global_transform.clone(),
                panel.base.size,
                panel.base.pivot,
                border_color,
                Some(corner_radius),
                border_thickness,
                true,
                z_index + 1, // Border slightly above background
            );
        }
    }
}
