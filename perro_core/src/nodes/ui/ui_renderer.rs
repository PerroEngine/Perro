use indexmap::IndexMap;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};
use uuid::Uuid;

use crate::{
    Graphics, RenderLayer,
    ast::FurAnchor,
    font::{Font, FontAtlas, Style, Weight},
    graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},
    structs2d::{Color, Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_elements::{
        ui_container::{ContainerMode, UIPanel},
        ui_text::UIText,
    },
    ui_node::UINode,
};

// Keep your existing LayoutSignature and LayoutCache structs...
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct LayoutSignature {
    size: (i32, i32),
    anchor: FurAnchor,
    children_count: usize,
    children_order: Vec<Uuid>,
    container_mode: Option<ContainerMode>,
    gap: Option<(i32, i32)>,
    cols: Option<usize>,
    style_affecting_layout: Vec<(String, i32)>,
}

impl LayoutSignature {
    fn from_element(element: &UIElement) -> Self {
        let size = element.get_size();
        let size_int = ((size.x * 1000.0) as i32, (size.y * 1000.0) as i32);

        let children_order = element.get_children().to_vec();

        let (container_mode, gap, cols) = match element {
            UIElement::Layout(layout) => (
                Some(layout.container.mode),
                Some((
                    (layout.container.gap.x * 1000.0) as i32,
                    (layout.container.gap.y * 1000.0) as i32,
                )),
                None,
            ),
            UIElement::GridLayout(grid) => (
                Some(grid.container.mode),
                Some((
                    (grid.container.gap.x * 1000.0) as i32,
                    (grid.container.gap.y * 1000.0) as i32,
                )),
                Some(grid.cols),
            ),
            UIElement::BoxContainer(_) => (None, None, None),
            _ => (None, None, None),
        };

        let mut style_affecting_layout = Vec::new();
        let style_map = element.get_style_map();
        for (key, value) in style_map {
            if key.contains("size.")
                || key.contains("transform.position.")
                || key.contains("transform.scale.")
            {
                style_affecting_layout.push((key.clone(), (*value * 1000.0) as i32));
            }
        }
        style_affecting_layout.sort();

        Self {
            size: size_int,
            anchor: *element.get_anchor(),
            children_count: children_order.len(),
            children_order,
            container_mode,
            gap,
            cols,
            style_affecting_layout,
        }
    }
}

#[derive(Debug)]
struct LayoutCacheEntry {
    signature: LayoutSignature,
    content_size: Vector2,
    layout_positions: Vec<(Uuid, Vector2)>,
    percentage_reference: Vector2,
}

#[derive(Debug, Default)]
pub struct LayoutCache {
    entries: HashMap<Uuid, LayoutCacheEntry>,
}

impl LayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn get_cached_content_size(&self, id: &Uuid, signature: &LayoutSignature) -> Option<Vector2> {
        self.entries
            .get(id)
            .filter(|entry| entry.signature == *signature)
            .map(|entry| entry.content_size)
    }

    fn get_cached_layout_positions(
        &self,
        id: &Uuid,
        signature: &LayoutSignature,
    ) -> Option<Vec<(Uuid, Vector2)>> {
        self.entries
            .get(id)
            .filter(|entry| entry.signature == *signature)
            .map(|entry| entry.layout_positions.clone())
    }

    fn get_cached_percentage_reference(
        &self,
        id: &Uuid,
        signature: &LayoutSignature,
    ) -> Option<Vector2> {
        self.entries
            .get(id)
            .filter(|entry| entry.signature == *signature)
            .map(|entry| entry.percentage_reference)
    }

    fn cache_results(
        &mut self,
        id: Uuid,
        signature: LayoutSignature,
        content_size: Vector2,
        layout_positions: Vec<(Uuid, Vector2)>,
        percentage_reference: Vector2,
    ) {
        self.entries.insert(
            id,
            LayoutCacheEntry {
                signature,
                content_size,
                layout_positions,
                percentage_reference,
            },
        );
    }
}

/// Helper function to find the first non-layout ancestor for percentage calculations
fn find_percentage_reference_ancestor(
    elements: &IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
) -> Option<Vector2> {
    let mut current = elements.get(current_id)?;

    // Walk up the parent chain
    while let Some(parent_id) = current.get_parent() {
        if let Some(parent) = elements.get(&parent_id) {
            // Check if parent is NOT a layout container
            match parent {
                UIElement::Layout(_) | UIElement::GridLayout(_) => {
                    // Skip layout containers, continue up the chain
                    current = parent;
                    continue;
                }
                _ => {
                    // Found a non-layout container, use its size
                    return Some(*parent.get_size());
                }
            }
        } else {
            break;
        }
    }

    // If we reach here, no non-layout parent found, use viewport
    Some(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT))
}

/// FIXED: Remove cache parameter to match working version
pub fn calculate_content_size(elements: &IndexMap<Uuid, UIElement>, parent_id: &Uuid) -> Vector2 {
    let parent = match elements.get(parent_id) {
        Some(p) => p,
        None => return Vector2::new(0.0, 0.0),
    };

    let children_ids = parent.get_children();
    if children_ids.is_empty() {
        return Vector2::new(0.0, 0.0);
    }

    // Convert to Vec for parallel processing
    let children_vec: Vec<&Uuid> = children_ids.iter().collect();
    let resolved_child_sizes: Vec<Vector2> = children_vec
        .par_iter()
        .filter_map(|&&child_id| {
            elements.get(&child_id).map(|child| {
                let mut child_size = *child.get_size();

                // Find the percentage reference for this child
                let percentage_reference_size =
                    find_percentage_reference_ancestor(elements, &child_id)
                        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

                // Resolve percentages using the smart reference
                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    child_size.x = percentage_reference_size.x * (pct / 100.0);
                }
                if let Some(&pct) = style_map.get("size.y") {
                    child_size.y = percentage_reference_size.y * (pct / 100.0);
                }

                child_size
            })
        })
        .collect();

    if resolved_child_sizes.is_empty() {
        return Vector2::new(0.0, 0.0);
    }

    // Handle different container types using resolved sizes
    match parent {
        UIElement::BoxContainer(_) => {
            // For BoxContainer, take the max of both dimensions using parallel processing
            let max_width = resolved_child_sizes
                .par_iter()
                .map(|size| size.x)
                .reduce(|| 0.0, f32::max);
            let max_height = resolved_child_sizes
                .par_iter()
                .map(|size| size.y)
                .reduce(|| 0.0, f32::max);

            Vector2::new(max_width, max_height)
        }
        UIElement::Layout(layout) => {
            let container_mode = &layout.container.mode;
            let gap = layout.container.gap;

            match container_mode {
                ContainerMode::Horizontal => {
                    // Width: sum of all children + gaps
                    let total_width: f32 = resolved_child_sizes.par_iter().map(|size| size.x).sum();
                    let gap_width = if resolved_child_sizes.len() > 1 {
                        gap.x * (resolved_child_sizes.len() - 1) as f32
                    } else {
                        0.0
                    };
                    // Height: max of all children
                    let max_height = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.y)
                        .reduce(|| 0.0, f32::max);
                    Vector2::new(total_width + gap_width, max_height)
                }
                ContainerMode::Vertical => {
                    // Width: max of all children
                    let max_width = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.x)
                        .reduce(|| 0.0, f32::max);
                    // Height: sum of all children + gaps
                    let total_height: f32 =
                        resolved_child_sizes.par_iter().map(|size| size.y).sum();
                    let gap_height = if resolved_child_sizes.len() > 1 {
                        gap.y * (resolved_child_sizes.len() - 1) as f32
                    } else {
                        0.0
                    };
                    Vector2::new(max_width, total_height + gap_height)
                }
                ContainerMode::Grid => {
                    // This shouldn't happen for Layout, but handle it anyway
                    let max_width = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.x)
                        .reduce(|| 0.0, f32::max);
                    let max_height = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.y)
                        .reduce(|| 0.0, f32::max);
                    Vector2::new(max_width, max_height)
                }
            }
        }
        UIElement::GridLayout(grid) => {
            let gap = grid.container.gap;
            let cols = grid.cols;

            if cols == 0 {
                return Vector2::new(0.0, 0.0);
            }

            if resolved_child_sizes.is_empty() {
                return Vector2::new(0.0, 0.0);
            }

            let rows = (resolved_child_sizes.len() + cols - 1) / cols; // Ceiling division

            // Find max dimensions for grid cells using parallel processing
            let max_cell_width = resolved_child_sizes
                .par_iter()
                .map(|size| size.x)
                .reduce(|| 0.0, f32::max);
            let max_cell_height = resolved_child_sizes
                .par_iter()
                .map(|size| size.y)
                .reduce(|| 0.0, f32::max);

            // Total width: (cols × max_width) + gaps between columns
            let total_width = max_cell_width * cols as f32
                + if cols > 1 {
                    gap.x * (cols - 1) as f32
                } else {
                    0.0
                };

            // Total height: (rows × max_height) + gaps between rows
            let total_height = max_cell_height * rows as f32
                + if rows > 1 {
                    gap.y * (rows - 1) as f32
                } else {
                    0.0
                };

            Vector2::new(total_width, total_height)
        }
        _ => Vector2::new(0.0, 0.0), // Not a container
    }
}

// Keep your cached version separate
pub fn calculate_content_size_smart_cached(
    elements: &IndexMap<Uuid, UIElement>,
    parent_id: &Uuid,
    cache: &RwLock<LayoutCache>,
) -> Vector2 {
    let parent = match elements.get(parent_id) {
        Some(p) => p,
        None => return Vector2::new(0.0, 0.0),
    };

    let signature = LayoutSignature::from_element(parent);

    // Check cache first with read lock
    if let Ok(cache_ref) = cache.read() {
        if let Some(cached) = cache_ref.get_cached_content_size(parent_id, &signature) {
            return cached;
        }
    }

    // Fall back to non-cached version
    let result = calculate_content_size(elements, parent_id);

    // Cache the result with write lock
    if let Ok(mut cache_ref) = cache.write() {
        cache_ref.cache_results(
            *parent_id,
            signature,
            result,
            Vec::new(),
            Vector2::new(0.0, 0.0),
        );
    }
    result
}

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

    // Get layout properties - BoxContainer doesn't do layout positioning
    let (container_mode, gap) = match parent {
        UIElement::Layout(layout) => (&layout.container.mode, layout.container.gap),
        UIElement::GridLayout(grid) => (&grid.container.mode, grid.container.gap),
        UIElement::BoxContainer(_) => {
            // BoxContainer doesn't position children - they use anchors/manual positioning
            return Vec::new();
        }
        _ => return Vec::new(), // Not a layout container
    };

    // Convert to Vec for parallel processing
    let children_vec: Vec<&Uuid> = children_ids.iter().collect();
    let child_info: Vec<(Uuid, Vector2)> = children_vec
        .par_iter()
        .filter_map(|&&child_id| {
            elements.get(&child_id).map(|child| {
                let mut child_size = *child.get_size();

                // Resolve percentages for layout positioning
                let percentage_reference_size =
                    find_percentage_reference_ancestor(elements, &child_id)
                        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    child_size.x = percentage_reference_size.x * (pct / 100.0);
                }
                if let Some(&pct) = style_map.get("size.y") {
                    child_size.y = percentage_reference_size.y * (pct / 100.0);
                }

                (child_id, child_size)
            })
        })
        .collect();

    if child_info.is_empty() {
        return Vec::new();
    }

    match container_mode {
        ContainerMode::Horizontal => calculate_horizontal_layout(&child_info, gap),
        ContainerMode::Vertical => calculate_vertical_layout(&child_info, gap),
        ContainerMode::Grid => {
            if let UIElement::GridLayout(grid) = parent {
                calculate_grid_layout(&child_info, gap, grid.cols)
            } else {
                Vec::new()
            }
        }
    }
}

// Keep your layout calculation functions exactly as they were in the working version
fn calculate_horizontal_layout(children: &[(Uuid, Vector2)], gap: Vector2) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    // Calculate total width needed using parallel processing
    let total_child_width: f32 = children.par_iter().map(|(_, size)| size.x).sum();
    let total_gap_width = if children.len() > 1 {
        gap.x * (children.len() - 1) as f32
    } else {
        0.0
    };
    let total_content_width = total_child_width + total_gap_width;

    // Start from the left edge of the content area (which is centered in the parent)
    let start_x = -total_content_width * 0.5;

    // Position each child from left to right
    let mut current_x = start_x;

    for (child_id, child_size) in children {
        // Position child at its left edge, then offset by half its width to center it
        let child_x = current_x + child_size.x * 0.5;
        let child_y = 0.0; // Center vertically in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Move to next position
        current_x += child_size.x + gap.x;
    }

    positions
}

fn calculate_vertical_layout(children: &[(Uuid, Vector2)], gap: Vector2) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    // Calculate total height needed using parallel processing
    let total_child_height: f32 = children.par_iter().map(|(_, size)| size.y).sum();
    let total_gap_height = if children.len() > 1 {
        gap.y * (children.len() - 1) as f32
    } else {
        0.0
    };
    let total_content_height = total_child_height + total_gap_height;

    // Start from the top edge of the content area (which is centered in the parent)
    let start_y = total_content_height * 0.5;

    // Position each child from top to bottom
    let mut current_y = start_y;

    for (child_id, child_size) in children {
        // Position child at its top edge, then offset by half its height to center it
        let child_y = current_y - child_size.y * 0.5;
        let child_x = 0.0; // Center horizontally in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Move to next position (downward)
        current_y -= child_size.y + gap.y;
    }

    positions
}

fn calculate_grid_layout(
    children: &[(Uuid, Vector2)],
    gap: Vector2,
    cols: usize,
) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    if cols == 0 {
        return positions;
    }

    let rows = (children.len() + cols - 1) / cols; // Ceiling division

    // Find the maximum width and height for consistent grid cells using parallel processing
    let max_width = children
        .par_iter()
        .map(|(_, size)| size.x)
        .reduce(|| 0.0, f32::max);
    let max_height = children
        .par_iter()
        .map(|(_, size)| size.y)
        .reduce(|| 0.0, f32::max);

    // Calculate total grid dimensions
    let total_width = max_width * cols as f32
        + if cols > 1 {
            gap.x * (cols - 1) as f32
        } else {
            0.0
        };
    let total_height = max_height * rows as f32
        + if rows > 1 {
            gap.y * (rows - 1) as f32
        } else {
            0.0
        };

    // Start from top-left of the grid (which is centered in the parent)
    let grid_start_x = -total_width * 0.5;
    let grid_start_y = total_height * 0.5;

    // Position each child in the grid
    for (index, (child_id, _child_size)) in children.iter().enumerate() {
        let col = index % cols;
        let row = index / cols;

        // Calculate cell position
        let cell_x = grid_start_x + col as f32 * (max_width + gap.x) + max_width * 0.5;
        let cell_y = grid_start_y - row as f32 * (max_height + gap.y) - max_height * 0.5;

        positions.push((*child_id, Vector2::new(cell_x, cell_y)));
    }

    positions
}

/// Recursively calculate content sizes for all containers, starting from leaves
fn calculate_all_content_sizes(elements: &mut IndexMap<Uuid, UIElement>, current_id: &Uuid) {
    // First, process all children recursively
    let children_ids = if let Some(element) = elements.get(current_id) {
        element.get_children().to_vec()
    } else {
        return;
    };

    for child_id in children_ids {
        calculate_all_content_sizes(elements, &child_id);
    }

    // Then calculate this element's size based on its (now correctly sized) children
    if let Some(element) = elements.get(current_id) {
        let is_container = matches!(
            element,
            UIElement::Layout(_) | UIElement::GridLayout(_) | UIElement::BoxContainer(_)
        );

        if is_container {
            let content_size = calculate_content_size(elements, current_id);
            if let Some(element) = elements.get_mut(current_id) {
                element.set_size(content_size);
            }
        }
    }
}

fn calculate_all_content_sizes_cached(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    cache: &RwLock<LayoutCache>,
) {
    let children_ids = if let Some(element) = elements.get(current_id) {
        element.get_children().to_vec()
    } else {
        return;
    };

    for child_id in children_ids {
        calculate_all_content_sizes_cached(elements, &child_id, cache);
    }

    if let Some(element) = elements.get(current_id) {
        let is_container = matches!(
            element,
            UIElement::Layout(_) | UIElement::GridLayout(_) | UIElement::BoxContainer(_)
        );

        if is_container {
            let content_size = calculate_content_size_smart_cached(elements, current_id, cache);
            if let Some(element) = elements.get_mut(current_id) {
                element.set_size(content_size);
            }
        }
    }
}

/// FIXED: Remove cache parameter to match working version
pub fn update_global_transforms_with_layout(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
    layout_positions: &HashMap<Uuid, Vector2>,
    parent_z: i32,
) {
    // Get parent info - FIXED: Use the working version's logic
    let (parent_size, parent_z) = {
        let parent_id = elements.get(current_id).and_then(|el| el.get_parent());

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

    // Find the reference size for percentages (first non-layout ancestor)
    let percentage_reference_size = find_percentage_reference_ancestor(elements, current_id)
        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

    // Calculate layout positions for this element's children BEFORE mutating
    let child_layout_positions = calculate_layout_positions(elements, current_id);
    let mut child_layout_map = HashMap::new();
    for (child_id, pos) in child_layout_positions {
        child_layout_map.insert(child_id, pos);
    }

    // Now borrow mutably - this is safe because we're done with immutable borrows
    if let Some(element) = elements.get_mut(current_id) {
        let style_map = element.get_style_map().clone(); // clone to break the borrow

        // Check if this is a layout container that should be auto-sized
        let is_layout_container =
            matches!(element, UIElement::Layout(_) | UIElement::GridLayout(_));
        let has_explicit_size =
            style_map.contains_key("size.x") || style_map.contains_key("size.y");

        // Apply percentage styles first (but skip auto-sizing containers unless they have explicit percentages)
        if !is_layout_container || has_explicit_size {
            for (key, pct) in style_map.iter() {
                let fraction = *pct / 100.0;

                match key.as_str() {
                    // Size percentages now use the first non-layout ancestor
                    "size.x" => {
                        element.set_size(Vector2::new(
                            percentage_reference_size.x * fraction,
                            element.get_size().y,
                        ));
                    }
                    "size.y" => {
                        element.set_size(Vector2::new(
                            element.get_size().x,
                            percentage_reference_size.y * fraction,
                        ));
                    }

                    // Position percentages still use immediate parent
                    "transform.position.x" => {
                        element.get_transform_mut().position.x = parent_size.x * fraction;
                    }
                    "transform.position.y" => {
                        element.get_transform_mut().position.y = parent_size.y * fraction;
                    }

                    // Scale percentages use parent scale
                    "transform.scale.x" => {
                        let parent_scale_x = parent_global.scale.x;
                        element.get_transform_mut().scale.x = 1.0 * fraction * parent_scale_x;
                    }
                    "transform.scale.y" => {
                        let parent_scale_y = parent_global.scale.y;
                        element.get_transform_mut().scale.y = 1.0 * fraction * parent_scale_y;
                    }

                    _ => {}
                }
            }
        } else {
            // For layout containers without explicit size percentages, apply position/scale percentages only
            for (key, pct) in style_map.iter() {
                let fraction = *pct / 100.0;

                match key.as_str() {
                    "transform.position.x" => {
                        element.get_transform_mut().position.x = parent_size.x * fraction;
                    }
                    "transform.position.y" => {
                        element.get_transform_mut().position.y = parent_size.y * fraction;
                    }
                    "transform.scale.x" => {
                        let parent_scale_x = parent_global.scale.x;
                        element.get_transform_mut().scale.x = 1.0 * fraction * parent_scale_x;
                    }
                    "transform.scale.y" => {
                        let parent_scale_y = parent_global.scale.y;
                        element.get_transform_mut().scale.y = 1.0 * fraction * parent_scale_y;
                    }
                    _ => {}
                }
            }
        }

        // NOW apply auto-sizing for layout containers (this will override any existing size)
        if is_layout_container && !has_explicit_size {
            let content_size = calculate_content_size(elements, current_id);
            if let Some(element) = elements.get_mut(current_id) {
                element.set_size(content_size);
            }
        }

        // Re-borrow for the rest of the function
        if let Some(element) = elements.get_mut(current_id) {
            // Local transform
            let mut local = element.get_transform().clone();
            let child_size = *element.get_size();
            let pivot = *element.get_pivot();

            // STEP 1: Layout positioning (if this element is in a layout)
            let mut layout_offset = Vector2::new(0.0, 0.0);
            if let Some(&layout_pos) = layout_positions.get(current_id) {
                layout_offset = layout_pos;
            } else {
                // STEP 2: Anchor positioning (only if NOT in a layout)
                // For anchoring, we want to use the immediate parent's size for positioning
                let anchor_reference_size = parent_size;

                let (anchor_x, anchor_y) = match element.get_anchor() {
                    // Corners - need to position the element so its corner aligns with parent corner
                    FurAnchor::TopLeft => {
                        // Parent's top-left corner
                        let parent_left = -anchor_reference_size.x * 0.5;
                        let parent_top = anchor_reference_size.y * 0.5;

                        // Position element so its top-left corner is at parent's top-left
                        let offset_x = parent_left + child_size.x * (1.0 - pivot.x);
                        let offset_y = parent_top - child_size.y * pivot.y;
                        (offset_x, offset_y)
                    }
                    FurAnchor::TopRight => {
                        let parent_right = anchor_reference_size.x * 0.5;
                        let parent_top = anchor_reference_size.y * 0.5;

                        let offset_x = parent_right - child_size.x * pivot.x;
                        let offset_y = parent_top - child_size.y * pivot.y;
                        (offset_x, offset_y)
                    }
                    FurAnchor::BottomLeft => {
                        let parent_left = -anchor_reference_size.x * 0.5;
                        let parent_bottom = -anchor_reference_size.y * 0.5;

                        let offset_x = parent_left + child_size.x * (1.0 - pivot.x);
                        let offset_y = parent_bottom + child_size.y * (1.0 - pivot.y);
                        (offset_x, offset_y)
                    }
                    FurAnchor::BottomRight => {
                        let parent_right = anchor_reference_size.x * 0.5;
                        let parent_bottom = -anchor_reference_size.y * 0.5;

                        let offset_x = parent_right - child_size.x * pivot.x;
                        let offset_y = parent_bottom + child_size.y * (1.0 - pivot.y);
                        (offset_x, offset_y)
                    }

                    // Edges - align the appropriate edge
                    FurAnchor::Top => {
                        let parent_top = anchor_reference_size.y * 0.5;
                        let offset_y = parent_top - child_size.y * pivot.y;
                        (0.0, offset_y) // Center horizontally
                    }
                    FurAnchor::Bottom => {
                        let parent_bottom = -anchor_reference_size.y * 0.5;
                        let offset_y = parent_bottom + child_size.y * (1.0 - pivot.y);
                        (0.0, offset_y) // Center horizontally
                    }
                    FurAnchor::Left => {
                        let parent_left = -anchor_reference_size.x * 0.5;
                        let offset_x = parent_left + child_size.x * (1.0 - pivot.x);
                        (offset_x, 0.0) // Center vertically
                    }
                    FurAnchor::Right => {
                        let parent_right = anchor_reference_size.x * 0.5;
                        let offset_x = parent_right - child_size.x * pivot.x;
                        (offset_x, 0.0) // Center vertically
                    }

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
            global.position.x =
                parent_global.position.x + (local.position.x * parent_global.scale.x);
            global.position.y =
                parent_global.position.y + (local.position.y * parent_global.scale.y);
            global.rotation = parent_global.rotation + local.rotation;

            element.set_global_transform(global.clone());

            // Set inherited z-index: local z + parent z
            let local_z = element.get_z_index(); // Get the element's explicitly set z-index
            let global_z = if local_z != 0 {
                // If element has explicit z-index, use it (but still inherit parent offset)
                parent_z + local_z + 2
            } else {
                // Otherwise use automatic depth-based z-index
                parent_z + 2
            };
            element.set_z_index(global_z);

            // Get children list before dropping the mutable borrow
            let children_ids = element.get_children().to_vec();

            // STEP 6: Recurse into children with their layout positions
            for child_id in children_ids {
                update_global_transforms_with_layout(
                    elements,
                    &child_id,
                    &global,
                    &child_layout_map,
                    global_z,
                );
            }
        }
    }
}

/// Updated layout function that uses the new layout system
pub fn update_ui_layout(ui_node: &mut UINode) {
    if let (Some(root_ids), Some(elements)) = (&ui_node.root_ids, &mut ui_node.elements) {
        for root_id in root_ids {
            calculate_all_content_sizes(elements, root_id);
        }

        let empty_layout_map = HashMap::new();
        for root_id in root_ids {
            update_global_transforms_with_layout(
                elements,
                root_id,
                &Transform2D::default(),
                &empty_layout_map,
                0,
            );
        }
    }
}

fn update_ui_layout_cached(ui_node: &mut UINode, cache: &RwLock<LayoutCache>) {
    if let (Some(root_ids), Some(elements)) = (&ui_node.root_ids, &mut ui_node.elements) {
        for root_id in root_ids {
            calculate_all_content_sizes_cached(elements, root_id, cache);
        }

        let empty_layout_map = HashMap::new();
        for root_id in root_ids {
            update_global_transforms_with_layout(
                elements,
                root_id,
                &Transform2D::default(),
                &empty_layout_map,
                0,
            );
        }
    }
}

// Updated cache to use RwLock instead of Mutex for better read performance
static LAYOUT_CACHE: OnceLock<RwLock<LayoutCache>> = OnceLock::new();
static FONT_ATLAS_INITIALIZED: OnceLock<RwLock<HashMap<(String, u32), bool>>> = OnceLock::new();

pub fn get_layout_cache() -> &'static RwLock<LayoutCache> {
    LAYOUT_CACHE.get_or_init(|| RwLock::new(LayoutCache::new()))
}

fn get_font_cache() -> &'static RwLock<HashMap<(String, u32), bool>> {
    FONT_ATLAS_INITIALIZED.get_or_init(|| RwLock::new(HashMap::new()))
}

// Updated render function with caching
pub fn render_ui(ui_node: &mut UINode, gfx: &mut Graphics) {
    let cache = get_layout_cache();
    update_ui_layout_cached(ui_node, cache);

    if let Some(elements) = &ui_node.elements {
        // Convert IndexMap values to Vec for parallel processing
        let elements_vec: Vec<_> = elements.iter().collect();
        let visible_elements: Vec<_> = elements_vec
            .par_iter()
            .filter(|(_, element)| element.get_visible())
            .collect();

        for (_, element) in visible_elements {
            match element {
                UIElement::BoxContainer(_) => { /* no-op */ }
                UIElement::Panel(panel) => render_panel(panel, gfx),
                UIElement::GridLayout(_) => { /* no-op */ }
                UIElement::Layout(_) => {}
                UIElement::Text(text) => render_text(text, gfx),
            }
        }
    }
}

fn render_panel(panel: &UIPanel, gfx: &mut Graphics) {
    let background_color = panel
        .props
        .background_color
        .clone()
        .unwrap_or(Color::new(0, 0, 0, 0));
    let corner_radius = panel.props.corner_radius;
    let border_color = panel.props.border_color.clone();
    let border_thickness = panel.props.border_thickness;
    let z_index = panel.z_index;
    let bg_id = panel.id;

    println!("{}", z_index);

    gfx.renderer_ui.queue_panel(
        &mut gfx.renderer_prim,
        bg_id,
        panel.base.global_transform,
        panel.base.size,
        panel.base.pivot,
        background_color,
        Some(corner_radius),
        0.0,
        false,
        z_index,
    );

    if border_thickness > 0.0 {
        if let Some(border_color) = border_color {
            let border_id = Uuid::new_v5(&bg_id, b"border");
            gfx.renderer_ui.queue_panel(
                &mut gfx.renderer_prim,
                border_id,
                panel.base.global_transform,
                panel.base.size,
                panel.base.pivot,
                border_color,
                Some(corner_radius),
                border_thickness,
                true,
                z_index + 1,
            );
        }
    }
}

// Optimized text rendering - only regenerate atlas when font properties change
fn render_text(text: &UIText, gfx: &mut Graphics) {
    let font_key = ("NotoSans".to_string(), 64);
    let font_cache = get_font_cache();

    // Check if font atlas is already initialized
    if let Ok(cache) = font_cache.read() {
        if !cache.contains_key(&font_key) {
            drop(cache);

            // Initialize font atlas
            if let Ok(mut cache) = font_cache.write() {
                if !cache.contains_key(&font_key) {
                    if let Some(font) = Font::from_name("NotoSans", Weight::Regular, Style::Normal)
                    {
                        let font_atlas = FontAtlas::new(font, 64.0);
                        gfx.initialize_font_atlas(font_atlas);
                        cache.insert(font_key, true);
                    }
                }
            }
        }
    }

    gfx.renderer_ui.queue_text(
        &mut gfx.renderer_prim,
        text.id,
        &text.props.content,
        text.props.font_size,
        text.base.global_transform,
        text.base.pivot,
        text.props.color,
        text.base.z_index,
    );
}
