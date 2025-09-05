use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wgpu::RenderPass;
use std::collections::HashMap;

use crate::{
    ast::FurAnchor, 
    graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH}, 
    ui_element::{BaseElement, UIElement}, 
    ui_elements::ui_container::{UIPanel, ContainerMode, Alignment}, 
    ui_node::Ui, 
    Color, 
    Graphics, 
    Transform2D, 
    Vector2
};

/// Calculate layout positions for children before anchor/translation transforms
pub fn calculate_layout_positions(
    elements: &IndexMap<Uuid, UIElement>,
    parent_id: &Uuid,
) -> Vec<(Uuid, Vector2)> {
    let parent = match elements.get(parent_id) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let children_ids = parent.get_children();
    if children_ids.is_empty() {
        return Vec::new();
    }

    // Get layout properties
    let (container_mode, alignment, gap) = match parent {
        UIElement::Layout(layout) => (
            &layout.container.mode,
            &layout.container.align,
            layout.container.gap,
        ),
        UIElement::GridLayout(grid) => (
            &grid.container.mode,
            &grid.container.align,
            grid.container.gap,
        ),
        _ => return Vec::new(), // Not a layout container
    };

    // Collect child sizes
    let mut child_info: Vec<(Uuid, Vector2)> = Vec::new();
    for &child_id in children_ids {
        if let Some(child) = elements.get(&child_id) {
            child_info.push((child_id, *child.get_size()));
        }
    }

    if child_info.is_empty() {
        return Vec::new();
    }

    let parent_size = *parent.get_size();

    match container_mode {
        ContainerMode::Horizontal => calculate_horizontal_layout(&child_info, parent_size, alignment, gap, elements),
        ContainerMode::Vertical => calculate_vertical_layout(&child_info, parent_size, alignment, gap, elements),
        ContainerMode::Grid => {
            if let UIElement::GridLayout(grid) = parent {
                calculate_grid_layout(&child_info, parent_size, alignment, gap, grid.cols, elements)
            } else {
                Vec::new()
            }
        }
    }
}

fn calculate_horizontal_layout(
    children: &[(Uuid, Vector2)],
    parent_size: Vector2,
    alignment: &Alignment,
    gap: Vector2,
    elements: &IndexMap<Uuid, UIElement>,
) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();
    
    // Calculate total width needed
    let total_child_width: f32 = children.iter().map(|(_, size)| size.x).sum();
    let total_gap_width = gap.x * (children.len() - 1) as f32;
    let total_content_width = total_child_width + total_gap_width;
    
    // Position each child based on horizontal alignment (LEFT/CENTER/RIGHT)
    let mut current_x = match alignment {
        Alignment::Start => {
            // LEFT alignment - start from left edge of parent
            parent_size.x * 0.5 - total_content_width
        },
        Alignment::Center => {
            // CENTER alignment - center the entire content block
            -total_content_width * 0.5
        },
        Alignment::End => {
            // RIGHT alignment - start from right edge, working backwards
            -parent_size.x * 0.5
        },
    };
    
    for (child_id, child_size) in children {
        // Get the child's pivot to calculate proper positioning
        let child_pivot = if let Some(child) = elements.get(child_id) {
            *child.get_pivot()
        } else {
            Vector2::new(0.5, 0.5) // Default pivot
        };
        
        // Calculate the transform position based on pivot
        let child_x = current_x + child_size.x * child_pivot.x;
        
        // Vertical positioning - just center all children (no cross-axis concept)
        let child_y = 0.0;
        
        positions.push((*child_id, Vector2::new(child_x, child_y)));
        current_x += child_size.x + gap.x;
    }
    
    positions
}

fn calculate_vertical_layout(
    children: &[(Uuid, Vector2)],
    parent_size: Vector2,
    alignment: &Alignment,
    gap: Vector2,
    elements: &IndexMap<Uuid, UIElement>,
) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();
    
    // Calculate total height needed
    let total_child_height: f32 = children.iter().map(|(_, size)| size.y).sum();
    let total_gap_height = gap.y * (children.len() - 1) as f32;
    let total_content_height = total_child_height + total_gap_height;
    
    // Position each child based on vertical alignment (TOP/CENTER/BOTTOM)
    let mut current_y = match alignment {
        Alignment::Start => {
            // TOP alignment - start from top edge of parent
            -parent_size.y * 0.5 + total_content_height
        },
        Alignment::Center => {
            // CENTER alignment - center the entire content block
            total_content_height * 0.5
        },
        Alignment::End => {
            // BOTTOM alignment - start from bottom edge, working upwards
            parent_size.y * 0.5
        },
    };
    
    for (child_id, child_size) in children {
        // Get the child's pivot to calculate proper positioning
        let child_pivot = if let Some(child) = elements.get(child_id) {
            *child.get_pivot()
        } else {
            Vector2::new(0.5, 0.5) // Default pivot
        };
        
        // Calculate the transform position based on pivot
        // For vertical layout, we work from top down
        let child_y = current_y - child_size.y * (1.0 - child_pivot.y);
        
        // Horizontal positioning - just center all children (no cross-axis concept)
        let child_x = 0.0;
        
        positions.push((*child_id, Vector2::new(child_x, child_y)));
        current_y -= child_size.y + gap.y;
    }
    
    positions
}

fn calculate_grid_layout(
    children: &[(Uuid, Vector2)],
    parent_size: Vector2,
    alignment: &Alignment,
    gap: Vector2,
    cols: usize,
    elements: &IndexMap<Uuid, UIElement>,
) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();
    
    if cols == 0 {
        return positions;
    }
    
    let rows = (children.len() + cols - 1) / cols; // Ceiling division
    
    // Find the maximum width and height for consistent grid cells
    let max_width = children.iter().map(|(_, size)| size.x).fold(0.0, f32::max);
    let max_height = children.iter().map(|(_, size)| size.y).fold(0.0, f32::max);
    
    // Calculate total grid dimensions
    let total_width = max_width * cols as f32 + gap.x * (cols - 1) as f32;
    let total_height = max_height * rows as f32 + gap.y * (rows - 1) as f32;
    
    // Determine grid starting position
    let grid_start_x = match alignment {
        Alignment::Start => -parent_size.x * 0.5,
        Alignment::Center => -total_width * 0.5,
        Alignment::End => parent_size.x * 0.5 - total_width,
    };
    
    let grid_start_y = match alignment {
        Alignment::Start => parent_size.y * 0.5,
        Alignment::Center => total_height * 0.5,
        Alignment::End => -parent_size.y * 0.5 + total_height,
    };
    
    // Position each child in the grid
    for (index, (child_id, _child_size)) in children.iter().enumerate() {
        let col = index % cols;
        let row = index / cols;
        
        // Get the child's pivot for proper positioning
        let child_pivot = if let Some(child) = elements.get(child_id) {
            *child.get_pivot()
        } else {
            Vector2::new(0.5, 0.5) // Default pivot
        };
        
        // Calculate cell boundaries
        let cell_left = grid_start_x + col as f32 * (max_width + gap.x);
        let cell_top = grid_start_y - row as f32 * (max_height + gap.y);
        
        // Position the child's pivot at the center of the cell
        // (This centers each child within its grid cell regardless of pivot)
        let cell_x = cell_left + max_width * 0.5;
        let cell_y = cell_top - max_height * 0.5;
        
        positions.push((*child_id, Vector2::new(cell_x, cell_y)));
    }
    
    positions
}

/// Modified version of update_global_transforms that integrates layout positioning
pub fn update_global_transforms_with_layout(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
    layout_positions: &HashMap<Uuid, Vector2>,
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

    // STEP 5: Calculate layout positions for this element's children BEFORE mutating
    let child_layout_positions = calculate_layout_positions(elements, current_id);
    let mut child_layout_map = HashMap::new();
    for (child_id, pos) in child_layout_positions {
        child_layout_map.insert(child_id, pos);
    }

    // Now borrow mutably - this is safe because we're done with immutable borrows
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

                _ => {}
            }
        }

        // Local transform
        let mut local = element.get_transform().clone();
        let local_z = element.get_z_index();
        let child_size = *element.get_size();
        let pivot = *element.get_pivot();

        // STEP 1: Layout positioning (if this element is in a layout)
        let mut layout_offset = Vector2::new(0.0, 0.0);
        if let Some(&layout_pos) = layout_positions.get(current_id) {
            layout_offset = layout_pos;
            // When in a layout, anchoring is disabled - the layout controls positioning
        } else {
            // STEP 2: Anchor positioning (only if NOT in a layout)
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
            layout_offset.x = anchor_x;
            layout_offset.y = anchor_y;
        }

        // STEP 3: Apply layout/anchor offset + user translation
        local.position.x += layout_offset.x;
        local.position.y += layout_offset.y;

        // STEP 4: Combine with parent transform
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

        // Get children list before dropping the mutable borrow
        let children_ids = element.get_children().to_vec();

        // STEP 6: Recurse into children with their layout positions
        for child_id in children_ids {
            update_global_transforms_with_layout(elements, &child_id, &global, &child_layout_map);
        }
    }
}

/// Updated layout function that uses the new layout system
pub fn update_ui_layout(ui_node: &mut Ui) {
    for root_id in &ui_node.root_ids {
        let empty_layout_map = HashMap::new();
        update_global_transforms_with_layout(
            &mut ui_node.elements, 
            root_id, 
            &Transform2D::default(),
            &empty_layout_map
        );
    }
}

pub fn render_ui(ui_node: &mut Ui, gfx: &mut Graphics) {
    update_ui_layout(ui_node); // now works with layout system
    for (_, element) in &ui_node.elements {
        if !element.get_visible() {
            continue;
        }
        match element {
            UIElement::BoxContainer(_) => { /* no-op */ },
            UIElement::Panel(panel) => render_panel(panel, gfx),
            UIElement::GridLayout(_) => { /* no-op */ },
            UIElement::Layout(_) => {},
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