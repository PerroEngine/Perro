use crate::ids::UIElementID;
use std::collections::HashMap;

use crate::{
    Graphics,
    fur_ast::FurAnchor,
    nodes::ui::apply_fur::{build_ui_elements_from_fur, parse_fur_file},
    structs2d::{Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_node::UINode,
};

fn render_ui_node(ui_node: &mut UINode, gfx: &mut Graphics) {
    // Reload FUR if the path changed, but always continue with layout/render.
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
            println!("🔄 Reloading FUR file: {}", current_fur_path_str.as_ref().unwrap());
            if let Some(path) = current_fur_path_str.as_deref() {
                match parse_fur_file(path) {
                    Ok(ast) => {
                        let elements: Vec<crate::fur_ast::FurElement> = ast
                            .into_iter()
                            .filter_map(|node| {
                                if let crate::fur_ast::FurNode::Element(elem) = node {
                                    Some(elem)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        build_ui_elements_from_fur(ui_node, &elements);
                        ui_node.loaded_fur_path = ui_node.fur_path.clone();
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to load FUR file {}: {}", path, e);
                    }
                }
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
                    Vector2::new(0.5, 0.5),
                    viewport_size,
                    0,
                );
            }
        }
    }

    // Run egui frame and render UI
    if let (Some(elements), Some(root_ids)) = (&ui_node.elements, &ui_node.root_ids) {
        let virtual_size = Vector2::new(gfx.virtual_width, gfx.virtual_height);
        let screen_size_pixels = Vector2::new(
            gfx.surface_config.width as f32,
            gfx.surface_config.height as f32,
        );
        let pixels_per_point = gfx.window().scale_factor() as f32;
        let screen_size_points = Vector2::new(
            screen_size_pixels.x / pixels_per_point,
            screen_size_pixels.y / pixels_per_point,
        );
        
        let mut raw_input = egui::RawInput::default();
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(screen_size_points.x, screen_size_points.y),
        ));
        if !raw_input.viewports.contains_key(&egui::ViewportId::ROOT) {
            raw_input.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo::default(),
            );
        }
        if let Some(vp) = raw_input.viewports.get_mut(&egui::ViewportId::ROOT) {
            vp.native_pixels_per_point = Some(pixels_per_point);
        }
        
        let integration = &mut gfx.egui_integration;
        let screen_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(screen_size_points.x, screen_size_points.y),
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
                        screen_size_points,
                        ui,
                        &mut None,
                    );
                });
        });

        // Accumulate texture updates across all UI nodes for this frame.
        if !full_output.textures_delta.is_empty() {
            gfx.egui_textures_delta_pending
                .append(full_output.textures_delta.clone());
        }

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
/// Uses size + pivot so anchored edges align correctly.
fn anchor_position(anchor: FurAnchor, viewport_size: Vector2, size: Vector2, pivot: Vector2) -> Vector2 {
    let hw = viewport_size.x * 0.5;
    let hh = viewport_size.y * 0.5;
    let x = match anchor {
        FurAnchor::Left | FurAnchor::TopLeft | FurAnchor::BottomLeft => -hw + size.x * pivot.x,
        FurAnchor::Right | FurAnchor::TopRight | FurAnchor::BottomRight => {
            hw - size.x * (1.0 - pivot.x)
        }
        FurAnchor::Center | FurAnchor::Top | FurAnchor::Bottom => 0.0,
    };
    let y = match anchor {
        FurAnchor::Bottom | FurAnchor::BottomLeft | FurAnchor::BottomRight => -hh + size.y * pivot.y,
        FurAnchor::Top | FurAnchor::TopLeft | FurAnchor::TopRight => {
            hh - size.y * (1.0 - pivot.y)
        }
        FurAnchor::Center | FurAnchor::Left | FurAnchor::Right => 0.0,
    };
    Vector2::new(x, y)
}

/// Anchor position within a parent rect (parent origin is its pivot point).
/// Uses parent/child size + pivots so edges align correctly.
fn anchor_position_in_parent(
    anchor: FurAnchor,
    parent_size: Vector2,
    parent_pivot: Vector2,
    child_size: Vector2,
    child_pivot: Vector2,
) -> Vector2 {
    let parent_left = -parent_size.x * parent_pivot.x;
    let parent_right = parent_size.x * (1.0 - parent_pivot.x);
    let parent_bottom = -parent_size.y * parent_pivot.y;
    let parent_top = parent_size.y * (1.0 - parent_pivot.y);

    let x = match anchor {
        FurAnchor::Left | FurAnchor::TopLeft | FurAnchor::BottomLeft => {
            parent_left + child_size.x * child_pivot.x
        }
        FurAnchor::Right | FurAnchor::TopRight | FurAnchor::BottomRight => {
            parent_right - child_size.x * (1.0 - child_pivot.x)
        }
        FurAnchor::Center | FurAnchor::Top | FurAnchor::Bottom => 0.0,
    };

    let y = match anchor {
        FurAnchor::Bottom | FurAnchor::BottomLeft | FurAnchor::BottomRight => {
            parent_bottom + child_size.y * child_pivot.y
        }
        FurAnchor::Top | FurAnchor::TopLeft | FurAnchor::TopRight => {
            parent_top - child_size.y * (1.0 - child_pivot.y)
        }
        FurAnchor::Center | FurAnchor::Left | FurAnchor::Right => 0.0,
    };

    Vector2::new(x, y)
}

/// Update global_transform for the tree: position from anchor + viewport
fn update_transforms_recursive(
    elements: &mut HashMap<UIElementID, UIElement>,
    element_id: &UIElementID,
    parent_transform: &Transform2D,
    parent_size: Vector2,
    parent_pivot: Vector2,
    viewport_size: Vector2,
    depth: usize,
) {
    let Some(element) = elements.get_mut(element_id) else {
        return;
    };
    
    let anchor = *element.get_anchor();
    let local = element.get_transform().clone();
    let pivot = *element.get_pivot();
    let size = *element.get_size();
    let scaled_size = Vector2::new(size.x * local.scale.x, size.y * local.scale.y);
    
    let position = if depth == 0 {
        // Root element: position from anchor + local offset
        anchor_position(anchor, viewport_size, scaled_size, pivot) + local.position
    } else {
        // Child element: anchored within parent, then local offset
        parent_transform.position
            + anchor_position_in_parent(anchor, parent_size, parent_pivot, scaled_size, pivot)
            + local.position
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
            scaled_size,
            pivot,
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
