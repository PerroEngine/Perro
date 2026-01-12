use indexmap::IndexMap;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    sync::{OnceLock, RwLock},
};
use uuid::Uuid;

use crate::{
    Graphics,
    font::{Font, FontAtlas, Style, Weight},
    fur_ast::FurAnchor,
    graphics::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH},
    structs::Color,
    structs2d::{Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_elements::{
        ui_container::{ContainerMode, LayoutAlignment, UIPanel},
        ui_text::UIText,
        ui_text_input::UITextInput,
        ui_text_edit::UITextEdit,
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
    padding: Option<(i32, i32, i32, i32)>, // top, right, bottom, left
    align: Option<u8>, // LayoutAlignment as u8
    cols: Option<usize>,
    style_affecting_layout: Vec<(String, i32)>,
}

impl LayoutSignature {
    fn from_element(element: &UIElement) -> Self {
        let size = element.get_size();
        let size_int = ((size.x * 1000.0) as i32, (size.y * 1000.0) as i32);

        let children_order = element.get_children().to_vec();

        let (container_mode, gap, padding, align, cols) = match element {
            UIElement::Layout(layout) => (
                Some(layout.container.mode),
                Some((
                    (layout.container.gap.x * 1000.0) as i32,
                    (layout.container.gap.y * 1000.0) as i32,
                )),
                Some((
                    (layout.container.padding.top * 1000.0) as i32,
                    (layout.container.padding.right * 1000.0) as i32,
                    (layout.container.padding.bottom * 1000.0) as i32,
                    (layout.container.padding.left * 1000.0) as i32,
                )),
                Some(layout.container.align as u8),
                None,
            ),
            UIElement::GridLayout(grid) => (
                Some(grid.container.mode),
                Some((
                    (grid.container.gap.x * 1000.0) as i32,
                    (grid.container.gap.y * 1000.0) as i32,
                )),
                Some((
                    (grid.container.padding.top * 1000.0) as i32,
                    (grid.container.padding.right * 1000.0) as i32,
                    (grid.container.padding.bottom * 1000.0) as i32,
                    (grid.container.padding.left * 1000.0) as i32,
                )),
                Some(grid.container.align as u8),
                Some(grid.cols),
            ),
            UIElement::BoxContainer(_) => (None, None, None, None, None),
            _ => (None, None, None, None, None),
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
            padding,
            align,
            cols,
            style_affecting_layout,
        }
    }
}

#[derive(Debug)]
struct LayoutCacheEntry {
    signature: LayoutSignature,
    content_size: Vector2,
    #[allow(dead_code)]
    layout_positions: Vec<(Uuid, Vector2)>,
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

/// Helper function to find the parent element for percentage calculations
/// Uses layout containers with explicit sizes, but skips auto-sizing layout containers
fn find_percentage_reference_ancestor(
    elements: &IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
) -> Option<Vector2> {
    let current = elements.get(current_id)?;

    // Check immediate parent first - if it's a layout container, use it
    let parent_id = current.get_parent();
    if !parent_id.is_nil() {
        if let Some(parent) = elements.get(&parent_id) {
            // Check if immediate parent is a layout container
            if matches!(parent, UIElement::Layout(_) | UIElement::GridLayout(_)) {
                let parent_size = *parent.get_size();
                // Use the layout container if it has any non-zero size
                // This ensures children inside layouts use the layout's size as reference
                if parent_size.x > 0.0 || parent_size.y > 0.0 {
                    return Some(parent_size);
                }
            }
        }
    }

    // If immediate parent is not a layout or has zero size, walk up the chain
    let mut current = current;
    let mut parent_id = current.get_parent();
    while !parent_id.is_nil() {
        if let Some(parent) = elements.get(&parent_id) {
            // Check if parent is a layout container
            match parent {
                UIElement::Layout(_) | UIElement::GridLayout(_) => {
                    // Check if this layout container has explicit size
                    let style_map = parent.get_style_map();
                    let has_explicit_size = style_map.contains_key("size.x") || style_map.contains_key("size.y");
                    
                    if has_explicit_size {
                        // Layout container has explicit size, use it as reference
                        // But only if the size has been computed (non-zero)
                        let parent_size = *parent.get_size();
                        if parent_size.x > 0.0 || parent_size.y > 0.0 {
                            return Some(parent_size);
                        }
                        // If size is zero, it might not be computed yet, so continue up the chain
                    }
                    // Layout container is auto-sizing or size not computed yet, skip it
                    current = parent;
                    parent_id = current.get_parent();
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

    // If we reach here, no suitable parent found, use viewport
    Some(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT))
}

/// Helper function to check if an element is effectively visible (considering parent visibility)
/// Walks up the parent chain to ensure all ancestors are visible
fn is_effectively_visible(elements: &IndexMap<Uuid, UIElement>, element_id: Uuid) -> bool {
    let mut current_id = element_id;
    let mut visited = std::collections::HashSet::new();
    const MAX_DEPTH: usize = 100; // Prevent infinite loops
    let mut depth = 0;
    
    loop {
        if depth > MAX_DEPTH {
            // Safety: prevent infinite loops if parent chain is broken
            eprintln!("⚠️ is_effectively_visible: Max depth reached for element {}", element_id);
            return false;
        }
        depth += 1;
        
        // Prevent infinite loops
        if visited.contains(&current_id) {
            eprintln!("⚠️ is_effectively_visible: Circular parent chain detected for element {}", element_id);
            return false;
        }
        visited.insert(current_id);
        
        if let Some(element) = elements.get(&current_id) {
            // If this element is not visible, return false
            if !element.get_visible() {
                return false;
            }
            // Check parent
            let parent_id = element.get_parent();
            if parent_id.is_nil() {
                // Reached root, element is visible
                return true;
            }
            current_id = parent_id;
        } else {
            // Element not found, consider it invisible
            eprintln!("⚠️ is_effectively_visible: Element {} not found in elements map", current_id);
            return false;
        }
    }
}

/// Calculate content size using pre-computed caches (FAST - no parent chain walks!)
fn calculate_content_size_with_visibility_cache(
    elements: &IndexMap<Uuid, UIElement>,
    parent_id: &Uuid,
    visibility_cache: &HashSet<Uuid>,
    percentage_ref_cache: &HashMap<Uuid, Vector2>,
) -> Vector2 {
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
            // Use cached visibility instead of calling is_effectively_visible!
            if !visibility_cache.contains(&child_id) {
                return None; // Child is invisible
            }
            
            elements.get(&child_id).map(|child| {
                let mut child_size = *child.get_size();

                // Use cached percentage reference (no parent chain walk!)
                let percentage_reference_size = percentage_ref_cache
                    .get(&child_id)
                    .copied()
                    .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

                // Resolve percentages
                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    if pct >= 0.0 {
                        child_size.x = (percentage_reference_size.x as f64 * (pct as f64 / 100.0)) as f32;
                    }
                }
                if let Some(&pct) = style_map.get("size.y") {
                    if pct >= 0.0 {
                        child_size.y = (percentage_reference_size.y as f64 * (pct as f64 / 100.0)) as f32;
                    }
                }

                child_size
            })
        })
        .collect();

    // If all children are invisible, return 0 size
    if resolved_child_sizes.is_empty() {
        return Vector2::new(0.0, 0.0);
    }

    // Handle different container types
    match parent {
        UIElement::BoxContainer(_) => {
            let max_width = resolved_child_sizes.par_iter().map(|size| size.x).reduce(|| 0.0, f32::max);
            let max_height = resolved_child_sizes.par_iter().map(|size| size.y).reduce(|| 0.0, f32::max);
            Vector2::new(max_width, max_height)
        }
        UIElement::Layout(layout) => {
            let gap = layout.container.gap;
            let padding = layout.container.padding;

            let content_size = match layout.container.mode {
                ContainerMode::Horizontal => {
                    let total_width: f64 = resolved_child_sizes.par_iter().map(|size| size.x as f64).sum();
                    let gap_width = if resolved_child_sizes.len() > 1 {
                        gap.x as f64 * (resolved_child_sizes.len() - 1) as f64
                    } else {
                        0.0
                    };
                    let max_height = resolved_child_sizes.par_iter().map(|size| size.y).reduce(|| 0.0, f32::max);
                    Vector2::new((total_width + gap_width) as f32, max_height)
                }
                ContainerMode::Vertical => {
                    let max_width = resolved_child_sizes.par_iter().map(|size| size.x).reduce(|| 0.0, f32::max);
                    let total_height: f64 = resolved_child_sizes.par_iter().map(|size| size.y as f64).sum();
                    let gap_height = if resolved_child_sizes.len() > 1 {
                        gap.y as f64 * (resolved_child_sizes.len() - 1) as f64
                    } else {
                        0.0
                    };
                    Vector2::new(max_width, (total_height + gap_height) as f32)
                }
                ContainerMode::Grid => {
                    let max_width = resolved_child_sizes.par_iter().map(|size| size.x).reduce(|| 0.0, f32::max);
                    let max_height = resolved_child_sizes.par_iter().map(|size| size.y).reduce(|| 0.0, f32::max);
                    Vector2::new(max_width, max_height)
                }
            };
            
            Vector2::new(
                content_size.x + padding.horizontal(),
                content_size.y + padding.vertical(),
            )
        }
        UIElement::GridLayout(grid) => {
            let gap = grid.container.gap;
            let padding = grid.container.padding;
            let cols = grid.cols;

            if cols == 0 || resolved_child_sizes.is_empty() {
                return Vector2::new(0.0, 0.0);
            }

            let rows = (resolved_child_sizes.len() + cols - 1) / cols;
            let max_cell_width = resolved_child_sizes.par_iter().map(|size| size.x as f64).reduce(|| 0.0, f64::max);
            let max_cell_height = resolved_child_sizes.par_iter().map(|size| size.y as f64).reduce(|| 0.0, f64::max);

            let total_width = max_cell_width * cols as f64 + if cols > 1 { gap.x as f64 * (cols - 1) as f64 } else { 0.0 };
            let total_height = max_cell_height * rows as f64 + if rows > 1 { gap.y as f64 * (rows - 1) as f64 } else { 0.0 };

            Vector2::new(
                (total_width + padding.horizontal() as f64) as f32,
                (total_height + padding.vertical() as f64) as f32,
            )
        }
        _ => Vector2::new(0.0, 0.0),
    }
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
                // Check effective visibility (including parent visibility)
                if !is_effectively_visible(elements, child_id) {
                    return Vector2::new(0.0, 0.0);
                }
                
                let mut child_size = *child.get_size();

                // Find the percentage reference for this child
                let percentage_reference_size =
                    find_percentage_reference_ancestor(elements, &child_id)
                        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

                // Resolve percentages using the smart reference (skip auto-sizing)
                // Use f64 for precision to avoid rounding errors
                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    if pct >= 0.0 {
                        // Not auto-sizing, resolve percentage
                        child_size.x = (percentage_reference_size.x as f64 * (pct as f64 / 100.0)) as f32;
                    }
                    // If pct < 0.0, it's auto-sizing - keep default size for now
                }
                if let Some(&pct) = style_map.get("size.y") {
                    if pct >= 0.0 {
                        // Not auto-sizing, resolve percentage
                        child_size.y = (percentage_reference_size.y as f64 * (pct as f64 / 100.0)) as f32;
                    }
                    // If pct < 0.0, it's auto-sizing - keep default size for now
                }

                child_size
            })
        })
        .collect();

    // If all children are invisible, return 0 size (layout should collapse)
    if resolved_child_sizes.is_empty() || resolved_child_sizes.iter().all(|s| s.x == 0.0 && s.y == 0.0) {
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
            let padding = layout.container.padding;

            let content_size = match container_mode {
                ContainerMode::Horizontal => {
                    // Width: sum of all children + gaps (use actual gap value, not default)
                    let total_width: f64 = resolved_child_sizes.par_iter().map(|size| size.x as f64).sum();
                    let gap_width = if resolved_child_sizes.len() > 1 {
                        gap.x as f64 * (resolved_child_sizes.len() - 1) as f64
                    } else {
                        0.0
                    };
                    // Height: max of all children
                    let max_height = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.y)
                        .reduce(|| 0.0, f32::max);
                    Vector2::new((total_width + gap_width) as f32, max_height)
                }
                ContainerMode::Vertical => {
                    // Width: max of all children
                    let max_width = resolved_child_sizes
                        .par_iter()
                        .map(|size| size.x)
                        .reduce(|| 0.0, f32::max);
                    // Height: sum of all children + gaps (use actual gap value, not default)
                    let total_height: f64 =
                        resolved_child_sizes.par_iter().map(|size| size.y as f64).sum();
                    let gap_height = if resolved_child_sizes.len() > 1 {
                        gap.y as f64 * (resolved_child_sizes.len() - 1) as f64
                    } else {
                        0.0
                    };
                    Vector2::new(max_width, (total_height + gap_height) as f32)
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
            };
            
            // Add padding to content size
            Vector2::new(
                content_size.x + padding.horizontal(),
                content_size.y + padding.vertical(),
            )
        }
        UIElement::GridLayout(grid) => {
            let gap = grid.container.gap;
            let padding = grid.container.padding;
            let cols = grid.cols;

            if cols == 0 {
                return Vector2::new(0.0, 0.0);
            }

            if resolved_child_sizes.is_empty() {
                return Vector2::new(0.0, 0.0);
            }

            let rows = (resolved_child_sizes.len() + cols - 1) / cols; // Ceiling division

            // Find max dimensions for grid cells using parallel processing
            // Use f64 for precision during calculations
            let max_cell_width = resolved_child_sizes
                .par_iter()
                .map(|size| size.x as f64)
                .reduce(|| 0.0, f64::max);
            let max_cell_height = resolved_child_sizes
                .par_iter()
                .map(|size| size.y as f64)
                .reduce(|| 0.0, f64::max);

            // Total width: (cols × max_width) + gaps between columns
            let total_width = max_cell_width * cols as f64
                + if cols > 1 {
                    gap.x as f64 * (cols - 1) as f64
                } else {
                    0.0
                };

            // Total height: (rows × max_height) + gaps between rows
            let total_height = max_cell_height * rows as f64
                + if rows > 1 {
                    gap.y as f64 * (rows - 1) as f64
                } else {
                    0.0
                };

            // Add padding to content size
            Vector2::new(
                (total_width + padding.horizontal() as f64) as f32,
                (total_height + padding.vertical() as f64) as f32,
            )
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
    elements: &mut IndexMap<Uuid, UIElement>,
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

    // Get layout properties and parent info before we start mutating
    // First, resolve parent's percentage size if it has one
    // For layout containers, if they have percentage sizes, use the immediate parent's resolved size
    let mut resolved_parent_size = *parent.get_size();
    let parent_style_map = parent.get_style_map();
    
    // Resolve parent's percentage sizes
    // For layout containers with percentage sizes, use immediate parent's actual size (not percentage reference ancestor)
    if let Some(&pct_x) = parent_style_map.get("size.x") {
        if pct_x >= 0.0 {  // Not auto-sizing
            // For layout containers, use immediate parent's resolved size
            let parent_parent_id = parent.get_parent();
            if !parent_parent_id.is_nil() {
                if let Some(parent_parent) = elements.get(&parent_parent_id) {
                    resolved_parent_size.x = (parent_parent.get_size().x as f64 * (pct_x as f64 / 100.0)) as f32;
                } else {
                    let percentage_reference_size =
                        find_percentage_reference_ancestor(elements, parent_id)
                            .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
                    resolved_parent_size.x = (percentage_reference_size.x as f64 * (pct_x as f64 / 100.0)) as f32;
                }
            } else {
                let percentage_reference_size =
                    find_percentage_reference_ancestor(elements, parent_id)
                        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
                resolved_parent_size.x = (percentage_reference_size.x as f64 * (pct_x as f64 / 100.0)) as f32;
            }
        }
    }
    if let Some(&pct_y) = parent_style_map.get("size.y") {
        if pct_y >= 0.0 {  // Not auto-sizing
            // For layout containers, use immediate parent's resolved size
            let parent_parent_id = parent.get_parent();
            if !parent_parent_id.is_nil() {
                if let Some(parent_parent) = elements.get(&parent_parent_id) {
                    resolved_parent_size.y = (parent_parent.get_size().y as f64 * (pct_y as f64 / 100.0)) as f32;
                } else {
                    let percentage_reference_size =
                        find_percentage_reference_ancestor(elements, parent_id)
                            .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
                    resolved_parent_size.y = (percentage_reference_size.y as f64 * (pct_y as f64 / 100.0)) as f32;
                }
            } else {
                let percentage_reference_size =
                    find_percentage_reference_ancestor(elements, parent_id)
                        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
                resolved_parent_size.y = (percentage_reference_size.y as f64 * (pct_y as f64 / 100.0)) as f32;
            }
        }
    }
    
    let (container_mode, extra_gap, padding, layout_align) = match parent {
        UIElement::Layout(layout) => (
            layout.container.mode,  // Copy the enum
            layout.container.gap,   // Copy the Vector2
            layout.container.padding, // Copy the Padding
            layout.container.align, // Copy the LayoutAlignment
        ),
        UIElement::GridLayout(grid) => (
            grid.container.mode,  // Copy the enum
            grid.container.gap,   // Copy the Vector2
            grid.container.padding, // Copy the Padding
            grid.container.align, // Copy the LayoutAlignment
        ),
        UIElement::BoxContainer(_) => {
            // BoxContainer doesn't position children - they use anchors/manual positioning
            return Vec::new();
        }
        _ => return Vec::new(), // Not a layout container
    };
    
    // Reduce effective parent size by padding for child layout calculations
    let effective_parent_size = Vector2::new(
        (resolved_parent_size.x - padding.horizontal()).max(0.0),
        (resolved_parent_size.y - padding.vertical()).max(0.0),
    );
    
    let parent_size = resolved_parent_size;
    let use_content_based = effective_parent_size.x < 1.0 && effective_parent_size.y < 1.0;
    
    // Convert to Vec for processing
    let children_vec: Vec<&Uuid> = children_ids.iter().collect();
    
    // First pass: resolve all non-auto sizes and identify auto-sized children
    let mut child_info: Vec<(Uuid, Vector2, bool, bool)> = children_vec
        .iter()
        .filter_map(|&&child_id| {
            elements.get(&child_id).map(|child| {
                // Check effective visibility (including parent visibility)
                let is_visible = is_effectively_visible(elements, child_id);
                let mut child_size = *child.get_size();
                let style_map = child.get_style_map();
                
                // If element is invisible (or parent is invisible), treat it as having 0 size for layout calculations
                if !is_visible {
                    return (child_id, Vector2::new(0.0, 0.0), false, false);
                }
                
                // Check for auto-sizing: explicit "auto" (< 0.0) OR no size specified
                // Absolute values are stored in style_map with a marker (> 10000)
                const ABSOLUTE_MARKER: f32 = 10000.0;
                let mut auto_x = style_map.get("size.x").map(|&v| v < 0.0).unwrap_or(true); // No size = auto
                let mut auto_y = style_map.get("size.y").map(|&v| v < 0.0).unwrap_or(true); // No size = auto
                
                // Check if this is an explicit absolute size (marked with > 10000)
                let is_abs_x = style_map.get("size.x").map(|&v| v >= ABSOLUTE_MARKER).unwrap_or(false);
                let is_abs_y = style_map.get("size.y").map(|&v| v >= ABSOLUTE_MARKER).unwrap_or(false);
                
                // If it's an explicit absolute size, it's not auto
                if is_abs_x {
                    auto_x = false;
                    // Extract the actual absolute value (remove the marker)
                    if let Some(&marked_value) = style_map.get("size.x") {
                        child_size.x = marked_value - ABSOLUTE_MARKER;
                    }
                }
                if is_abs_y {
                    auto_y = false;
                    // Extract the actual absolute value (remove the marker)
                    if let Some(&marked_value) = style_map.get("size.y") {
                        child_size.y = marked_value - ABSOLUTE_MARKER;
                    }
                }
                
                // Resolve percentages for non-auto sizes
                // For layout children, percentages are relative to the effective parent size (after padding)
                // Use f64 for precision to avoid rounding errors in percentage calculations
                // Round up to ensure elements don't get smaller than intended
                if !auto_x && !is_abs_x {
                    if let Some(&pct) = style_map.get("size.x") {
                        // Use the effective parent size (after padding) as the reference for layout children
                        child_size.x = (effective_parent_size.x as f64 * (pct as f64 / 100.0)).ceil() as f32;
                    }
                }
                
                if !auto_y && !is_abs_y {
                    if let Some(&pct) = style_map.get("size.y") {
                        // Use the effective parent size (after padding) as the reference for layout children
                        child_size.y = (effective_parent_size.y as f64 * (pct as f64 / 100.0)).ceil() as f32;
                    }
                }
                
                (child_id, child_size, auto_x, auto_y)
            })
        })
        .collect();

    if child_info.is_empty() {
        return Vec::new();
    }

    // Calculate gaps and resolve auto-sizing
    // Default gap = 1/n of parent (where n = number of children)
    // Gap attribute adds EXTRA spacing on top
    let _individual_gaps: Vec<f32> = Vec::new();
    
    match &container_mode {
        ContainerMode::Horizontal => {
            let auto_count = child_info.iter().filter(|(_, _, auto_x, _)| *auto_x).count();
            
            if !use_content_based && effective_parent_size.x >= 1.0 {
                // Parent has explicit size - calculate all child sizes first
                if auto_count > 0 {
                    // We have auto children - calculate their sizes
                    // Use f64 for precision to avoid rounding errors
                    let total_fixed: f64 = child_info.iter()
                        .map(|(_, size, auto_x, _)| if !*auto_x { size.x as f64 } else { 0.0 })
                        .sum();
                    
                    // Remaining space for auto children (each gets 1/n of remaining)
                    // Use effective parent size (after padding)
                    let remaining = effective_parent_size.x as f64 - total_fixed;
                    let auto_width = remaining / auto_count as f64;
                    
                    // Apply auto sizes
                    // Round up to ensure elements don't get smaller than intended
                    for (_, size, auto_x, _) in child_info.iter_mut() {
                        if *auto_x {
                            size.x = auto_width.max(0.0).ceil() as f32;
                        }
                    }
                }
            } else {
                // Parent is auto-sizing - use defaults for auto children
                if auto_count > 0 {
                    let default_width = 100.0;
                    for (_, size, auto_x, _) in child_info.iter_mut() {
                        if *auto_x {
                            size.x = default_width;
                        }
                    }
                }
            }
            
            // Gaps will be recalculated later to only include gaps between visible elements
            // This is a placeholder - actual gap calculation happens after filtering
        }
        ContainerMode::Vertical => {
            let auto_count = child_info.iter().filter(|(_, _, _, auto_y)| *auto_y).count();
            
            if !use_content_based && effective_parent_size.y >= 1.0 {
                // Parent has explicit size - calculate all child sizes first
                if auto_count > 0 {
                    // We have auto children - calculate their sizes
                    // Use f64 for precision to avoid rounding errors
                    let total_fixed: f64 = child_info.iter()
                        .map(|(_, size, _, auto_y)| if !*auto_y { size.y as f64 } else { 0.0 })
                        .sum();
                    
                    // Remaining space for auto children (each gets 1/n of remaining)
                    // Use effective parent size (after padding)
                    let remaining = effective_parent_size.y as f64 - total_fixed;
                    let auto_height = remaining / auto_count as f64;
                    
                    // Apply auto sizes
                    // Round up to ensure elements don't get smaller than intended
                    for (_, size, _, auto_y) in child_info.iter_mut() {
                        if *auto_y {
                            size.y = auto_height.max(0.0).ceil() as f32;
                        }
                    }
                }
            } else {
                // Parent is auto-sizing - use defaults for auto children
                if auto_count > 0 {
                    let default_height = 100.0;
                    for (_, size, _, auto_y) in child_info.iter_mut() {
                        if *auto_y {
                            size.y = default_height;
                        }
                    }
                }
            }
            
            // Gaps will be recalculated later to only include gaps between visible elements
            // This is a placeholder - actual gap calculation happens after filtering
        }
        ContainerMode::Grid => {
            // Grid layout doesn't support auto-sizing in the same way
            // Auto-sized children in grid would need special handling
        }
    }
    
    // Inherit perpendicular dimension from parent layout
    // HLayout children inherit height, VLayout children inherit width
    // Use effective parent size (after padding)
    match &container_mode {
        ContainerMode::Horizontal => {
            // For horizontal layouts, children without height inherit effective parent height
            for (_, size, _, auto_y) in child_info.iter_mut() {
                if *auto_y && effective_parent_size.y >= 1.0 {
                    size.y = effective_parent_size.y;
                }
            }
        }
        ContainerMode::Vertical => {
            // For vertical layouts, children without width inherit effective parent width
            for (_, size, auto_x, _) in child_info.iter_mut() {
                if *auto_x && effective_parent_size.x >= 1.0 {
                    size.x = effective_parent_size.x;
                }
            }
        }
        ContainerMode::Grid => {
            // Grid layout could inherit both dimensions, but that's more complex
            // For now, we'll leave grid as-is
        }
    }
    
    // Apply calculated sizes to the actual elements
    for (child_id, size, _, _) in &child_info {
        if let Some(child) = elements.get_mut(child_id) {
            child.set_size(*size);
        }
    }
    
    // Recalculate gaps only between visible elements (non-zero size)
    let mut visible_gaps: Vec<f32> = Vec::new();
    let mut prev_was_visible = false;
    for (_, size, _, _) in &child_info {
        let is_visible = size.x > 0.0 || size.y > 0.0;
        if prev_was_visible && is_visible {
            // Add gap between two visible elements
            match container_mode {
                ContainerMode::Horizontal => visible_gaps.push(extra_gap.x),
                ContainerMode::Vertical => visible_gaps.push(extra_gap.y),
                ContainerMode::Grid => {}, // Grid handles gaps differently
            }
        }
        prev_was_visible = is_visible;
    }
    
    // Convert to format expected by layout functions
    let child_info_simple: Vec<(Uuid, Vector2)> = child_info
        .iter()
        .map(|(id, size, _, _)| (*id, *size))
        .collect();
    
    // Use calculated individual gaps (now only between visible elements)
    let mut positions = match container_mode {
        ContainerMode::Horizontal => {
            if !visible_gaps.is_empty() {
                calculate_horizontal_layout_with_individual_gaps(&child_info_simple, &visible_gaps)
            } else {
                // Fallback: use default gap calculation
                let default_gap = Vector2::new(extra_gap.x, 0.0);
                calculate_horizontal_layout(&child_info_simple, default_gap)
            }
        }
        ContainerMode::Vertical => {
            if !visible_gaps.is_empty() {
                calculate_vertical_layout_with_individual_gaps(&child_info_simple, &visible_gaps)
            } else {
                // Fallback: use default gap calculation
                let default_gap = Vector2::new(0.0, extra_gap.y);
                calculate_vertical_layout(&child_info_simple, default_gap)
            }
        }
        ContainerMode::Grid => {
            // Get grid cols from parent (need to re-borrow)
            let cols = if let Some(p) = elements.get(parent_id) {
                if let UIElement::GridLayout(grid) = p {
                    grid.cols
                } else {
                    1
                }
            } else {
                1
            };
            let mut grid_positions = calculate_grid_layout(&child_info_simple, extra_gap, cols);
            
            // Apply padding offset to grid positions
            // Shift content right by left padding and down by top padding
            let padding_offset_x = padding.left - padding.right;
            let padding_offset_y = padding.top - padding.bottom;
            for (_, pos) in &mut grid_positions {
                pos.x += padding_offset_x;
                pos.y += padding_offset_y;
            }
            
            grid_positions
        }
    };
    
    // Adjust positions based on layout alignment for horizontal layouts
    // Account for padding by offsetting positions
    if matches!(&container_mode, ContainerMode::Horizontal) {
        // Apply vertical padding offset to center children within the padded area
        // The effective area is centered, so offset by (top - bottom) / 2
        let padding_offset_y = (padding.top - padding.bottom) * 0.5;
        for (_, pos) in &mut positions {
            pos.y += padding_offset_y;
        }
        
        // Use layout alignment (align parameter) instead of parent anchor
        match layout_align {
            LayoutAlignment::Start => {
                // Start align: shift all positions to start from left edge (accounting for padding)
                if let Some((_, first_pos)) = positions.first() {
                    if let Some((_, first_size)) = child_info_simple.first() {
                        // First child's left edge = first_pos.x - first_size.x * 0.5
                        // We want it at -parent_size.x * 0.5 + padding.left
                        let first_left = first_pos.x - first_size.x * 0.5;
                        let target_left = -parent_size.x * 0.5 + padding.left;
                        let offset = target_left - first_left;
                        for (_, pos) in &mut positions {
                            pos.x += offset;
                        }
                    }
                }
            }
            LayoutAlignment::End => {
                // End align: shift all positions to end at right edge (accounting for padding)
                if let Some((_, last_pos)) = positions.last() {
                    if let Some((_, last_size)) = child_info_simple.last() {
                        // Last child's right edge = last_pos.x + last_size.x * 0.5
                        // We want it at parent_size.x * 0.5 - padding.right
                        let last_right = last_pos.x + last_size.x * 0.5;
                        let target_right = parent_size.x * 0.5 - padding.right;
                        let offset = target_right - last_right;
                        for (_, pos) in &mut positions {
                            pos.x += offset;
                        }
                    }
                }
            }
            LayoutAlignment::Center => {
                // Center (default) - positions are already centered, just apply horizontal padding offset
                let padding_offset_x = (padding.left - padding.right) * 0.5;
                for (_, pos) in &mut positions {
                    pos.x += padding_offset_x;
                }
            }
        }
    }
    
    // Adjust positions based on layout alignment for vertical layouts
    // Account for padding by offsetting positions
    if matches!(&container_mode, ContainerMode::Vertical) {
        match layout_align {
            LayoutAlignment::Start => {
                // Start align: shift all positions to start from top edge (accounting for padding)
                if let Some((_, first_pos)) = positions.first() {
                    if let Some((_, first_size)) = child_info_simple.first() {
                        // First child's top edge = first_pos.y + first_size.y * 0.5
                        // We want it at parent_size.y * 0.5 - padding.top (top of effective area)
                        let first_top = first_pos.y + first_size.y * 0.5;
                        let target_top = parent_size.y * 0.5 - padding.top;
                        let offset = target_top - first_top;
                        for (_, pos) in &mut positions {
                            pos.y += offset;
                        }
                    }
                }
            }
            LayoutAlignment::End => {
                // End align: shift all positions to end at bottom edge (accounting for padding)
                if let Some((_, last_pos)) = positions.last() {
                    if let Some((_, last_size)) = child_info_simple.last() {
                        // Last child's bottom edge = last_pos.y - last_size.y * 0.5
                        // We want it at -parent_size.y * 0.5 + padding.bottom (bottom of effective area)
                        let last_bottom = last_pos.y - last_size.y * 0.5;
                        let target_bottom = -parent_size.y * 0.5 + padding.bottom;
                        let offset = target_bottom - last_bottom;
                        for (_, pos) in &mut positions {
                            pos.y += offset;
                        }
                    }
                }
            }
            LayoutAlignment::Center => {
                // Center (default) - center within the effective area (after padding)
                // Apply vertical padding offset to center content within padded area
                let padding_offset_y = (padding.top - padding.bottom) * 0.5;
                for (_, pos) in &mut positions {
                    pos.y += padding_offset_y;
                }
            }
        }
    }
    
    positions
}

// Keep your layout calculation functions exactly as they were in the working version
fn calculate_horizontal_layout(children: &[(Uuid, Vector2)], gap: Vector2) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    if children.is_empty() {
        return positions;
    }

    // Calculate total width needed using parallel processing
    // Use f64 for precision during calculations
    let total_child_width: f64 = children.par_iter().map(|(_, size)| size.x as f64).sum();
    let total_gap_width = if children.len() > 1 {
        gap.x as f64 * (children.len() - 1) as f64
    } else {
        0.0
    };
    let total_content_width = total_child_width + total_gap_width;

    // Start from the left edge of the content area (which is centered in the parent)
    let start_x = -total_content_width * 0.5;

    // Calculate positions directly to avoid floating point accumulation errors
    // Use f64 for precision during accumulation
    // When gap=0, ensure edges align exactly by calculating from edges
    let mut accumulated_width = 0.0_f64;
    for (child_id, child_size) in children {
        // Calculate position from left edge, then add half width to get center
        // This ensures exact edge alignment when gap=0
        // Use floor for positions to ensure consistent alignment
        let child_left_edge = start_x + accumulated_width;
        let child_x = (child_left_edge + child_size.x as f64 * 0.5).floor() as f32;
        let child_y = 0.0; // Center vertically in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Update accumulated width: move to right edge of current element, then add gap
        accumulated_width += child_size.x as f64 + gap.x as f64;
    }

    positions
}

fn calculate_horizontal_layout_with_individual_gaps(children: &[(Uuid, Vector2)], gaps: &[f32]) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    if children.is_empty() {
        return positions;
    }

    // Calculate total width needed
    // Use f64 for precision during calculations
    let total_child_width: f64 = children.iter().map(|(_, size)| size.x as f64).sum();
    let total_gap_width: f64 = gaps.iter().map(|&g| g as f64).sum();
    let total_content_width = total_child_width + total_gap_width;

    // Start from the left edge of the content area (which is centered in the parent)
    let start_x = -total_content_width * 0.5;

    // Calculate positions directly to avoid floating point accumulation errors
    // Use f64 for precision during accumulation
    // When gap=0, ensure edges align exactly by calculating from edges
    let mut accumulated_width = 0.0_f64;
    for (i, (child_id, child_size)) in children.iter().enumerate() {
        // Calculate position from left edge, then add half width to get center
        // This ensures exact edge alignment when gap=0
        // Use floor for positions to ensure consistent alignment
        let child_left_edge = start_x + accumulated_width;
        let child_x = (child_left_edge + child_size.x as f64 * 0.5).floor() as f32;
        let child_y = 0.0; // Center vertically in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Update accumulated width: move to right edge of current element, then add gap
        accumulated_width += child_size.x as f64;
        if i < gaps.len() {
            accumulated_width += gaps[i] as f64;
        }
    }

    positions
}

fn calculate_vertical_layout(children: &[(Uuid, Vector2)], gap: Vector2) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    if children.is_empty() {
        return positions;
    }

    // Calculate total height needed using parallel processing
    // Use f64 for precision during calculations
    let total_child_height: f64 = children.par_iter().map(|(_, size)| size.y as f64).sum();
    let total_gap_height = if children.len() > 1 {
        gap.y as f64 * (children.len() - 1) as f64
    } else {
        0.0
    };
    let total_content_height = total_child_height + total_gap_height;

    // Start from the top edge of the content area (which is centered in the parent)
    let start_y = total_content_height * 0.5;

    // Calculate positions directly to avoid floating point accumulation errors
    // Use f64 for precision during accumulation
    // When gap=0, ensure edges align exactly by calculating from edges
    let mut accumulated_height = 0.0_f64;
    for (child_id, child_size) in children {
        // Calculate position from top edge, then subtract half height to get center
        // This ensures exact edge alignment when gap=0
        // Use floor for positions to ensure consistent alignment
        let child_top_edge = start_y - accumulated_height;
        let child_y = (child_top_edge - child_size.y as f64 * 0.5).floor() as f32;
        let child_x = 0.0; // Center horizontally in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Update accumulated height: move to bottom edge of current element, then add gap
        accumulated_height += child_size.y as f64 + gap.y as f64;
    }

    positions
}

fn calculate_vertical_layout_with_individual_gaps(children: &[(Uuid, Vector2)], gaps: &[f32]) -> Vec<(Uuid, Vector2)> {
    let mut positions = Vec::new();

    if children.is_empty() {
        return positions;
    }

    // Calculate total height needed
    // Use f64 for precision during calculations
    let total_child_height: f64 = children.iter().map(|(_, size)| size.y as f64).sum();
    let total_gap_height: f64 = gaps.iter().map(|&g| g as f64).sum();
    let total_content_height = total_child_height + total_gap_height;

    // Start from the top edge of the content area (which is centered in the parent)
    let start_y = total_content_height * 0.5;

    // Calculate positions directly to avoid floating point accumulation errors
    // Use f64 for precision during accumulation
    // When gap=0, ensure edges align exactly by calculating from edges
    let mut accumulated_height = 0.0_f64;
    for (i, (child_id, child_size)) in children.iter().enumerate() {
        // Calculate position from top edge, then subtract half height to get center
        // This ensures exact edge alignment when gap=0
        // Use floor for positions to ensure consistent alignment
        let child_top_edge = start_y - accumulated_height;
        let child_y = (child_top_edge - child_size.y as f64 * 0.5).floor() as f32;
        let child_x = 0.0; // Center horizontally in parent

        positions.push((*child_id, Vector2::new(child_x, child_y)));

        // Update accumulated height: move to bottom edge of current element, then add gap
        accumulated_height += child_size.y as f64;
        if i < gaps.len() {
            accumulated_height += gaps[i] as f64;
        }
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
    // Use f64 for precision during calculations
    let max_width = children
        .par_iter()
        .map(|(_, size)| size.x as f64)
        .reduce(|| 0.0, f64::max);
    let max_height = children
        .par_iter()
        .map(|(_, size)| size.y as f64)
        .reduce(|| 0.0, f64::max);

    // Calculate total grid dimensions
    let total_width = max_width * cols as f64
        + if cols > 1 {
            gap.x as f64 * (cols - 1) as f64
        } else {
            0.0
        };
    let total_height = max_height * rows as f64
        + if rows > 1 {
            gap.y as f64 * (rows - 1) as f64
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

        // Calculate cell position using f64 for precision
        let cell_x = (grid_start_x + col as f64 * (max_width + gap.x as f64) + max_width * 0.5) as f32;
        let cell_y = (grid_start_y - row as f64 * (max_height + gap.y as f64) - max_height * 0.5) as f32;

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
/// Filtered version that skips unaffected branches for performance
pub fn update_global_transforms_with_layout_filtered(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
    layout_positions: &HashMap<Uuid, Vector2>,
    _parent_z: i32,
    initial_z_indices: &HashMap<Uuid, i32>,
    affected_elements: &HashSet<Uuid>,
) {
    // OPTIMIZATION: Skip this element entirely if it's not affected
    if !affected_elements.contains(current_id) {
        return;
    }
    
    // Rest is identical to the unfiltered version
    update_global_transforms_with_layout_impl(
        elements,
        current_id,
        parent_global,
        layout_positions,
        _parent_z,
        initial_z_indices,
        Some(affected_elements),
    );
}

pub fn update_global_transforms_with_layout(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
    layout_positions: &HashMap<Uuid, Vector2>,
    _parent_z: i32,
    initial_z_indices: &HashMap<Uuid, i32>,
) {
    update_global_transforms_with_layout_impl(
        elements,
        current_id,
        parent_global,
        layout_positions,
        _parent_z,
        initial_z_indices,
        None,
    );
}

fn update_global_transforms_with_layout_impl(
    elements: &mut IndexMap<Uuid, UIElement>,
    current_id: &Uuid,
    parent_global: &Transform2D,
    layout_positions: &HashMap<Uuid, Vector2>,
    _parent_z: i32,
    initial_z_indices: &HashMap<Uuid, i32>,
    affected_elements: Option<&HashSet<Uuid>>,
) {
    // Get parent info - FIXED: Use the working version's logic
    let (parent_size, parent_z) = {
        let parent_id = elements.get(current_id).map(|el| el.get_parent());

        if let Some(pid) = parent_id {
            if let Some(parent) = elements.get(&pid) {
                let size = *parent.get_size();

               
                (size, parent.get_z_index())
            } else {
                (Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT), 0)
            }
        } else {
            // This is a root element - check its own size
            if let Some(element) = elements.get(current_id) {
                let _size = *element.get_size();
              
            }
            (Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT), 0)
        }
    };

    // Find the reference size for percentages
    let percentage_reference_size = find_percentage_reference_ancestor(elements, current_id)
        .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));

    // Check if this element is a layout container or child of layout (before mutable borrow)
    let (is_layout_container, has_explicit_size, is_child_of_layout) = if let Some(element) = elements.get(current_id) {
        let is_layout = matches!(element, UIElement::Layout(_) | UIElement::GridLayout(_));
        let style_map = element.get_style_map();
        let has_size = style_map.contains_key("size.x") || style_map.contains_key("size.y");
        let parent_id = element.get_parent();
        let is_child = if !parent_id.is_nil() {
            if let Some(parent) = elements.get(&parent_id) {
                matches!(parent, UIElement::Layout(_) | UIElement::GridLayout(_))
            } else {
                false
            }
        } else {
            false
        };
        (is_layout, has_size, is_child)
    } else {
        (false, false, false)
    };

    // Calculate layout positions for this element's children BEFORE mutating
    // OPTIMIZATION: Only calculate if this element has children that are affected
    let child_layout_positions = if let Some(affected) = affected_elements {
        // Check if any children are affected
        if let Some(elem) = elements.get(current_id) {
            let has_affected_child = elem.get_children().iter().any(|child_id| affected.contains(child_id));
            if has_affected_child {
                calculate_layout_positions(elements, current_id)
            } else {
                Vec::new() // No affected children, skip layout calc!
            }
        } else {
            Vec::new()
        }
    } else {
        calculate_layout_positions(elements, current_id) // No filter, calculate normally
    };
    
    let mut child_layout_map = HashMap::new();
    for (child_id, pos) in child_layout_positions {
        child_layout_map.insert(child_id, pos);
    }

    // Now borrow mutably - this is safe because we're done with immutable borrows
    if let Some(element) = elements.get_mut(current_id) {
        let style_map = element.get_style_map().clone(); // clone to break the borrow

        // Apply percentage styles first (but skip auto-sizing containers unless they have explicit percentages)
        // Also skip percentage sizing for children in layouts - they should be sized by the layout system
        if (!is_layout_container || has_explicit_size) && !is_child_of_layout {
            const ABSOLUTE_MARKER: f32 = 10000.0;
            for (key, pct) in style_map.iter() {
                let fraction = *pct / 100.0;

                match key.as_str() {
                    // Size percentages use parent (or first non-auto-sizing layout ancestor)
                    // Use f64 for precision to avoid rounding errors
                    // Skip absolute values (marked with >= 10000)
                    "size.x" => {
                        if *pct < ABSOLUTE_MARKER {
                            element.set_size(Vector2::new(
                                (percentage_reference_size.x as f64 * fraction as f64) as f32,
                                element.get_size().y,
                            ));
                        }
                    }
                    "size.y" => {
                        if *pct < ABSOLUTE_MARKER {
                            element.set_size(Vector2::new(
                                element.get_size().x,
                                (percentage_reference_size.y as f64 * fraction as f64) as f32,
                            ));
                        }
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

        // Check if parent is a layout container (before mutable borrow)
        let (is_in_vlayout, is_in_hlayout, _is_child_of_layout) = {
            if let Some(element) = elements.get(current_id) {
                let pid = element.get_parent();
                let in_v = if !pid.is_nil() {
                    if let Some(parent) = elements.get(&pid) {
                        matches!(parent, UIElement::Layout(layout) if layout.container.mode == ContainerMode::Vertical)
                    } else {
                        false
                    }
                } else {
                    false
                };
                let in_h = if !pid.is_nil() {
                    if let Some(parent) = elements.get(&pid) {
                        matches!(parent, UIElement::Layout(layout) if layout.container.mode == ContainerMode::Horizontal)
                    } else {
                        false
                    }
                } else {
                    false
                };
                let is_child = in_v || in_h || (if !pid.is_nil() {
                    if let Some(parent) = elements.get(&pid) {
                        matches!(parent, UIElement::Layout(_) | UIElement::GridLayout(_))
                    } else {
                        false
                    }
                } else {
                    false
                });
                (in_v, in_h, is_child)
            } else {
                (false, false, false)
            }
        };

        // Re-borrow for the rest of the function
        if let Some(element) = elements.get_mut(current_id) {
            // Calculate actual text size for text elements (before anchoring)
            if let UIElement::Text(text) = element {
                let text_size = calculate_text_size(&text.props.content, text.props.font_size);
                element.set_size(text_size);
            }
            
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
                // The parent's local coordinate system has its center at (0,0)
                // Parent bounds: (-parent_size.x/2, -parent_size.y/2) to (parent_size.x/2, parent_size.y/2)
                // NOTE: If panels appear "double off", try using parent_size directly (current) or half (if coordinate system is different)
                let anchor_reference_size = parent_size;
                
                let (anchor_x, anchor_y) = match element.get_anchor() {
                    // Corners - need to position the element so its corner aligns with parent corner
                    FurAnchor::TopLeft => {
                        // Parent's top-left corner in parent's local space
                        let parent_left = -anchor_reference_size.x * 0.5;
                        let parent_top = anchor_reference_size.y * 0.5;

                        // Position element so its top-left corner is at parent's top-left
                        // Child's top-left relative to its pivot: (child_size.x * (1.0 - pivot.x), -child_size.y * pivot.y)
                        // So child's center should be at: parent_corner - child_top_left_offset
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
                
                // If parent is a VLayout, children should be centered horizontally (x = 0)
                // If parent is an HLayout, children should be centered vertically (y = 0)
                if is_in_vlayout {
                    layout_offset.x = 0.0; // Center horizontally in VLayout
                    layout_offset.y = anchor_y; // Use anchor for vertical positioning
                } else if is_in_hlayout {
                    layout_offset.x = anchor_x; // Use anchor for horizontal positioning
                    layout_offset.y = 0.0; // Center vertically in HLayout
                } else {
                    layout_offset.x = anchor_x;
                    layout_offset.y = anchor_y;
                }
             
            }

            // STEP 3: Apply layout/anchor offset + user translation
            local.position.x += layout_offset.x;
            local.position.y += layout_offset.y;
            
            // For text elements, adjust position to account for baseline positioning
            // Bottom anchors: move down by full height (text bottom should be at anchor)
            // Top anchors: move down by 1.5x height (text top should be at anchor, needs more adjustment)
            // Center/Left/Right: move down by 1.25x height (text center should be at anchor, needs slight adjustment)
            // Other: move down by full height (same as bottom)
            if let UIElement::Text(_) = element {
                match element.get_anchor() {
                    crate::fur_ast::FurAnchor::Top | 
                    crate::fur_ast::FurAnchor::TopLeft | 
                    crate::fur_ast::FurAnchor::TopRight => {
                        // For top anchors, move down by 1.5x height to account for baseline
                        local.position.y -= child_size.y * 1.5;
                    }
                    crate::fur_ast::FurAnchor::Center |
                    crate::fur_ast::FurAnchor::Left |
                    crate::fur_ast::FurAnchor::Right => {
                        // For center/left/right anchors, move down by 1.25x height to vertically center
                        local.position.y -= child_size.y * 1.25;
                    }
                    _ => {
                        // For bottom/other anchors, move down by full height
                        local.position.y -= child_size.y;
                    }
                }
            }



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
            // Use INITIAL z-index from FUR (not accumulated z-index from previous frames)
            // This prevents z-index accumulation across frames
            let element_id = element.get_id();
            let local_z = initial_z_indices.get(&element_id).copied().unwrap_or(0);
            let global_z = if local_z != 0 {
                // If element has explicit z-index from FUR, use it (but still inherit parent offset)
                // Clamp to prevent overflow
                (parent_z.saturating_add(local_z).saturating_add(2)).min(1000000)
            } else {
                // Otherwise use automatic depth-based z-index
                // Clamp to prevent overflow
                (parent_z.saturating_add(2)).min(1000000)
            };
            element.set_z_index(global_z);

            // Get children list before dropping the mutable borrow
            let children_ids = element.get_children().to_vec();

            // STEP 6: Handle button and text input children specially (panel and text are not in elements map)
            // They should be positioned like regular children using the anchor system
            if let UIElement::Button(button) = element {
                // Sync button size to panel (panel should match button size for rendering)
                // This must happen after the button's size is finalized in layout
                button.panel.base.size = button.base.size;
                
                // Calculate actual text size based on content (don't use button size for text)
                let actual_text_size = calculate_text_size(&button.text.props.content, button.text.props.font_size);
                button.text.base.size = actual_text_size;
                
                // Use the button's size as the parent size for anchor calculations
                let button_size = button.base.size;
                
              
                
                // Helper function to calculate anchor offset (same logic as regular children)
                // This matches the anchor calculation in update_global_transforms_with_layout
                // The parent's center is at (0, 0) in parent's local space, regardless of pivot
                let calculate_anchor_offset = |child_size: Vector2, child_pivot: Vector2, anchor: FurAnchor, parent_size: Vector2| -> Vector2 {
                    match anchor {
                        FurAnchor::TopLeft => {
                            // Parent's top-left corner
                            let parent_left = -parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            // Position child so its top-left corner is at parent's top-left
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::TopRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomLeft => {
                            let parent_left = -parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::Top => {
                            let parent_top = parent_size.y * 0.5;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Bottom => {
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Left => {
                            let parent_left = -parent_size.x * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Right => {
                            let parent_right = parent_size.x * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Center => {
                            // Center anchor: no offset needed, pivot points align
                            Vector2::new(0.0, 0.0)
                        }
                    }
                };
                
                // Process panel - positioned like a child using button's anchor
                let panel_size = button.panel.base.size;
                let panel_pivot = button.panel.base.pivot;
                let panel_anchor = button.panel.base.anchor;
                let panel_local = button.panel.base.transform.clone();
                
                // Calculate anchor offset (same as regular children - parent center is at 0,0)
                let panel_anchor_offset = calculate_anchor_offset(panel_size, panel_pivot, panel_anchor, button_size);
                
                // Apply anchor offset + local transform
                let mut panel_local_pos = panel_local.position;
                panel_local_pos.x += panel_anchor_offset.x;
                panel_local_pos.y += panel_anchor_offset.y;
                
                // Combine with button's global transform
                let mut panel_global = Transform2D::default();
                panel_global.scale.x = global.scale.x * panel_local.scale.x;
                panel_global.scale.y = global.scale.y * panel_local.scale.y;
                panel_global.position.x = global.position.x + (panel_local_pos.x * global.scale.x);
                panel_global.position.y = global.position.y + (panel_local_pos.y * global.scale.y);
                panel_global.rotation = global.rotation + panel_local.rotation;
                
                button.panel.base.global_transform = panel_global;
                button.panel.base.z_index = global_z.min(1000000);
                
              
                // Process text - positioned like a child using button's text_anchor
                let text_size = actual_text_size; // Use calculated text size, not button size
                let text_pivot = button.text.base.pivot;
                let text_anchor = button.text_anchor;
                let text_local = button.text.base.transform.clone();
                
                // Calculate anchor offset (same as regular children - parent center is at 0,0)
                let text_anchor_offset = calculate_anchor_offset(text_size, text_pivot, text_anchor, button_size);
                
                // Apply anchor offset + local transform
                let mut text_local_pos = text_local.position;
                text_local_pos.x += text_anchor_offset.x;
                text_local_pos.y += text_anchor_offset.y;
                
                // For button text, adjust position to account for baseline positioning
                // Same logic as regular text elements (lines 862-878)
                match text_anchor {
                    crate::fur_ast::FurAnchor::Top | 
                    crate::fur_ast::FurAnchor::TopLeft | 
                    crate::fur_ast::FurAnchor::TopRight => {
                        // For top anchors, move down by 1.5x height to account for baseline
                        text_local_pos.y -= text_size.y * 1.5;
                    }
                    crate::fur_ast::FurAnchor::Center |
                    crate::fur_ast::FurAnchor::Left |
                    crate::fur_ast::FurAnchor::Right => {
                        // For center/left/right anchors, move down by 1.25x height to vertically center
                        text_local_pos.y -= text_size.y * 1.25;
                    }
                    _ => {
                        // For bottom/other anchors, move down by full height
                        text_local_pos.y -= text_size.y;
                    }
                }
                
                // Combine with button's global transform
                let mut text_global = Transform2D::default();
                text_global.scale.x = global.scale.x * text_local.scale.x;
                text_global.scale.y = global.scale.y * text_local.scale.y;
                text_global.position.x = global.position.x + (text_local_pos.x * global.scale.x);
                text_global.position.y = global.position.y + (text_local_pos.y * global.scale.y);
                text_global.rotation = global.rotation + text_local.rotation;
                
                button.text.base.global_transform = text_global;
                button.text.base.z_index = (global_z + 1).min(1000000); // Text renders on top
                
              
            }
            
            // Handle TextInput children (same logic as Button)
            if let UIElement::TextInput(text_input) = element {
                // Sync text input size to panel
                text_input.panel.base.size = text_input.base.size;
                
                // Calculate actual text size based on content
                let actual_text_size = calculate_text_size(&text_input.text.props.content, text_input.text.props.font_size);
                text_input.text.base.size = actual_text_size;
                
                // Use the text input's size as the parent size for anchor calculations
                let text_input_size = text_input.base.size;
                
                // Helper function to calculate anchor offset (same as Button)
                let calculate_anchor_offset = |child_size: Vector2, child_pivot: Vector2, anchor: FurAnchor, parent_size: Vector2| -> Vector2 {
                    match anchor {
                        FurAnchor::TopLeft => {
                            let parent_left = -parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::TopRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomLeft => {
                            let parent_left = -parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::Top => {
                            let parent_top = parent_size.y * 0.5;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Bottom => {
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Left => {
                            let parent_left = -parent_size.x * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Right => {
                            let parent_right = parent_size.x * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Center => {
                            Vector2::new(0.0, 0.0)
                        }
                    }
                };
                
                // Process panel
                let panel_size = text_input.panel.base.size;
                let panel_pivot = text_input.panel.base.pivot;
                let panel_anchor = text_input.panel.base.anchor;
                let panel_local = text_input.panel.base.transform.clone();
                
                let panel_anchor_offset = calculate_anchor_offset(panel_size, panel_pivot, panel_anchor, text_input_size);
                
                let mut panel_local_pos = panel_local.position;
                panel_local_pos.x += panel_anchor_offset.x;
                panel_local_pos.y += panel_anchor_offset.y;
                
                let mut panel_global = Transform2D::default();
                panel_global.scale.x = global.scale.x * panel_local.scale.x;
                panel_global.scale.y = global.scale.y * panel_local.scale.y;
                panel_global.position.x = global.position.x + (panel_local_pos.x * global.scale.x);
                panel_global.position.y = global.position.y + (panel_local_pos.y * global.scale.y);
                panel_global.rotation = global.rotation + panel_local.rotation;
                
                text_input.panel.base.global_transform = panel_global;
                text_input.panel.base.z_index = global_z.min(1000000);
                
                // Process text
                let text_size = actual_text_size;
                let text_pivot = text_input.text.base.pivot;
                let text_anchor = text_input.text_anchor;
                let text_local = text_input.text.base.transform.clone();
                
                let text_anchor_offset = calculate_anchor_offset(text_size, text_pivot, text_anchor, text_input_size);
                
                let mut text_local_pos = text_local.position;
                text_local_pos.x += text_anchor_offset.x;
                text_local_pos.y += text_anchor_offset.y;
                
                // Adjust position for baseline (same as Button)
                match text_anchor {
                    crate::fur_ast::FurAnchor::Top | 
                    crate::fur_ast::FurAnchor::TopLeft | 
                    crate::fur_ast::FurAnchor::TopRight => {
                        text_local_pos.y -= text_size.y * 1.5;
                    }
                    crate::fur_ast::FurAnchor::Center |
                    crate::fur_ast::FurAnchor::Left |
                    crate::fur_ast::FurAnchor::Right => {
                        text_local_pos.y -= text_size.y * 1.25;
                    }
                    _ => {
                        text_local_pos.y -= text_size.y;
                    }
                }
                
                let mut text_global = Transform2D::default();
                text_global.scale.x = global.scale.x * text_local.scale.x;
                text_global.scale.y = global.scale.y * text_local.scale.y;
                text_global.position.x = global.position.x + (text_local_pos.x * global.scale.x);
                text_global.position.y = global.position.y + (text_local_pos.y * global.scale.y);
                text_global.rotation = global.rotation + text_local.rotation;
                
                text_input.text.base.global_transform = text_global;
                text_input.text.base.z_index = (global_z + 1).min(1000000);
            }
            
            // Handle TextEdit children (same logic as TextInput)
            if let UIElement::TextEdit(text_edit) = element {
                // Sync text edit size to panel
                text_edit.panel.base.size = text_edit.base.size;
                
                // Calculate actual text size based on content
                let actual_text_size = calculate_text_size(&text_edit.text.props.content, text_edit.text.props.font_size);
                text_edit.text.base.size = actual_text_size;
                
                // Use the text edit's size as the parent size for anchor calculations
                let text_edit_size = text_edit.base.size;
                
                // Helper function to calculate anchor offset (same as TextInput)
                let calculate_anchor_offset = |child_size: Vector2, child_pivot: Vector2, anchor: FurAnchor, parent_size: Vector2| -> Vector2 {
                    match anchor {
                        FurAnchor::TopLeft => {
                            let parent_left = -parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::TopRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_top = parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomLeft => {
                            let parent_left = -parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::BottomRight => {
                            let parent_right = parent_size.x * 0.5;
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(offset_x, offset_y)
                        }
                        FurAnchor::Top => {
                            let parent_top = parent_size.y * 0.5;
                            let offset_y = parent_top - child_size.y * child_pivot.y;
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Bottom => {
                            let parent_bottom = -parent_size.y * 0.5;
                            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
                            Vector2::new(0.0, offset_y)
                        }
                        FurAnchor::Left => {
                            let parent_left = -parent_size.x * 0.5;
                            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Right => {
                            let parent_right = parent_size.x * 0.5;
                            let offset_x = parent_right - child_size.x * child_pivot.x;
                            Vector2::new(offset_x, 0.0)
                        }
                        FurAnchor::Center => {
                            Vector2::new(0.0, 0.0)
                        }
                    }
                };
                
                // Process panel
                let panel_size = text_edit.panel.base.size;
                let panel_pivot = text_edit.panel.base.pivot;
                let panel_anchor = text_edit.panel.base.anchor;
                let panel_local = text_edit.panel.base.transform.clone();
                
                let panel_anchor_offset = calculate_anchor_offset(panel_size, panel_pivot, panel_anchor, text_edit_size);
                
                let mut panel_local_pos = panel_local.position;
                panel_local_pos.x += panel_anchor_offset.x;
                panel_local_pos.y += panel_anchor_offset.y;
                
                let mut panel_global = Transform2D::default();
                panel_global.scale.x = global.scale.x * panel_local.scale.x;
                panel_global.scale.y = global.scale.y * panel_local.scale.y;
                panel_global.position.x = global.position.x + (panel_local_pos.x * global.scale.x);
                panel_global.position.y = global.position.y + (panel_local_pos.y * global.scale.y);
                panel_global.rotation = global.rotation + panel_local.rotation;
                
                text_edit.panel.base.global_transform = panel_global;
                text_edit.panel.base.z_index = global_z.min(1000000);
                
                // Process text
                let text_size = actual_text_size;
                let text_pivot = text_edit.text.base.pivot;
                let text_anchor = text_edit.text_anchor;
                let text_local = text_edit.text.base.transform.clone();
                
                let text_anchor_offset = calculate_anchor_offset(text_size, text_pivot, text_anchor, text_edit_size);
                
                let mut text_local_pos = text_local.position;
                text_local_pos.x += text_anchor_offset.x;
                text_local_pos.y += text_anchor_offset.y;
                
                // Adjust position for baseline (same as TextInput)
                match text_anchor {
                    crate::fur_ast::FurAnchor::Top | 
                    crate::fur_ast::FurAnchor::TopLeft | 
                    crate::fur_ast::FurAnchor::TopRight => {
                        text_local_pos.y -= text_size.y * 1.5;
                    }
                    crate::fur_ast::FurAnchor::Center |
                    crate::fur_ast::FurAnchor::Left |
                    crate::fur_ast::FurAnchor::Right => {
                        text_local_pos.y -= text_size.y * 1.25;
                    }
                    _ => {
                        text_local_pos.y -= text_size.y;
                    }
                }
                
                let mut text_global = Transform2D::default();
                text_global.scale.x = global.scale.x * text_local.scale.x;
                text_global.scale.y = global.scale.y * text_local.scale.y;
                text_global.position.x = global.position.x + (text_local_pos.x * global.scale.x);
                text_global.position.y = global.position.y + (text_local_pos.y * global.scale.y);
                text_global.rotation = global.rotation + text_local.rotation;
                
                text_edit.text.base.global_transform = text_global;
                text_edit.text.base.z_index = (global_z + 1).min(1000000);
            }
            
            // Handle CodeEdit children (composed of text_edit + line numbers)
            if let UIElement::CodeEdit(_code_edit) = element {
                // CodeEdit's sync_base_to_children already handles positioning
                // We just need to ensure the layout is calculated
                // The text_edit and line_number components are positioned relative to code_edit
            }

            // STEP 7: Recurse into regular children with their layout positions
            for child_id in children_ids {
                // OPTIMIZATION: Skip children not in affected set (if filtering is enabled)
                if let Some(affected) = affected_elements {
                    if !affected.contains(&child_id) {
                        continue; // Skip this entire branch!
                    }
                }
                
                update_global_transforms_with_layout_impl(
                    elements,
                    &child_id,
                    &global,
                    &child_layout_map,
                    global_z,
                    initial_z_indices,
                    affected_elements,
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
        let initial_z_indices = &ui_node.initial_z_indices;
        for root_id in root_ids {
            update_global_transforms_with_layout(
                elements,
                root_id,
                &Transform2D::default(),
                &empty_layout_map,
                0,
                initial_z_indices,
            );
          
        }
    }
}

fn update_ui_layout_cached(ui_node: &mut UINode, cache: &RwLock<LayoutCache>) {
    if let (Some(root_ids), Some(elements)) = (&ui_node.root_ids, &mut ui_node.elements) {
        // First, sync all buttons', text inputs', text edits', and code edits' base properties
        // This must happen before layout calculation
        for (_, element) in elements.iter_mut() {
            match element {
                UIElement::Button(button) => {
                    button.sync_base_to_children();
                }
                UIElement::TextInput(text_input) => {
                    text_input.sync_base_to_children();
                }
                UIElement::TextEdit(text_edit) => {
                    text_edit.sync_base_to_children();
                }
                UIElement::CodeEdit(code_edit) => {
                    code_edit.sync_base_to_children();
                }
                _ => {}
            }
        }
        
        for root_id in root_ids {
            calculate_all_content_sizes_cached(elements, root_id, cache);
        }

        let empty_layout_map = HashMap::new();
        let initial_z_indices = &ui_node.initial_z_indices;
        for root_id in root_ids {
            update_global_transforms_with_layout(
                elements,
                root_id,
                &Transform2D::default(),
                &empty_layout_map,
                0,
                initial_z_indices,
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

// Helper function to collect all ancestors of dirty elements (needed for layout recalculation)
fn collect_dirty_with_ancestors(
    elements: &IndexMap<Uuid, UIElement>,
    dirty_ids: &HashSet<Uuid>,
) -> HashSet<Uuid> {
    let mut to_process = HashSet::new();
    
    for &dirty_id in dirty_ids {
        // Add the dirty element itself
        to_process.insert(dirty_id);
        
        // Walk up the parent chain and add all ancestors
        let mut current_id = dirty_id;
        while let Some(element) = elements.get(&current_id) {
            let parent_id = element.get_parent();
            if parent_id.is_nil() {
                break;
            }
            to_process.insert(parent_id);
            current_id = parent_id;
        }
    }
    
    to_process
}

// Updated render function with caching and dirty element optimization
pub fn render_ui(ui_node: &mut UINode, gfx: &mut Graphics, provider: Option<&dyn crate::script::ScriptProvider>) {
    let cache = get_layout_cache();
    
    // Get timestamp from UINode's base node
    let timestamp = ui_node.base.created_timestamp;

    // Check if fur_path has changed or if elements need to be loaded
    // Extract fur_path string first to avoid borrow conflicts
    {
        let current_fur_path_str = ui_node.fur_path.as_ref().map(|fp| fp.as_ref().to_string());
        let loaded_fur_path_str = ui_node.loaded_fur_path.as_ref().map(|fp| fp.as_ref().to_string());
        
        let needs_load = current_fur_path_str.as_ref().map(|current| {
            loaded_fur_path_str.as_ref().map(|loaded| loaded != current).unwrap_or(true)
        }).unwrap_or(false);
        
        if needs_load {
            if let Some(ref fur_path_str) = current_fur_path_str {
                // Try to load the fur file using the provider if available, otherwise fall back to parse_fur_file
                use crate::apply_fur::build_ui_elements_from_fur;
                let fur_elements_result = if let Some(provider) = provider {
                    // Use provider to load FUR (works in both dev and release mode)
                    provider.load_fur_data(fur_path_str)
                } else {
                    // Fallback: use parse_fur_file directly (for backwards compatibility)
                    use crate::apply_fur::parse_fur_file;
                    parse_fur_file(fur_path_str)
                        .map(|ast| {
                            ast.into_iter()
                                .filter_map(|f| match f {
                                    crate::fur_ast::FurNode::Element(el) => Some(el),
                                    _ => None,
                                })
                                .collect::<Vec<crate::fur_ast::FurElement>>()
                        })
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                };
                
                match fur_elements_result {
                    Ok(fur_elements) => {
                        if !fur_elements.is_empty() {
                            build_ui_elements_from_fur(ui_node, &fur_elements);
                            ui_node.loaded_fur_path = Some(std::borrow::Cow::Owned(fur_path_str.clone()));
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to load FUR file {}: {}", fur_path_str, e);
                    }
                }
            }
        }
    }

    // Check if elements exist - if not, we can't render yet
    let elements_exist = ui_node.elements.is_some();
    
    // Check if we have any dirty elements
    // If empty, mark all elements as dirty for initial render
    if ui_node.needs_rerender.is_empty() {
        ui_node.mark_all_needs_rerender();
    }
    
    // Collect dirty element IDs before borrowing elements
    let dirty_elements: Vec<Uuid> = ui_node.needs_rerender.iter().copied().collect();
    let needs_layout = !ui_node.needs_layout_recalc.is_empty();
    
    // If elements don't exist yet, return early - elements will be marked when FUR loads
    // The UINode will be re-added to scene's needs_rerender after FUR loads
    if !elements_exist {
        return;
    }
    
    // Build visibility cache ONCE for the frame (used by both visibility check and layout)
    let visibility_cache: HashSet<Uuid> = if let Some(elements) = &ui_node.elements {
        let elements_ref: &IndexMap<Uuid, UIElement> = elements;
        elements_ref
            .par_iter()
            .filter_map(|(id, _)| {
                if is_effectively_visible(elements_ref, *id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    } else {
        HashSet::new()
    };
    
    // Build percentage reference cache ONCE (no more repeated parent chain walks!)
    let percentage_ref_cache: HashMap<Uuid, Vector2> = if let Some(elements) = &ui_node.elements {
        let elements_ref: &IndexMap<Uuid, UIElement> = elements;
        elements_ref
            .par_iter()
            .map(|(id, _)| {
                let ref_size = find_percentage_reference_ancestor(elements_ref, id)
                    .unwrap_or(Vector2::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT));
                (*id, ref_size)
            })
            .collect()
    } else {
        HashMap::new()
    };
    
    // Only recalculate layout if layout actually changed (not just visual state like button hover)
    if needs_layout {
        let _layout_start = std::time::Instant::now();
        
        // OPTIMIZATION: Only recalculate dirty elements and their ancestors
        // Don't recalculate the entire tree!
        if let Some(elements) = &mut ui_node.elements {
            // Collect dirty elements + ancestors (parents up to root)
            let dirty_with_ancestors = collect_dirty_with_ancestors(elements, &ui_node.needs_layout_recalc);
            
            
            // Sync button properties only for dirty buttons (not all elements!)
            for dirty_id in &dirty_with_ancestors {
                if let Some(element) = elements.get_mut(dirty_id) {
                    if let UIElement::Button(button) = element {
                        button.sync_base_to_children();
                    }
                }
            }
            
            // Clear cache only for elements we're recalculating
            if let Ok(mut cache_ref) = cache.write() {
                for dirty_id in &dirty_with_ancestors {
                    cache_ref.entries.remove(dirty_id);
                }
            }
            
            // Recalculate content sizes only for dirty elements (bottom-up)
            // We need to process them in order: children before parents
            let mut sorted_dirty: Vec<Uuid> = dirty_with_ancestors.iter().copied().collect();
            // Sort by depth (deeper elements first) by counting ancestors
            sorted_dirty.sort_by_key(|id| {
                let mut depth = 0;
                let mut current = *id;
                while let Some(el) = elements.get(&current) {
                    let parent = el.get_parent();
                    if parent.is_nil() { break; }
                    depth += 1;
                    current = parent;
                    if depth > 100 { break; } // Safety
                }
                -(depth as i32) // Negative so deeper elements come first
            });
            
            // Recalculate content sizes for dirty elements only
            // (visibility_cache was already built at the start of the function)
            for element_id in sorted_dirty {
                if let Some(element) = elements.get(&element_id) {
                    let is_container = matches!(
                        element,
                        UIElement::Layout(_) | UIElement::GridLayout(_) | UIElement::BoxContainer(_)
                    );
                    
                    if is_container {
                        // Use cached visibility AND percentage refs (no parent chain walks!)
                        let content_size = calculate_content_size_with_visibility_cache(
                            elements, 
                            &element_id, 
                            &visibility_cache,
                            &percentage_ref_cache
                        );
                        
                        if let Some(element) = elements.get_mut(&element_id) {
                            element.set_size(content_size);
                        }
                    }
                }
            }
            
            // Build "affected elements" set: dirty elements + all their descendants + immediate siblings
            // This is the minimum set needed for correct positioning without updating the entire tree
            let mut affected_elements = HashSet::new();
            
            for dirty_id in dirty_elements.iter() {
                // Mark the dirty element itself
                affected_elements.insert(*dirty_id);
                
                // Mark all descendants (they inherit from this element)
                let mut stack = vec![*dirty_id];
                while let Some(current_id) = stack.pop() {
                    if let Some(element) = elements.get(&current_id) {
                        for child_id in element.get_children() {
                            if affected_elements.insert(*child_id) {
                                stack.push(*child_id);
                            }
                        }
                    }
                }
                
                // Mark parent + ancestors up the chain (they all need layout recalc)
                if let Some(element) = elements.get(dirty_id) {
                    let mut current_parent = element.get_parent();
                    while !current_parent.is_nil() {
                        if !affected_elements.insert(current_parent) {
                            break; // Already marked, can stop
                        }
                        if let Some(parent_elem) = elements.get(&current_parent) {
                            current_parent = parent_elem.get_parent();
                        } else {
                            break;
                        }
                    }
                }
            }
            
            // Update transforms from root, SKIPPING unaffected branches
            let empty_layout_map = HashMap::new();
            let initial_z_indices = &ui_node.initial_z_indices;
            
            if let Some(root_ids) = &ui_node.root_ids {
                for root_id in root_ids {
                    update_global_transforms_with_layout_filtered(
                        elements,
                        root_id,
                        &Transform2D::default(),
                        &empty_layout_map,
                        0,
                        initial_z_indices,
                        &affected_elements,
                    );
                }
            }
        }
        
        // Uncomment for timing debug:
        // eprintln!("⏱️ Layout recalc took: {:?}", layout_start.elapsed());
    }

    // Now borrow elements for rendering
    if let Some(elements) = &mut ui_node.elements {
        // OPTIMIZATION: Only check dirty elements for visibility changes
        // instead of checking ALL elements every frame
        let dirty_set: HashSet<Uuid> = dirty_elements.iter().copied().collect();
        
        // Collect elements that need visibility checking (dirty elements + their descendants)
        let elements_to_check: HashSet<Uuid> = if dirty_set.is_empty() {
            // First frame: check all elements
            elements.keys().copied().collect()
        } else {
            // Subsequent frames: only check dirty elements and their descendants
            let mut to_check = HashSet::new();
            for dirty_id in &dirty_set {
                to_check.insert(*dirty_id);
                // Add all descendants
                let mut stack = vec![*dirty_id];
                while let Some(current_id) = stack.pop() {
                    if let Some(element) = elements.get(&current_id) {
                        for &child_id in element.get_children() {
                            if to_check.insert(child_id) {
                                stack.push(child_id);
                            }
                        }
                    }
                }
            }
            to_check
        };

        // Use pre-computed visibility cache instead of rechecking!
        let (visible_element_ids, newly_invisible_ids): (Vec<Uuid>, Vec<Uuid>) = {
            let visible: Vec<Uuid> = visibility_cache.iter().copied().collect();
            let mut invisible = Vec::new();
            
            // Find elements that JUST became invisible (were in dirty set but not visible)
            for id in &elements_to_check {
                if !visibility_cache.contains(id) {
                    invisible.push(*id);
                }
            }
            
            (visible, invisible)
        };
        
        // OPTIMIZATION: Only remove newly invisible elements (not all invisible elements)
        // This avoids iterating through everything every frame
        for element_id in newly_invisible_ids {
            if let Some(element) = elements.get(&element_id) {
                match element {
                    UIElement::Panel(_) => {
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, element_id);
                    }
                    UIElement::Text(_) => {
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, element_id);
                    }
                    UIElement::Button(button) => {
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, button.panel.id);
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, button.text.id);
                    }
                    UIElement::TextInput(text_input) => {
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, text_input.panel.id);
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, text_input.text.id);
                    }
                    UIElement::TextEdit(text_edit) => {
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, text_edit.panel.id);
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, text_edit.text.id);
                    }
                    UIElement::CodeEdit(code_edit) => {
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, code_edit.text_edit.panel.id);
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, code_edit.text_edit.text.id);
                        gfx.renderer_ui.remove_panel(&mut gfx.renderer_prim, code_edit.line_number_panel.id);
                        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, code_edit.line_number_text.id);
                    }
                    _ => {}
                }
            }
        }
        
        // Now render only the visible elements (mutable borrow)
        for element_id in visible_element_ids {
            if let Some(element) = elements.get_mut(&element_id) {
                match element {
                    UIElement::BoxContainer(_) => { /* no-op */ }
                    UIElement::Panel(panel) => render_panel(panel, gfx, timestamp),
                    UIElement::GridLayout(_) => { /* no-op */ }
                    UIElement::Layout(_) => {
                        // Layouts don't render themselves - they only manage child positioning
                    }
                    UIElement::Text(text) => render_text(text, gfx, timestamp),
                    UIElement::Button(button) => {
                        // Get the base background color
                        let base_bg = button.panel_props().background_color;
                        
                        // Determine which color to use based on button state
                        let bg_color = if button.is_pressed {
                            // Pressed state: use pressed_bg if specified, otherwise darken base by 20%
                            if let Some(pressed) = button.pressed_bg {
                                Some(pressed)
                            } else if let Some(base) = base_bg {
                                Some(base.darken(0.2))
                            } else {
                                None
                            }
                        } else if button.is_hovered {
                            // Hover state: use hover_bg if specified, otherwise lighten base by 20%
                            if let Some(hover) = button.hover_bg {
                                Some(hover)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.2))
                            } else {
                                None
                            }
                        } else {
                            // Normal state: use base background color
                            base_bg
                        };
                        
                        // Render panel with the appropriate color (create a temporary panel copy)
                        let mut panel_copy = button.panel.clone();
                        panel_copy.props.background_color = bg_color;
                        render_panel(&panel_copy, gfx, timestamp);
                        render_text(&button.text, gfx, timestamp);
                    }
                    UIElement::TextInput(text_input) => {
                        // Scroll offset is calculated in render_text_input_with_clipping
                        // Get the base background color
                        let base_bg = text_input.panel_props().background_color;
                        
                        // Determine which color to use based on text input state
                        let bg_color = if text_input.is_focused {
                            // Focused state: use focused_bg if specified, otherwise lighten base by 10%
                            if let Some(focused) = text_input.focused_bg {
                                Some(focused)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.1))
                            } else {
                                None
                            }
                        } else if text_input.is_hovered {
                            // Hover state: use hover_bg if specified, otherwise lighten base by 5%
                            if let Some(hover) = text_input.hover_bg {
                                Some(hover)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.05))
                            } else {
                                None
                            }
                        } else {
                            // Normal state: use base background color
                            base_bg
                        };
                        
                        // Render panel with the appropriate color
                        let mut panel_copy = text_input.panel.clone();
                        panel_copy.props.background_color = bg_color;
                        render_panel(&panel_copy, gfx, timestamp);
                        
                        // Render selection highlight FIRST (below text) if focused and has selection
                        let selection_id = uuid::Uuid::new_v5(&text_input.text.id, b"selection");
                        if text_input.is_focused && text_input.base.visible {
                            render_text_input_selection(text_input, gfx, timestamp);
                        } else {
                            // Remove selection highlight when unfocused
                            gfx.renderer_prim.remove_rect(selection_id);
                        }
                        
                        // Render text with clipping (only show visible portion)
                        render_text_input_with_clipping(text_input, gfx, timestamp);

                        // Render cursor if focused, visible (blink), and element is visible
                        let cursor_id = uuid::Uuid::new_v5(&text_input.text.id, b"cursor");
                        if text_input.is_focused && text_input.is_cursor_visible() && text_input.base.visible {
                            render_text_input_cursor(text_input, gfx, timestamp);
                        } else {
                            // Remove cursor panel when unfocused OR when not visible (blink off)
                            gfx.renderer_prim.remove_rect(cursor_id);
                        }
                    }
                    UIElement::TextEdit(text_edit) => {
                        // Get the base background color
                        let base_bg = text_edit.panel_props().background_color;
                        
                        // Determine which color to use based on text edit state
                        let bg_color = if text_edit.is_focused {
                            if let Some(focused) = text_edit.focused_bg {
                                Some(focused)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.1))
                            } else {
                                None
                            }
                        } else if text_edit.is_hovered {
                            if let Some(hover) = text_edit.hover_bg {
                                Some(hover)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.05))
                            } else {
                                None
                            }
                        } else {
                            base_bg
                        };
                        
                        // Render panel with the appropriate color
                        let mut panel_copy = text_edit.panel.clone();
                        panel_copy.props.background_color = bg_color;
                        render_panel(&panel_copy, gfx, timestamp);
                        
                        // Render selection highlight if focused and has selection
                        let selection_id = uuid::Uuid::new_v5(&text_edit.panel.base.id, b"selection");
                        if text_edit.is_focused && text_edit.base.visible && text_edit.has_selection() {
                            render_text_edit_selection(text_edit, gfx, timestamp);
                        } else {
                            // Remove selection highlight when unfocused or no selection
                            gfx.renderer_prim.remove_rect(selection_id);
                        }
                        
                        render_text(&text_edit.text, gfx, timestamp);
                        
                        // Render cursor if focused, visible (blink), and element is visible
                        // Note: TextEdit uses panel ID for cursor, not text ID
                        if text_edit.is_focused && text_edit.is_cursor_visible() && text_edit.base.visible {
                            render_text_edit_cursor(text_edit, gfx, timestamp);
                        } else if text_edit.is_focused {
                            // Remove cursor when not visible (blink off) but still focused
                            let cursor_id = uuid::Uuid::new_v5(&text_edit.panel.base.id, b"cursor");
                            gfx.renderer_prim.remove_rect(cursor_id);
                        }
                    }
                    UIElement::CodeEdit(code_edit) => {
                        // Get the base background color
                        let base_bg = code_edit.panel_props().background_color;
                        
                        // Determine which color to use based on code edit state
                        let bg_color = if code_edit.is_focused {
                            if let Some(focused) = code_edit.focused_bg {
                                Some(focused)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.1))
                            } else {
                                None
                            }
                        } else if code_edit.is_hovered {
                            if let Some(hover) = code_edit.hover_bg {
                                Some(hover)
                            } else if let Some(base) = base_bg {
                                Some(base.lighten(0.05))
                            } else {
                                None
                            }
                        } else {
                            base_bg
                        };
                        
                        // Render line number panel
                        render_panel(&code_edit.line_number_panel, gfx, timestamp);
                        render_text(&code_edit.line_number_text, gfx, timestamp);
                        
                        // Render text edit panel
                        let mut panel_copy = code_edit.text_edit.panel.clone();
                        panel_copy.props.background_color = bg_color;
                        render_panel(&panel_copy, gfx, timestamp);
                        
                        // Render selection highlight if focused and has selection
                        let selection_id = uuid::Uuid::new_v5(&code_edit.text_edit.panel.base.id, b"selection");
                        if code_edit.is_focused && code_edit.base.visible && code_edit.has_selection() {
                            render_text_edit_selection(&code_edit.text_edit, gfx, timestamp);
                        } else {
                            // Remove selection highlight when unfocused or no selection
                            gfx.renderer_prim.remove_rect(selection_id);
                        }
                        
                        render_text(&code_edit.text_edit.text, gfx, timestamp);
                        
                        // Render cursor if focused, visible (blink), and element is visible
                        if code_edit.is_focused && code_edit.is_cursor_visible() && code_edit.base.visible {
                            render_text_edit_cursor(&code_edit.text_edit, gfx, timestamp);
                        } else if code_edit.is_focused {
                            // Remove cursor when not visible (blink off) but still focused
                            let cursor_id = uuid::Uuid::new_v5(&code_edit.text_edit.panel.base.id, b"cursor");
                            gfx.renderer_prim.remove_rect(cursor_id);
                        }
                    }
                }
            }
        }
    }
    
    // Clear dirty flags after rendering (outside the elements borrow)
    ui_node.clear_rerender_flags();
}

fn render_panel(panel: &UIPanel, gfx: &mut Graphics, timestamp: u64) {
    // VALIDATION: Skip rendering if size is zero, negative, NaN, or infinite
    // This prevents buffer overflow and GPU errors
    let size = panel.base.size;
    if size.x <= 0.0 || size.y <= 0.0 || !size.x.is_finite() || !size.y.is_finite() {
        // Skip rendering zero-size or invalid panels
        return;
    }
    
    // VALIDATION: Check for invalid transform values
    let transform = panel.base.global_transform;
    if !transform.position.x.is_finite() || !transform.position.y.is_finite() ||
       !transform.scale.x.is_finite() || !transform.scale.y.is_finite() ||
       !transform.rotation.is_finite() {
        // Skip rendering panels with invalid transforms
        return;
    }
    
    let mut background_color = panel
        .props
        .background_color
        .clone()
        .unwrap_or(Color::new(0, 0, 0, 0));
    let opacity = panel.props.opacity;

    // Apply opacity to background color alpha
    background_color.a = ((background_color.a as f32 * opacity) as u8).min(255);

    let corner_radius = panel.props.corner_radius;
    let mut border_color = panel.props.border_color.clone();
    let border_thickness = panel.props.border_thickness;
    let z_index = panel.z_index;
    let bg_id = panel.id;

    gfx.renderer_ui.queue_panel(
        &mut gfx.renderer_prim,
        bg_id,
        transform,
        size,
        panel.base.pivot,
        background_color,
        Some(corner_radius),
        0.0,
        false,
        z_index,
        timestamp,
    );

    if border_thickness > 0.0 {
        if let Some(ref mut bc) = border_color {
            // Apply opacity to border color alpha
            bc.a = ((bc.a as f32 * opacity) as u8).min(255);
            let border_id = Uuid::new_v5(&bg_id, b"border");
            gfx.renderer_ui.queue_panel(
                &mut gfx.renderer_prim,
                border_id,
                panel.base.global_transform,
                panel.base.size,
                panel.base.pivot,
                *bc,
                Some(corner_radius),
                border_thickness,
                true,
                z_index + 1,
                timestamp,
            );
        }
    }
}


/// Calculate text size from font metrics
/// Returns (width, height) where height = (ascent + descent) * scale
/// Calculate cumulative character positions (width at each character boundary)
pub fn calculate_character_positions(text: &str, font_size: f32) -> Vec<f32> {
    use fontdue::Font as Fontdue;
    use fontdue::FontSettings;

    const DESIGN_SIZE: f32 = 192.0; // Must match the font atlas design size!
    
    if let Some(font) = Font::from_name("NotoSans", Weight::Regular, Style::Normal) {
        let fd_font = Fontdue::from_bytes(font.data, FontSettings::default())
            .expect("Invalid font data");
        
        let scale = font_size / DESIGN_SIZE;
        let mut cumulative_width = 0.0;
        let mut positions = Vec::with_capacity(text.len());
        
        for ch in text.chars() {
            let (metrics, _) = fd_font.rasterize(ch, DESIGN_SIZE);
            cumulative_width += metrics.advance_width as f32 * scale;
            positions.push(cumulative_width);
        }
        
        return positions;
    }
    
    // Fallback: approximate positions
    let char_width = font_size * 0.6;
    (0..text.len())
        .map(|i| (i + 1) as f32 * char_width)
        .collect()
}

fn calculate_text_size(text: &str, font_size: f32) -> Vector2 {
    use fontdue::Font as Fontdue;
    use fontdue::FontSettings;
    
    const DESIGN_SIZE: f32 = 192.0; // High resolution for sharp text (3x supersampling)
    
    if let Some(font) = Font::from_name("NotoSans", Weight::Regular, Style::Normal) {
        let fd_font = Fontdue::from_bytes(font.data, FontSettings::default())
            .expect("Invalid font data");
        
        // Get line metrics for height calculation
        if let Some(line_metrics) = fd_font.horizontal_line_metrics(DESIGN_SIZE) {
            let scale = font_size / DESIGN_SIZE;
            let text_height = (line_metrics.ascent + line_metrics.descent) * scale;
            
            // Calculate text width by measuring each character
            let mut text_width = 0.0;
            for ch in text.chars() {
                let (metrics, _) = fd_font.rasterize(ch, DESIGN_SIZE);
                text_width += metrics.advance_width as f32 * scale;
            }
            
            return Vector2::new(text_width, text_height);
        }
    }
    
    // Fallback: use font_size as height if we can't get metrics
    Vector2::new(font_size * text.len() as f32 * 0.6, font_size)
}

// Optimized text rendering - only regenerate atlas when font properties change
fn render_text(text: &UIText, gfx: &mut Graphics, timestamp: u64) {
    // Skip rendering if text content is empty - but remove from cache to clear old text
    if text.props.content.is_empty() {
        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, text.id);
        return;
    }
    
    let font_key = ("NotoSans".to_string(), 192);
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
                        let font_atlas = FontAtlas::new(font, 192.0);
                        gfx.initialize_font_atlas(font_atlas);
                        cache.insert(font_key, true);
                    }
                }
            }
        }
    }

    // Convert TextFlow to horizontal alignment only
    // align parameter only affects horizontal text flow (left/center/right)
    // Vertical alignment is always Center (text is positioned at the anchor point's y coordinate)
    // align=start means left alignment (text starts at anchor, flows right)
    // align=end means right alignment (text ends at anchor, flows left)
    let align_h = match text.props.align {
        crate::ui_elements::ui_text::TextFlow::Start => crate::ui_elements::ui_text::TextAlignment::Left,   // Start = Left (text starts at anchor, flows right)
        crate::ui_elements::ui_text::TextFlow::Center => crate::ui_elements::ui_text::TextAlignment::Center,
        crate::ui_elements::ui_text::TextFlow::End => crate::ui_elements::ui_text::TextAlignment::Right,   // End = Right (text ends at anchor, flows left)
    };
    // Vertical alignment is always Center - align parameter doesn't affect vertical positioning
    let align_v = crate::ui_elements::ui_text::TextAlignment::Center;
    
    gfx.renderer_ui.queue_text_aligned(
        &mut gfx.renderer_prim,
        text.id,
        &text.props.content,
        text.props.font_size,
        text.global_transform,
        text.pivot,
        text.props.color,
        text.z_index,
        timestamp,
        align_h,
        align_v,
    );
}

/// Calculate scroll offset (in pixels) for text input to keep cursor visible
fn calculate_text_input_scroll_offset(text_input: &mut UITextInput) -> f32 {
    let panel_width = text_input.panel.base.size.x;
    let full_text = &text_input.text.props.content;
    let cursor_pos = text_input.cursor_position.min(full_text.len());
    
    // Cache should already be updated in cursor rendering, but check anyway
    if text_input.cached_text_content != *full_text {
        text_input.cached_text_content = full_text.to_string();
        
        if full_text.is_empty() {
            text_input.cached_text_width = 0.0;
            text_input.cached_char_positions.clear();
        } else {
            text_input.cached_char_positions = calculate_character_positions(full_text, text_input.text.props.font_size);
            text_input.cached_text_width = text_input.cached_char_positions.last().copied().unwrap_or(0.0);
        }
    }
    
    let full_text_width = text_input.cached_text_width;
    
    // Add padding (~2 character widths) to give breathing room at edges
    let char_width = text_input.text.props.font_size * 0.6;
    let padding = char_width * 2.0;
    
    // If text fits in panel, no scrolling needed
    if full_text_width <= panel_width {
        return 0.0;
    }
    
    // Get accurate cursor X position from cached character positions
    let cursor_x = if cursor_pos == 0 {
        0.0
    } else if cursor_pos <= text_input.cached_char_positions.len() {
        text_input.cached_char_positions[cursor_pos - 1]
    } else {
        full_text_width
    };
    
    // Current scroll offset
    let current_scroll = text_input.scroll_offset;
    
    // Scroll calculation with equal side padding:
    // Cursor on screen = panel_left + side_padding + (cursor_x - scroll)
    // For cursor to be at right edge with padding:
    // panel_left + side_padding + (cursor_x - scroll) = panel_left + panel_width - side_padding
    // => cursor_x - scroll = panel_width - 2*side_padding
    // => cursor_x = scroll + panel_width - 2*side_padding
    
    let side_padding = 5.0;
    
    // Boundaries in text coordinates where cursor should be
    let min_cursor_text_pos = current_scroll + side_padding;
    let max_cursor_text_pos = current_scroll + panel_width - 2.0 * side_padding;
    
    let new_scroll = if cursor_x < min_cursor_text_pos {
        // Cursor scrolled off left - scroll so cursor is at left boundary (with padding)
        (cursor_x - side_padding).max(0.0)
    } else if cursor_x > max_cursor_text_pos {
        // Cursor scrolled off right - scroll so cursor is at right boundary (with padding)
        cursor_x - (panel_width - 2.0 * side_padding)
    } else {
        // Cursor is visible within padding bounds
        current_scroll
    };
    
    // Clamp scroll to valid range
    // The max scroll should account for side padding on both sides
    let usable_width = panel_width - 2.0 * side_padding;
    let max_scroll = (full_text_width - usable_width).max(0.0);
    let clamped_scroll = new_scroll.max(0.0).min(max_scroll);

    clamped_scroll
}

/// Render text input with horizontal scrolling and clipping
fn render_text_input_with_clipping(text_input: &mut UITextInput, gfx: &mut Graphics, timestamp: u64) {
    // If text is empty, explicitly remove it and return early
    if text_input.text.props.content.is_empty() {
        gfx.renderer_ui.remove_text(&mut gfx.renderer_prim, text_input.text.id);
        return;
    }
    
    // Update scroll offset to keep cursor visible
    text_input.scroll_offset = calculate_text_input_scroll_offset(text_input);
    
    let panel_width = text_input.panel.base.size.x;
    let full_text_width = text_input.cached_text_width;
    
    // If text fits entirely, no scrolling or clipping needed
    if full_text_width <= panel_width {
        text_input.scroll_offset = 0.0;
        let mut text_copy = text_input.text.clone();
        text_copy.base.z_index = text_input.panel.base.z_index + 2; // Text at z+2
        render_text(&text_copy, gfx, timestamp);
        return;
    }
    
    // Text is longer than panel - need to show only visible portion
    // Calculate which part of text is visible based on scroll
    // The visible window in text coordinates is simply [scroll, scroll + panel_width]
    // Side padding affects screen positioning, not which characters are included
    // Add safety margin on right to prevent bleeding (especially when scrolling right)
    let side_padding = 5.0;
    let right_margin = 5.0;
    let visible_start_x = text_input.scroll_offset;
    let visible_end_x = text_input.scroll_offset + panel_width - right_margin;
    
    // Find which characters are visible
    let mut start_char = 0;
    let mut end_char = text_input.text.props.content.len();
    
    if !text_input.cached_char_positions.is_empty() {
        // Find characters FULLY within the visible window [visible_start_x, visible_end_x]
        // This prevents bleeding at panel edges by excluding partially visible characters
        let mut found_start = false;
        for i in 0..text_input.cached_char_positions.len() {
            let char_start = if i > 0 {
                text_input.cached_char_positions[i - 1]
            } else {
                0.0
            };
            let char_end = text_input.cached_char_positions[i];
            
            // Only include character if BOTH start and end are within visible bounds
            if char_start >= visible_start_x && char_end <= visible_end_x {
                if !found_start {
                    start_char = i;
                    found_start = true;
                }
                end_char = i + 1;
            } else if char_end > visible_end_x {
                // Character extends past visible area, stop
                break;
            }
        }
    }
    
    // Get visible substring
    let visible_text = if start_char < end_char && end_char <= text_input.text.props.content.len() {
        &text_input.text.props.content[start_char..end_char]
    } else {
        &text_input.text.props.content
    };
    
    // Calculate where this visible text should be positioned
    // It should appear to start at the left edge of the panel
    let panel_left = text_input.panel.base.global_transform.position.x 
        - (text_input.panel.base.size.x * text_input.panel.base.pivot.x);
    
    // Offset within the visible text (how far into first character)
    let start_offset = if start_char > 0 && start_char <= text_input.cached_char_positions.len() {
        text_input.cached_char_positions[start_char - 1]
    } else {
        0.0
    };
    
    // Render visible text, aligned to left edge of panel (with side padding)
    // Override alignment to Start so text appears from left edge
    let side_padding = 5.0;
    let mut text_copy = text_input.text.clone();
    text_copy.props.content = visible_text.to_string();
    text_copy.props.align = crate::ui_elements::ui_text::TextFlow::Start;
    // Text positioned: panel_left + side_padding + (where_this_char_starts - scroll)
    // start_offset is where the first visible char starts in text coordinates
    // scroll is text_input.scroll_offset
    text_copy.base.transform.position.x = panel_left + side_padding + (start_offset - text_input.scroll_offset);
    text_copy.base.global_transform.position.x = panel_left + side_padding + (start_offset - text_input.scroll_offset);
    text_copy.base.z_index = text_input.panel.base.z_index + 2; // Text at z+2
    
    render_text(&text_copy, gfx, timestamp);
}

/// Render selection highlight for a focused TextInput element
fn render_text_input_selection(text_input: &mut UITextInput, gfx: &mut Graphics, timestamp: u64) {
    // Check if there's a selection
    if let Some((start, end)) = text_input.get_selection_range() {
        if start == end {
            return; // No selection
        }
        
        let font_size = text_input.text.props.font_size;
        
        // Get character positions
        if text_input.cached_char_positions.is_empty() {
            return;
        }
        
        // Calculate selection start and end positions in text coordinate space
        let selection_start_x = if start == 0 {
            0.0
        } else if start <= text_input.cached_char_positions.len() {
            text_input.cached_char_positions[start - 1]
        } else {
            text_input.cached_text_width
        };
        
        let selection_end_x = if end == 0 {
            0.0
        } else if end <= text_input.cached_char_positions.len() {
            text_input.cached_char_positions[end - 1]
        } else {
            text_input.cached_text_width
        };
        
        let selection_width = selection_end_x - selection_start_x;
        
        // Position selection based on whether text is scrolled or centered
        let panel_width = text_input.panel.base.size.x;
        let full_text_width = text_input.cached_text_width;
        let side_padding = 5.0;
        
        let (selection_global_x, visible_selection_width) = if full_text_width <= panel_width {
            // Text fits entirely - it's centered in the panel
            let panel_center_x = text_input.panel.base.global_transform.position.x;
            let text_start_x = panel_center_x - (full_text_width / 2.0);
            let selection_x = text_start_x + selection_start_x;
            (selection_x, selection_width)
        } else {
            // Text is scrolled - left-aligned with padding
            // Clip selection to visible bounds
            let visible_start_x = text_input.scroll_offset;
            let visible_end_x = text_input.scroll_offset + panel_width - (2.0 * side_padding);
            
            // Clamp selection to visible area
            let visible_sel_start = selection_start_x.max(visible_start_x);
            let visible_sel_end = selection_end_x.min(visible_end_x);
            
            if visible_sel_start >= visible_sel_end {
                // Selection is completely outside visible area
                let selection_id = uuid::Uuid::new_v5(&text_input.text.id, b"selection");
                gfx.renderer_prim.remove_rect(selection_id);
                return;
            }
            
            let vis_width = visible_sel_end - visible_sel_start;
            
            // Calculate panel position
            let panel_left = text_input.panel.base.global_transform.position.x 
                - (text_input.panel.base.size.x * text_input.panel.base.pivot.x);
            
            // Position selection rectangle in panel coordinate space
            let selection_x_in_panel = visible_sel_start - text_input.scroll_offset;
            let selection_x = panel_left + side_padding + selection_x_in_panel;
            (selection_x, vis_width)
        };
        
        // Create selection highlight rectangle
        // Match the text height more closely
        let selection_height = font_size * 1.2;
        // Move up by adding more to align with text
        let selection_y = text_input.text.base.global_transform.position.y + (selection_height * 0.75);
        
        let selection_id = uuid::Uuid::new_v5(&text_input.text.id, b"selection");
        
        // Get selection color: use a lighter version of the background with full opacity
        // Z-index layering will handle visibility (selection between panel and text)
        let selection_color = if let Some(custom_color) = text_input.highlight_color {
            // Use custom highlight color
            custom_color
        } else if let Some(bg_color) = text_input.panel.props.background_color {
            // Lighten by 20% with full opacity - text will render on top
            bg_color.lighten(0.2)
        } else {
            // Fallback to light gray
            crate::structs::Color::new(200, 200, 200, 255)
        };
        
        let mut selection_transform = crate::structs2d::Transform2D::default();
        selection_transform.position = crate::structs2d::Vector2::new(selection_global_x, selection_y);
        
        // Render the selection rectangle (only the visible portion)
        // Use pivot (0.0, 0.5) to align from left edge and center vertically
        // Panel is at z, Selection is at z+1, Text is at z+2
        // This ensures text renders on TOP of the selection, and selection is above panel
        use crate::rendering::renderer_prim::RenderLayer;
        gfx.renderer_prim.queue_rect(
            selection_id,
            RenderLayer::UI,
            selection_transform,
            crate::structs2d::Vector2::new(visible_selection_width, selection_height),
            crate::structs2d::Vector2::new(0.0, 0.5), // Left-aligned horizontally, centered vertically
            selection_color,
            None, // no corner radius
            0.0,  // no border thickness
            false, // not a border
            text_input.panel.base.z_index + 1, // Highlight at z+1 (above panel, below text at z+2)
            timestamp,
        );
    } else {
        // No selection, remove any previous selection highlight
        let selection_id = uuid::Uuid::new_v5(&text_input.text.id, b"selection");
        gfx.renderer_prim.remove_rect(selection_id);
    }
}

/// Render selection highlight for a focused TextEdit element (multiline)
fn render_text_edit_selection(text_edit: &crate::ui_elements::ui_text_edit::UITextEdit, gfx: &mut Graphics, timestamp: u64) {
    // Check if there's a selection
    if let Some((start, end)) = text_edit.get_selection_range() {
        if start == end {
            return; // No selection
        }
        
        let font_size = text_edit.text.props.font_size;
        let line_height = font_size * 1.3;
        let selection_height = font_size * 1.2;
        
        // Get start and end cursor positions
        let start_pos = text_edit.char_index_to_cursor_pos(start);
        let end_pos = text_edit.char_index_to_cursor_pos(end);
        
        // Get selection color - lighter version of background
        let selection_color = if let Some(bg_color) = text_edit.panel.props.background_color {
            bg_color.lighten(0.2)
        } else {
            crate::structs::Color::new(200, 200, 200, 255)
        };
        
        // Calculate text area position
        let text_left = text_edit.text.base.global_transform.position.x - (text_edit.text.base.size.x / 2.0);
        let text_top = text_edit.text.base.global_transform.position.y - (text_edit.text.base.size.y / 2.0);
        
        // Render selection for each line
        for line_idx in start_pos.line..=end_pos.line {
            let line_text = text_edit.get_line(line_idx);
            let char_positions = calculate_character_positions(line_text, font_size);
            
            // Determine start and end columns for this line
            let line_start_col = if line_idx == start_pos.line { start_pos.column } else { 0 };
            let line_end_col = if line_idx == end_pos.line { end_pos.column } else { line_text.len() };
            
            if line_start_col >= line_end_col {
                continue;
            }
            
            // Calculate X positions for this line's selection
            let sel_start_x = if line_start_col == 0 {
                0.0
            } else if line_start_col <= char_positions.len() {
                char_positions[line_start_col - 1]
            } else {
                char_positions.last().copied().unwrap_or(0.0)
            };
            
            let sel_end_x = if line_end_col == 0 {
                0.0
            } else if line_end_col <= char_positions.len() {
                char_positions[line_end_col - 1]
            } else {
                char_positions.last().copied().unwrap_or(0.0)
            };
            
            let sel_width = sel_end_x - sel_start_x;
            if sel_width <= 0.0 {
                continue;
            }
            
            // Calculate Y position for this line
            let line_y = text_top + (line_idx as f32 * line_height) + (line_height * 0.75);
            let sel_x = text_left + sel_start_x;
            
            let selection_id = uuid::Uuid::new_v5(&text_edit.panel.base.id, format!("selection_{}", line_idx).as_bytes());
            
            let mut selection_transform = crate::structs2d::Transform2D::default();
            selection_transform.position = crate::structs2d::Vector2::new(sel_x, line_y);
            
            use crate::rendering::renderer_prim::RenderLayer;
            gfx.renderer_prim.queue_rect(
                selection_id,
                RenderLayer::UI,
                selection_transform,
                crate::structs2d::Vector2::new(sel_width, selection_height),
                crate::structs2d::Vector2::new(0.0, 0.5),
                selection_color,
                None,
                0.0,
                false,
                text_edit.panel.base.z_index + 1, // Between panel and text
                timestamp,
            );
        }
    }
}

/// Render a cursor for a focused TextInput element
fn render_text_input_cursor(text_input: &mut UITextInput, gfx: &mut Graphics, timestamp: u64) {
    let cursor_pos_in_full_text = text_input.cursor_position.min(text_input.text.props.content.len());
    
    // Use cached character positions for accurate cursor placement
    let full_text = &text_input.text.props.content;
    
    // Update cache if text changed (including when cleared to empty)
    if text_input.cached_text_content != *full_text || (full_text.is_empty() && !text_input.cached_char_positions.is_empty()) {
        text_input.cached_text_content = full_text.to_string();
        
        if full_text.is_empty() {
            text_input.cached_text_width = 0.0;
            text_input.cached_char_positions.clear();
        } else {
            // Calculate cumulative character positions for accurate cursor placement
            text_input.cached_char_positions = calculate_character_positions(full_text, text_input.text.props.font_size);
            text_input.cached_text_width = text_input.cached_char_positions.last().copied().unwrap_or(0.0);
        }
    }
    
    // Get accurate width of text before cursor from cached positions
    let text_before_cursor_width = if cursor_pos_in_full_text == 0 {
        0.0
    } else if cursor_pos_in_full_text <= text_input.cached_char_positions.len() {
        text_input.cached_char_positions[cursor_pos_in_full_text - 1]
    } else {
        text_input.cached_text_width
    };
    
    // Get text transform and position
    let text_transform = text_input.text.base.global_transform;
    let text_align = text_input.text.props.align;
    
    // Use cached width (already calculated above)
    let full_text_width = text_input.cached_text_width;
    
    // Calculate cursor X position based on text alignment
    // Note: text_transform.position already accounts for text-anchor (where text is positioned in panel)
    // We just need to calculate the offset based on how text flows relative to that anchor point
    let cursor_x_offset = match text_align {
        crate::ui_elements::ui_text::TextFlow::Start => {
            // align=start (left): text starts at anchor point and flows right
            // Cursor is at the width of text before cursor position
            text_before_cursor_width
        }
        crate::ui_elements::ui_text::TextFlow::Center => {
            // align=center: text is centered on anchor point
            // Cursor = (text_before_cursor_width) - (full_text_width / 2)
            text_before_cursor_width - (full_text_width * 0.5)
        }
        crate::ui_elements::ui_text::TextFlow::End => {
            // align=end (right): text ends at anchor point and flows left
            // Cursor = text_before_cursor_width - full_text_width
            text_before_cursor_width - full_text_width
        }
    };
    
    let font_size = text_input.text.props.font_size;
    
    // Cursor positioning for scrollable text input:
    // The text is rendered as if it were left-aligned starting from panel left edge
    // cursor_x_offset contains the position based on original alignment (we calculated it above)
    // We need to convert this to "absolute position from start of text"
    
    let panel_left = text_input.panel.base.global_transform.position.x 
        - (text_input.panel.base.size.x * text_input.panel.base.pivot.x);
    let panel_width = text_input.panel.base.size.x;
    let full_text_width = text_input.cached_text_width;
    
    // Use deterministic cursor ID so we don't create duplicates
    let cursor_id = uuid::Uuid::new_v5(&text_input.text.id, b"cursor");
    
    // If text fits in panel, use original centered positioning
    if full_text_width <= panel_width {
        let cursor_global_x = text_transform.position.x + cursor_x_offset;
        let cursor_global_y = text_transform.position.y + (font_size * 1.0);
        
        // Create cursor transform
        let mut cursor_transform = Transform2D::default();
        cursor_transform.position = Vector2::new(cursor_global_x, cursor_global_y);
        cursor_transform.scale = Vector2::new(1.0, 1.0);
        cursor_transform.rotation = 0.0;
        
        // Create cursor panel
        let mut cursor_panel = UIPanel::default();
        cursor_panel.base.id = cursor_id;
        cursor_panel.base.size = Vector2::new(1.5, font_size);
        cursor_panel.base.pivot = Vector2::new(0.5, 0.5);
        cursor_panel.base.global_transform = cursor_transform;
        cursor_panel.props.background_color = Some(text_input.text.props.color);
        cursor_panel.base.z_index = text_input.text.base.z_index + 1;
        
        if (text_input.cursor_blink_timer % 1000.0) < 500.0 {
            render_panel(&cursor_panel, gfx, timestamp);
        }
        
        return;
    }
    
    // For scrollable text, cursor position is based on actual character widths
    // text_before_cursor_width is the cumulative width from start of text to cursor position
    let side_padding = 5.0;
    
    // Text rendering logic with side padding:
    // - Visible window in text coordinates: [scroll, scroll + panel_width]
    // - Text is rendered starting at: panel_left + side_padding
    // - A character at text position P appears at: panel_left + side_padding + (P - scroll)
    let cursor_global_x = panel_left + side_padding + (text_before_cursor_width - text_input.scroll_offset);
    let cursor_global_y = text_transform.position.y + (font_size * 1.0);
    
    // Create cursor transform with top-left pivot so Y position is at the top of the cursor
    let mut cursor_transform = Transform2D::default();
    cursor_transform.position = Vector2::new(cursor_global_x, cursor_global_y);
    cursor_transform.scale = Vector2::new(1.0, 1.0);
    cursor_transform.rotation = 0.0;
    
    // Cursor is a thin vertical line (1.5px wide, font_size tall) - thin to clearly show between letters
    let cursor_width = 1.5;
    let cursor_height = font_size;
    
    // Use text color for cursor
    let cursor_color = text_input.text.props.color;
    
    // Create a temporary panel for the cursor (cursor_id already defined above)
    use crate::ui_elements::ui_container::UIPanel;
    let mut cursor_panel = UIPanel::default();
    cursor_panel.base.id = cursor_id;
    cursor_panel.base.size = Vector2::new(cursor_width, cursor_height);
    cursor_panel.base.global_transform = cursor_transform;
    cursor_panel.base.pivot = Vector2::new(0.5, 0.5); // Center pivot so cursor is centered between characters
    cursor_panel.props.background_color = Some(cursor_color);
    cursor_panel.base.z_index = text_input.text.base.z_index + 1; // Cursor on top of text
    
    // Only render if cursor is in visible phase of blink cycle
    if (text_input.cursor_blink_timer % 1000.0) < 500.0 {
        render_panel(&cursor_panel, gfx, timestamp);
    }
}

/// Render a cursor for a focused TextEdit element (multiline)
fn render_text_edit_cursor(text_edit: &UITextEdit, gfx: &mut Graphics, timestamp: u64) {
    // Get the current line
    let line_text = text_edit.get_line(text_edit.cursor_pos.line);
    let text_before_cursor = &line_text[..text_edit.cursor_pos.column.min(line_text.len())];
    
    // Estimate character width
    let char_width = text_edit.text.props.font_size * 0.6;
    let text_width = text_before_cursor.len() as f32 * char_width;
    
    // Get text transform
    let text_transform = text_edit.text.base.global_transform;
    
    // Calculate cursor X position (always left-aligned for multiline)
    let cursor_x_offset = text_width;
    
    // Calculate cursor Y position based on line number
    let font_size = text_edit.text.props.font_size;
    let line_height = font_size * 1.2; // Line spacing
    let cursor_y_offset = -(text_edit.cursor_pos.line as f32 * line_height);
    
    // Apply baseline alignment adjustment - text anchor is center
    let baseline_offset = -(font_size * 0.5);

    // Cursor position in text's local space
    let cursor_local_x = cursor_x_offset;
    let cursor_local_y = cursor_y_offset + baseline_offset;
    
    // Transform to global space
    let cursor_global_x = text_transform.position.x + (cursor_local_x * text_transform.scale.x);
    let cursor_global_y = text_transform.position.y + (cursor_local_y * text_transform.scale.y);
    
    // Create cursor transform
    let mut cursor_transform = Transform2D::default();
    cursor_transform.position = Vector2::new(cursor_global_x, cursor_global_y);
    cursor_transform.scale = text_transform.scale;
    cursor_transform.rotation = text_transform.rotation;
    
    // Cursor is a thin vertical line
    let cursor_width = 2.0;
    let cursor_height = text_edit.text.props.font_size;
    
    // Use text color for cursor
    let cursor_color = text_edit.text.props.color;
    
    // Render cursor as a small rectangle
    let cursor_id = uuid::Uuid::new_v5(&text_edit.text.id, b"cursor");
    
    let mut cursor_panel = UIPanel::default();
    cursor_panel.base.id = cursor_id;
    cursor_panel.base.size = Vector2::new(cursor_width, cursor_height);
    cursor_panel.base.global_transform = cursor_transform;
    cursor_panel.base.pivot = Vector2::new(0.0, 0.5); // Left-center pivot
    cursor_panel.props.background_color = Some(cursor_color);
    cursor_panel.base.z_index = text_edit.text.base.z_index + 1;
    
    render_panel(&cursor_panel, gfx, timestamp);
}
