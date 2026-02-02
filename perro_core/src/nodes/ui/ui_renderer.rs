use crate::ids::UIElementID;
use rayon::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    sync::{OnceLock, RwLock},
};

use crate::{
    Graphics,
    font::{Font, FontAtlas, Style, Weight},
    fur_ast::FurAnchor,
    structs::Color,
    structs2d::{Transform2D, Vector2},
    ui_element::{BaseElement, UIElement},
    ui_elements::{ui_button::UIButton, ui_container::UIPanel, ui_text::UIText},
    ui_node::UINode,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct LayoutSignature {
    size: (i32, i32),
    anchor: FurAnchor,
    children_count: usize,
    children_order: Vec<UIElementID>,
    style_affecting_layout: Vec<(String, i32)>,
}

impl LayoutSignature {
    fn from_element(element: &UIElement) -> Self {
        let size = element.get_size();
        let size_int = ((size.x * 1000.0) as i32, (size.y * 1000.0) as i32);

        let children_order = element.get_children().to_vec();

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
            style_affecting_layout,
        }
    }
}

#[derive(Debug)]
struct LayoutCacheEntry {
    signature: LayoutSignature,
    content_size: Vector2,
}

#[derive(Debug, Default)]
pub struct LayoutCache {
    entries: HashMap<UIElementID, LayoutCacheEntry>,
}

impl LayoutCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn get_cached_content_size(
        &self,
        id: &UIElementID,
        signature: &LayoutSignature,
    ) -> Option<Vector2> {
        self.entries
            .get(id)
            .filter(|entry| entry.signature == *signature)
            .map(|entry| entry.content_size)
    }

    fn cache_results(
        &mut self,
        id: UIElementID,
        signature: LayoutSignature,
        content_size: Vector2,
    ) {
        self.entries.insert(
            id,
            LayoutCacheEntry {
                signature,
                content_size,
            },
        );
    }
}

/// Default viewport size when not provided (e.g. when layout is run without Graphics).
const DEFAULT_VIEWPORT: Vector2 = Vector2 {
    x: 1920.0,
    y: 1080.0,
};

/// Helper function to find the parent element for percentage calculations
/// Uses layout containers with explicit sizes, but skips auto-sizing layout containers.
/// When no suitable parent is found, returns viewport_size or DEFAULT_VIEWPORT.
fn find_percentage_reference_ancestor(
    elements: &HashMap<UIElementID, UIElement>,
    current_id: &UIElementID,
    viewport_size: Option<Vector2>,
) -> Option<Vector2> {
    let _current = elements.get(current_id)?;

    // Walk up the parent chain to find a non-zero size

    // If we reach here, no suitable parent found, use viewport
    Some(viewport_size.unwrap_or(DEFAULT_VIEWPORT))
}

/// Helper function to check if an element is effectively visible (considering parent visibility)
/// Walks up the parent chain to ensure all ancestors are visible
fn is_effectively_visible(
    elements: &HashMap<UIElementID, UIElement>,
    element_id: UIElementID,
) -> bool {
    let mut current_id = element_id;
    let mut visited = std::collections::HashSet::new();
    const MAX_DEPTH: usize = 100; // Prevent infinite loops
    let mut depth = 0;

    loop {
        if depth > MAX_DEPTH {
            // Safety: prevent infinite loops if parent chain is broken
            eprintln!(
                "⚠️ is_effectively_visible: Max depth reached for element {}",
                element_id
            );
            return false;
        }
        depth += 1;

        // Prevent infinite loops
        if visited.contains(&current_id) {
            eprintln!(
                "⚠️ is_effectively_visible: Circular parent chain detected for element {}",
                element_id
            );
            return false;
        }
        visited.insert(current_id);

        if let Some(element) = elements.get(&current_id) {
            // If this element is not visible, return false
            if !element.get_visible() {
                // if is_file_tree {
                //     eprintln!("🌳 [visibility] FileTree parent chain broken: {} ({}) is not visible",
                //         element.get_name(), current_id);
                // }
                return false;
            }
            // Check parent
            let parent_id = element.get_parent();
            if parent_id.is_nil() {
                // Reached root, element is visible
                // if is_file_tree {
                //     eprintln!("🌳 [visibility] FileTree is effectively visible (reached root)");
                // }
                return true;
            }
            current_id = parent_id;
        } else {
            // Element not found, consider it invisible
            // eprintln!("⚠️ is_effectively_visible: Element {} not found in elements map", current_id);
            // if is_file_tree {
            //     eprintln!("🌳 [visibility] FileTree parent not found: {}", current_id);
            // }
            return false;
        }
    }
}

/// Calculate content size using pre-computed caches (FAST - no parent chain walks!)
fn calculate_content_size_with_visibility_cache(
    elements: &HashMap<UIElementID, UIElement>,
    parent_id: &UIElementID,
    visibility_cache: &HashSet<UIElementID>,
    percentage_ref_cache: &HashMap<UIElementID, Vector2>,
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
    let children_vec: Vec<&UIElementID> = children_ids.iter().collect();
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
                    .unwrap_or(DEFAULT_VIEWPORT);

                // Resolve percentages
                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    if pct >= 0.0 {
                        child_size.x =
                            (percentage_reference_size.x as f64 * (pct as f64 / 100.0)) as f32;
                    }
                }
                if let Some(&pct) = style_map.get("size.y") {
                    if pct >= 0.0 {
                        child_size.y =
                            (percentage_reference_size.y as f64 * (pct as f64 / 100.0)) as f32;
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

    // Calculate max width and height for all children
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

/// FIXED: Remove cache parameter to match working version
pub fn calculate_content_size(
    elements: &HashMap<UIElementID, UIElement>,
    parent_id: &UIElementID,
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
    let children_vec: Vec<&UIElementID> = children_ids.iter().collect();
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
                    find_percentage_reference_ancestor(elements, &child_id, None)
                        .unwrap_or(DEFAULT_VIEWPORT);

                // Resolve percentages using the smart reference (skip auto-sizing)
                // Use f64 for precision to avoid rounding errors
                let style_map = child.get_style_map();
                if let Some(&pct) = style_map.get("size.x") {
                    if pct >= 0.0 {
                        // Not auto-sizing, resolve percentage
                        child_size.x =
                            (percentage_reference_size.x as f64 * (pct as f64 / 100.0)) as f32;
                    }
                    // If pct < 0.0, it's auto-sizing - keep default size for now
                }
                if let Some(&pct) = style_map.get("size.y") {
                    if pct >= 0.0 {
                        // Not auto-sizing, resolve percentage
                        child_size.y =
                            (percentage_reference_size.y as f64 * (pct as f64 / 100.0)) as f32;
                    }
                    // If pct < 0.0, it's auto-sizing - keep default size for now
                }

                child_size
            })
        })
        .collect();

    // If all children are invisible, return 0 size (layout should collapse)
    if resolved_child_sizes.is_empty()
        || resolved_child_sizes
            .iter()
            .all(|s| s.x == 0.0 && s.y == 0.0)
    {
        return Vector2::new(0.0, 0.0);
    }

    // Calculate max width and height for all children
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

// Keep your cached version separate
pub fn calculate_content_size_smart_cached(
    elements: &HashMap<UIElementID, UIElement>,
    parent_id: &UIElementID,
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
        cache_ref.cache_results(*parent_id, signature, result);
    }
    result
}

pub fn calculate_layout_positions(
    _elements: &mut HashMap<UIElementID, UIElement>,
    _parent_id: &UIElementID,
) -> Vec<(UIElementID, Vector2)> {
    // No layout containers - return empty vec
    Vec::new()
}

/// Recursively calculate content sizes for all containers, starting from leaves
fn calculate_all_content_sizes(
    elements: &mut HashMap<UIElementID, UIElement>,
    current_id: &UIElementID,
) {
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
        let is_container = matches!(element, UIElement::Panel(_) | UIElement::Button(_));

        if is_container {
            let content_size = calculate_content_size(elements, current_id);
            if let Some(element) = elements.get_mut(current_id) {
                element.set_size(content_size);
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
        let initial_z_indices = ui_node.initial_z_indices.as_ref();
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
    elements: &HashMap<UIElementID, UIElement>,
    dirty_ids: Option<&HashSet<UIElementID>>,
) -> HashSet<UIElementID> {
    let Some(set) = dirty_ids else {
        return HashSet::new();
    };
    let mut to_process = HashSet::new();

    for &dirty_id in set {
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

/// Filtered version that skips unaffected branches for performance
pub fn update_global_transforms_with_layout_filtered(
    elements: &mut HashMap<UIElementID, UIElement>,
    current_id: &UIElementID,
    parent_global: &Transform2D,
    layout_positions: &HashMap<UIElementID, Vector2>,
    _parent_z: i32,
    initial_z_indices: Option<&HashMap<UIElementID, i32>>,
    affected_elements: &HashSet<UIElementID>,
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
    elements: &mut HashMap<UIElementID, UIElement>,
    current_id: &UIElementID,
    parent_global: &Transform2D,
    layout_positions: &HashMap<UIElementID, Vector2>,
    _parent_z: i32,
    initial_z_indices: Option<&HashMap<UIElementID, i32>>,
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
    elements: &mut HashMap<UIElementID, UIElement>,
    current_id: &UIElementID,
    parent_global: &Transform2D,
    layout_positions: &HashMap<UIElementID, Vector2>,
    _parent_z: i32,
    initial_z_indices: Option<&HashMap<UIElementID, i32>>,
    affected_elements: Option<&HashSet<UIElementID>>,
) {
    // Get parent info
    let (parent_size, parent_z) = {
        let parent_id = elements.get(current_id).map(|el| el.get_parent());

        if let Some(pid) = parent_id {
            if let Some(parent) = elements.get(&pid) {
                let size = *parent.get_size();
                (size, parent.get_z_index())
            } else {
                (DEFAULT_VIEWPORT, 0)
            }
        } else {
            (DEFAULT_VIEWPORT, 0)
        }
    };

    // Find the reference size for percentages
    let percentage_reference_size =
        find_percentage_reference_ancestor(elements, current_id, None).unwrap_or(DEFAULT_VIEWPORT);

    // Get element and calculate its global transform
    if let Some(element) = elements.get_mut(current_id) {
        // Clone local transform before mutable borrow
        let local = element.get_transform().clone();
        let mut size = *element.get_size();

        // Resolve percentage sizes
        let style_map = element.get_style_map();
        if let Some(&pct) = style_map.get("size.x") {
            if pct >= 0.0 {
                size.x = (percentage_reference_size.x as f64 * (pct as f64 / 100.0)) as f32;
            }
        }
        if let Some(&pct) = style_map.get("size.y") {
            if pct >= 0.0 {
                size.y = (percentage_reference_size.y as f64 * (pct as f64 / 100.0)) as f32;
            }
        }

        // Apply size
        element.set_size(size);

        // Calculate global transform
        let mut global = Transform2D::default();
        global.scale.x = parent_global.scale.x * local.scale.x;
        global.scale.y = parent_global.scale.y * local.scale.y;
        global.position.x = parent_global.position.x + (local.position.x * parent_global.scale.x);
        global.position.y = parent_global.position.y + (local.position.y * parent_global.scale.y);
        global.rotation = parent_global.rotation + local.rotation;

        // Calculate anchor offset
        let anchor = *element.get_anchor();
        let pivot = *element.get_pivot();
        let anchor_offset = calculate_anchor_offset(size, pivot, anchor, parent_size);

        // Apply anchor offset
        global.position.x += anchor_offset.x * parent_global.scale.x;
        global.position.y += anchor_offset.y * parent_global.scale.y;

        // Calculate z-index
        let base_z = initial_z_indices
            .and_then(|m| m.get(current_id))
            .copied()
            .unwrap_or(0);
        let global_z = (base_z + parent_z).min(1000000);

        // Apply global transform
        element.set_global_transform(global);
        element.set_z_index(global_z);

        // Get children list before dropping the mutable borrow
        let children_ids = element.get_children().to_vec();

        // No child element handling needed - Text and Panel are simple elements
        // Recurse into children
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
                layout_positions,
                global_z,
                initial_z_indices,
                affected_elements,
            );
        }
    }
}

fn calculate_anchor_offset(
    child_size: Vector2,
    child_pivot: Vector2,
    anchor: FurAnchor,
    parent_size: Vector2,
) -> Vector2 {
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
            Vector2::new(0.0, offset_y) // Center horizontally
        }
        FurAnchor::Bottom => {
            let parent_bottom = -parent_size.y * 0.5;
            let offset_y = parent_bottom + child_size.y * (1.0 - child_pivot.y);
            Vector2::new(0.0, offset_y) // Center horizontally
        }
        FurAnchor::Left => {
            let parent_left = -parent_size.x * 0.5;
            let offset_x = parent_left + child_size.x * (1.0 - child_pivot.x);
            Vector2::new(offset_x, 0.0) // Center vertically
        }
        FurAnchor::Right => {
            let parent_right = parent_size.x * 0.5;
            let offset_x = parent_right - child_size.x * child_pivot.x;
            Vector2::new(offset_x, 0.0) // Center vertically
        }
        FurAnchor::Center => Vector2::new(0.0, 0.0),
    }
}

pub fn update_ui_layout_cached_optimized(ui_node: &mut UINode, cache: &RwLock<LayoutCache>) {
    if let (Some(root_ids), Some(elements)) = (&ui_node.root_ids, &mut ui_node.elements) {
        // Build visibility cache once at the start (no parent chain walks!)
        let visibility_cache: HashSet<UIElementID> = elements
            .keys()
            .copied()
            .filter(|&id| is_effectively_visible(elements, id))
            .collect();

        // Collect dirty elements + ancestors (parents up to root)
        let dirty_with_ancestors =
            collect_dirty_with_ancestors(elements, ui_node.needs_layout_recalc.as_ref());

        // Clear cache only for elements we're recalculating
        if let Ok(mut cache_ref) = cache.write() {
            for dirty_id in &dirty_with_ancestors {
                cache_ref.entries.remove(dirty_id);
            }
        }

        // Recalculate content sizes only for dirty elements (bottom-up)
        // We need to process them in order: children before parents
        let mut sorted_dirty: Vec<UIElementID> = dirty_with_ancestors.iter().copied().collect();
        // Sort by depth (deeper elements first) by counting ancestors
        sorted_dirty.sort_by_key(|id| {
            let mut depth = 0;
            let mut current = *id;
            while let Some(el) = elements.get(&current) {
                let parent = el.get_parent();
                if parent.is_nil() {
                    break;
                }
                depth += 1;
                current = parent;
                if depth > 100 {
                    break;
                } // Safety
            }
            -(depth as i32) // Negative so deeper elements come first
        });

        // Recalculate content sizes for dirty elements only
        for element_id in sorted_dirty {
            if let Some(element) = elements.get(&element_id) {
                let is_container = matches!(element, UIElement::Panel(_) | UIElement::Button(_));

                if is_container {
                    // Use cached visibility AND percentage refs (no parent chain walks!)
                    let content_size = calculate_content_size_with_visibility_cache(
                        elements,
                        &element_id,
                        &visibility_cache,
                        &HashMap::new(),
                    );
                    if let Some(element) = elements.get_mut(&element_id) {
                        element.set_size(content_size);
                    }
                }
            }
        }

        let empty_layout_map = HashMap::new();
        let initial_z_indices = ui_node.initial_z_indices.as_ref();
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

// Updated render function with caching and dirty element optimization
pub fn render_ui(
    ui_node: &mut UINode,
    gfx: &mut Graphics,
    provider: Option<&dyn crate::script::ScriptProvider>,
) {
    let cache = get_layout_cache();

    // Get timestamp from UINode's base node
    let timestamp = ui_node.base.created_timestamp;

    // Check if fur_path has changed or if elements need to be loaded
    // Extract fur_path string first to avoid borrow conflicts
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
                        // Always rebuild UI elements, even if empty (this clears existing elements)
                        // build_ui_elements_from_fur calls elements.clear() which removes all old elements
                        build_ui_elements_from_fur(ui_node, &fur_elements);
                        ui_node.loaded_fur_path =
                            Some(std::borrow::Cow::Owned(fur_path_str.clone()));
                    }
                    Err(e) => {
                        eprintln!("⚠️ Failed to load FUR file {}: {}", fur_path_str, e);
                        // On parse error, clear all elements to prevent stale UI
                        // Call build_ui_elements_from_fur with empty array to trigger cleanup
                        build_ui_elements_from_fur(ui_node, &[]);
                    }
                }
            }
        }
    }

    // Check if elements exist - if not, we can't render yet
    let elements_exist = ui_node.elements.is_some();

    // If elements don't exist yet, return early - elements will be marked when FUR loads
    // The UINode will be re-added to scene's needs_rerender after FUR loads
    if !elements_exist {
        return;
    }

    // Check if we have any dirty elements
    // Only mark all on FIRST render (when elements just loaded but nothing marked yet)
    // Don't do this every frame when HashSet is empty after clearing!
    let needs_rerender_empty = ui_node
        .needs_rerender
        .as_ref()
        .map_or(true, |s| s.is_empty());
    let needs_layout_recalc_empty = ui_node
        .needs_layout_recalc
        .as_ref()
        .map_or(true, |s| s.is_empty());
    if needs_rerender_empty && needs_layout_recalc_empty {
        // If both are empty, nothing needs updating - return early
        return;
    }

    // If needs_rerender is empty but layout needs recalc, mark affected elements
    if needs_rerender_empty && !needs_layout_recalc_empty {
        // Layout changed but no elements marked - mark elements that need layout
        if let Some(layout_set) = ui_node.needs_layout_recalc.as_ref() {
            ui_node
                .needs_rerender
                .get_or_insert_with(HashSet::new)
                .extend(layout_set.iter().copied());
        }
    }

    // Collect dirty element IDs before borrowing elements
    let dirty_elements: Vec<UIElementID> = ui_node
        .needs_rerender
        .as_ref()
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    let needs_layout = !needs_layout_recalc_empty;

    // Build visibility cache ONCE for the frame (used by both visibility check and layout)
    let visibility_cache: HashSet<UIElementID> = if let Some(elements) = &ui_node.elements {
        let elements_ref: &HashMap<UIElementID, UIElement> = elements;
        elements_ref
            .iter()
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

    // Build percentage reference cache ONCE (use virtual size from Graphics for viewport)
    let viewport_size = Vector2::new(gfx.virtual_width, gfx.virtual_height);
    let percentage_ref_cache: HashMap<UIElementID, Vector2> =
        if let Some(elements) = &ui_node.elements {
            let elements_ref: &HashMap<UIElementID, UIElement> = elements;
            elements_ref
                .iter()
                .map(|(id, _)| {
                    let ref_size =
                        find_percentage_reference_ancestor(elements_ref, id, Some(viewport_size))
                            .unwrap_or(viewport_size);
                    (*id, ref_size)
                })
                .collect()
        } else {
            HashMap::new()
        };

    // Only recalculate layout if layout actually changed (not just visual state like button hover)
    if needs_layout {
        // OPTIMIZATION: Only recalculate dirty elements and their ancestors
        // Don't recalculate the entire tree!
        if let Some(elements) = &mut ui_node.elements {
            // Collect dirty elements + ancestors (parents up to root)
            let dirty_with_ancestors =
                collect_dirty_with_ancestors(elements, ui_node.needs_layout_recalc.as_ref());

            // Clear cache only for elements we're recalculating
            if let Ok(mut cache_ref) = cache.write() {
                for dirty_id in &dirty_with_ancestors {
                    cache_ref.entries.remove(dirty_id);
                }
            }

            // Recalculate content sizes only for dirty elements (bottom-up)
            // We need to process them in order: children before parents
            let mut sorted_dirty: Vec<UIElementID> = dirty_with_ancestors.iter().copied().collect();
            // Sort by depth (deeper elements first) by counting ancestors
            sorted_dirty.sort_by_key(|id| {
                let mut depth = 0;
                let mut current = *id;
                while let Some(el) = elements.get(&current) {
                    let parent = el.get_parent();
                    if parent.is_nil() {
                        break;
                    }
                    depth += 1;
                    current = parent;
                    if depth > 100 {
                        break;
                    } // Safety
                }
                -(depth as i32) // Negative so deeper elements come first
            });

            // Recalculate content sizes for dirty elements only
            // (visibility_cache was already built at the start of the function)
            for element_id in sorted_dirty {
                if let Some(element) = elements.get(&element_id) {
                    let is_container = matches!(element, UIElement::Panel(_) | UIElement::Button(_));

                    if is_container {
                        // Use cached visibility AND percentage refs (no parent chain walks!)
                        let content_size = calculate_content_size_with_visibility_cache(
                            elements,
                            &element_id,
                            &visibility_cache,
                            &percentage_ref_cache,
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
            let initial_z_indices = ui_node.initial_z_indices.as_ref();

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
    }

    // Now borrow elements for rendering
    if let Some(elements) = &mut ui_node.elements {
        // OPTIMIZATION: Only check dirty elements for visibility changes
        // instead of checking ALL elements every frame
        let dirty_set: HashSet<UIElementID> = dirty_elements.iter().copied().collect();

        // Collect elements that need visibility checking (dirty elements + their descendants)
        let elements_to_check: HashSet<UIElementID> = if dirty_set.is_empty() {
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
        let (visible_element_ids, newly_invisible_ids): (Vec<UIElementID>, Vec<UIElementID>) = {
            let visible: Vec<UIElementID> = visibility_cache.iter().copied().collect();
            let mut invisible = Vec::new();

            // Find elements that JUST became invisible (were in dirty set but not visible)
            for id in &elements_to_check {
                if !visibility_cache.contains(id) {
                    invisible.push(*id);
                }
            }

            (visible, invisible)
        };

        // Process elements marked for deletion first
        // These are explicitly marked for deletion and should be removed from primitive renderer and map
        let pending_empty = ui_node
            .pending_deletion
            .as_ref()
            .map_or(true, |s| s.is_empty());
        if !pending_empty {
            let pending_deletion_set = ui_node.pending_deletion.as_ref().unwrap().clone();

            // Collect elements to remove by iterating over the map
            let mut to_remove: Vec<(UIElementID, UIElement)> = Vec::new();
            for (id, element) in elements.iter() {
                // Check if this ID is in pending_deletion_set by comparing directly
                if pending_deletion_set
                    .iter()
                    .any(|&pending_id| pending_id == *id)
                {
                    to_remove.push((*id, element.clone()));
                }
            }

            // Remove from primitive renderer cache
            for (element_id, element) in &to_remove {
                match element {
                    UIElement::Panel(_) | UIElement::Button(_) => {
                        gfx.renderer_ui
                            .remove_panel(&mut gfx.renderer_prim, *element_id);
                    }
                    UIElement::Text(_) => {
                        gfx.renderer_ui
                            .remove_text(&mut gfx.renderer_prim, *element_id);
                    }
                }
            }

            // Remove from elements map using retain
            let ids_to_remove: HashSet<UIElementID> = to_remove.iter().map(|(id, _)| *id).collect();
            elements.retain(|id, _| !ids_to_remove.contains(id));

            // Clean up other data structures
            for (element_id, _) in to_remove {
                if let Some(ref mut set) = ui_node.needs_rerender {
                    set.remove(&element_id);
                }
                if let Some(ref mut set) = ui_node.needs_layout_recalc {
                    set.remove(&element_id);
                }
                if let Some(ref mut map) = ui_node.initial_z_indices {
                    map.remove(&element_id);
                }
                if let Some(root_ids) = &mut ui_node.root_ids {
                    root_ids.retain(|id| id != &element_id);
                }
            }

            // Clear the pending_deletion set
            if let Some(ref mut set) = ui_node.pending_deletion {
                set.clear();
            }
        }

        // OPTIMIZATION: Only remove newly invisible elements from primitive renderer cache
        // (not from the elements map - invisible elements should stay in the map so they can become visible again)
        // This avoids iterating through everything every frame
        // Iterate over elements map to update slots
        let newly_invisible_set: HashSet<UIElementID> =
            newly_invisible_ids.iter().copied().collect();
        for (element_id, element) in elements.iter() {
            if newly_invisible_set.contains(element_id) {
                match element {
                    UIElement::Panel(_) | UIElement::Button(_) => {
                        gfx.renderer_ui
                            .remove_panel(&mut gfx.renderer_prim, *element_id);
                    }
                    UIElement::Text(_) => {
                        gfx.renderer_ui
                            .remove_text(&mut gfx.renderer_prim, *element_id);
                    }
                }
            }
        }

        // Now render only the visible elements (mutable borrow)
        for element_id in visible_element_ids {
            if let Some(element) = elements.get_mut(&element_id) {
                match element {
                    UIElement::Panel(panel) => render_panel(panel, gfx, timestamp),
                    UIElement::Button(button) => render_button(button, gfx, timestamp),
                    UIElement::Text(text) => render_text(text, gfx, timestamp),
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
    if !transform.position.x.is_finite()
        || !transform.position.y.is_finite()
        || !transform.scale.x.is_finite()
        || !transform.scale.y.is_finite()
        || !transform.rotation.is_finite()
    {
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
            let border_id = UIElementID::from_string(&format!("{}-border", bg_id.to_string()));
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
#[allow(deprecated)] // Font::from_name deprecated in favor of TextRenderer; UI still uses FontAtlas path
pub fn calculate_character_positions(text: &str, font_size: f32) -> Vec<f32> {
    use fontdue::Font as Fontdue;
    use fontdue::FontSettings;

    const DESIGN_SIZE: f32 = 192.0; // Must match the font atlas design size!

    if let Some(font) = Font::from_name("NotoSans", Weight::Regular, Style::Normal) {
        let fd_font =
            Fontdue::from_bytes(font.data(), FontSettings::default()).expect("Invalid font data");

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

/// Render button as a panel (same primitive path); label is not drawn in primitive renderer.
fn render_button(button: &UIButton, gfx: &mut Graphics, timestamp: u64) {
    let panel = UIPanel {
        base: button.base.clone(),
        props: Default::default(),
    };
    render_panel(&panel, gfx, timestamp);
}

// Optimized text rendering - only regenerate atlas when font properties change
#[allow(deprecated)] // Font::from_name deprecated in favor of TextRenderer; UI still uses FontAtlas path
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
        crate::ui_elements::ui_text::TextFlow::Start => {
            crate::ui_elements::ui_text::TextAlignment::Left
        } // Start = Left (text starts at anchor, flows right)
        crate::ui_elements::ui_text::TextFlow::Center => {
            crate::ui_elements::ui_text::TextAlignment::Center
        }
        crate::ui_elements::ui_text::TextFlow::End => {
            crate::ui_elements::ui_text::TextAlignment::Right
        } // End = Right (text ends at anchor, flows left)
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
        None, // font_spec
        &gfx.device,
        &gfx.queue,
    );
}
