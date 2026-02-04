use crate::ids::UIElementID;
use std::collections::HashMap;

use crate::{
    Graphics,
    fur_ast::FurAnchor,
    structs2d::{Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_node::UINode,
};

fn render_ui_node(ui_node: &mut UINode, gfx: &mut Graphics) {
    // Check if fur_path has changed or if elements need to be loaded
    {
        let current_fur_path_str = ui_node.fur_path.as_ref().map(|fp| fp.as_ref().to_string());
        let loaded_fur_path_str = ui_node
            .loaded_fur_path
            .as_ref()
            .map(|fp| fp.as_ref().to_string());

        let needs_load = current_fur_path_str
            .as_ref()
            .map(|current| {
                loaded_fur_path_str
                    .as_ref()
                    .map(|loaded| loaded != current)
                    .unwrap_or(true)
            })
            .unwrap_or(false);

        if needs_load {
            if current_fur_path_str.is_some() {
                // FUR file needs to be loaded, elements not ready yet
                return;
            }
        }
    }

    let viewport_size = Vector2::new(gfx.virtual_width, gfx.virtual_height);

    // Always recalculate layout for all elements
    if let Some(elements) = &mut ui_node.elements {
        // Calculate sizes for all containers (panels and buttons)
        for (_element_id, element) in elements.iter_mut() {
            if matches!(element, UIElement::Panel(_) | UIElement::Button(_)) {
                let size = calculate_element_size(element, viewport_size);
                element.set_size(size);
            }
        }

        // Update transforms for all elements starting from roots
        if let Some(root_ids) = &ui_node.root_ids {
            for root_id in root_ids {
                update_transforms_recursive(
                    elements,
                    root_id,
                    &Transform2D::default(),
                    viewport_size,
                    0,
                );
            }
        }
    }

    // Run egui frame and render UI
    if let (Some(elements), Some(root_ids)) = (&ui_node.elements, &ui_node.root_ids) {
        let virtual_size = Vector2::new(gfx.virtual_width, gfx.virtual_height);
        let screen_size = Vector2::new(
            gfx.surface_config.width as f32,
            gfx.surface_config.height as f32,
        );
        
        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(screen_size.x, screen_size.y),
        ));
        if let Some(vp) = raw_input.viewports.get_mut(&egui::ViewportId::ROOT) {
            // If you have a real window scale factor, set it here instead of 1.0.
            vp.native_pixels_per_point = Some(1.0);
        }
        
        let integration = &mut gfx.egui_integration;
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(screen_size.x, screen_size.y),
        );

        let full_output = gfx.egui_context.run(raw_input, |ctx| {
            // Use CentralPanel instead of Area for full-screen UI
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    ui.set_clip_rect(screen_rect);
                    
                    integration.render_element_tree(
                        elements,
                        root_ids,
                        virtual_size,
                        screen_size,
                        ui,
                        &mut None,
                    );
                });
        });
        
        integration.last_output = Some(full_output);
    }
}

/// Calculate size for an element based on its style_map
/// 
/// Encoding in style_map (set by builder.rs):
/// - Percentage (e.g., "15%"): stored as raw value 1-100 (e.g., 15.0)
/// - Absolute pixels: stored as value + 10000 (e.g., 32px → 10032.0)
/// - Auto-sizing: stored as -1.0
fn calculate_element_size(element: &UIElement, viewport_size: Vector2) -> Vector2 {
    let style = element.get_style_map();
    let current = element.get_size();
    
    let resolve = |key: &str, ref_dim: f32, fallback: f32| -> f32 {
        match style.get(key) {
            Some(&v) => {
                if v < 0.0 {
                    // Auto-sizing sentinel (-1.0) or other negative values
                    // For now, treat as fallback
                    fallback
                } else if v > 10000.0 {
                    // Absolute size: value - 10000
                    // e.g., 10032.0 → 32.0 pixels
                    v - 10000.0
                } else if v >= 1.0 && v <= 100.0 {
                    // Percentage: value is in 1-100 range
                    // e.g., 15.0 → 15% → 0.15 * viewport
                    ref_dim * (v / 100.0)
                } else {
                    // Edge case: values between 0.0 and 1.0
                    // Treat as absolute pixel values (for tiny sizes like 0.5px)
                    v
                }
            }
            None => fallback,
        }
    };
    
    Vector2::new(
        resolve("size.x", viewport_size.x, current.x),
        resolve("size.y", viewport_size.y, current.y),
    )
}

/// Anchor position in virtual coords (center origin, Y-up). Half viewport = hw, hh.
fn anchor_position(anchor: FurAnchor, viewport_size: Vector2) -> Vector2 {
    let hw = viewport_size.x * 0.5;
    let hh = viewport_size.y * 0.5;
    match anchor {
        FurAnchor::TopLeft => Vector2::new(-hw, hh),
        FurAnchor::Top => Vector2::new(0.0, hh),
        FurAnchor::TopRight => Vector2::new(hw, hh),
        FurAnchor::Left => Vector2::new(-hw, 0.0),
        FurAnchor::Center => Vector2::new(0.0, 0.0),
        FurAnchor::Right => Vector2::new(hw, 0.0),
        FurAnchor::BottomLeft => Vector2::new(-hw, -hh),
        FurAnchor::Bottom => Vector2::new(0.0, -hh),
        FurAnchor::BottomRight => Vector2::new(hw, -hh),
    }
}

/// Update global_transform for the tree: position from anchor + viewport
fn update_transforms_recursive(
    elements: &mut HashMap<UIElementID, UIElement>,
    element_id: &UIElementID,
    parent_transform: &Transform2D,
    viewport_size: Vector2,
    depth: usize,
) {
    let Some(element) = elements.get_mut(element_id) else {
        return;
    };
    
    let anchor = *element.get_anchor();
    let local = element.get_transform().clone();
    
    let position = if depth == 0 {
        // Root element: position from anchor + local offset
        anchor_position(anchor, viewport_size) + local.position
    } else {
        // Child element: position relative to parent
        parent_transform.position + local.position
    };

    let global = Transform2D {
        position,
        scale: local.scale,
        rotation: local.rotation,
    };
    
    element.set_global_transform(global);

    let child_ids: Vec<UIElementID> = element.get_children().to_vec();
    for child_id in child_ids {
        update_transforms_recursive(
            elements,
            &child_id,
            &global,
            viewport_size,
            depth + 1,
        );
    }
}

/// Public entry point for scene: run layout + render for a UINode.
pub fn calculate_ui(
    ui_node: &mut UINode,
    gfx: &mut Graphics,
    _provider: Option<&dyn crate::script::ScriptProvider>,
) {
    render_ui_node(ui_node, gfx);
}
