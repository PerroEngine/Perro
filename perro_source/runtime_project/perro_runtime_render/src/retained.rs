use ahash::{AHashMap, AHashSet};
use perro_ids::{MeshID, NodeID, SignalID, TextureID};
use perro_render_bridge::{
    AmbientLight3DState, Camera2DState, Camera3DState, Decal3DState, DenseInstancePose3D,
    LODOptions3D, Material3D, MeshBlendOptions3D, MeshSurfaceBinding3D, PointLight3DState,
    RayLight3DState, RenderEvent, RenderRequestID, SkeletonPalette, Sky3DState, SpotLight3DState,
    Sprite2DCommand, UiCommand, UiRectState,
};
use perro_structs::Vector2;
use perro_ui::{ComputedUiRect, UiSizeMode, UiVector2};
use std::{cell::RefCell, collections::VecDeque, sync::Arc, sync::mpsc};

pub fn sprite_2d_texture_request(node: NodeID) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 8) | 0x2D)
}

pub fn tilemap_2d_texture_request(node: NodeID) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 8) | 0x71)
}

pub fn mesh_3d_request(node: NodeID) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 8) | 0x3E)
}

pub fn material_3d_request(node: NodeID, surface_index: u32) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 16) | ((surface_index as u64) << 8) | 0x3F)
}

pub fn ui_image_texture_request(node: NodeID) -> RenderRequestID {
    RenderRequestID::new((node.as_u64() << 8) | 0xE9)
}

pub fn decode_3d_mesh_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x3E {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

pub fn decode_2d_texture_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x2D {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

pub fn decode_tilemap_2d_texture_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x71 {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

pub fn decode_ui_image_texture_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0xE9 {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 8))
}

pub fn decode_3d_material_request_node(request: RenderRequestID) -> Option<NodeID> {
    if (request.0 & 0xFF) != 0x3F {
        return None;
    }
    Some(NodeID::from_u64(request.0 >> 16))
}

pub fn decode_render_request_node(request: RenderRequestID) -> Option<NodeID> {
    decode_2d_texture_request_node(request)
        .or_else(|| decode_tilemap_2d_texture_request_node(request))
        .or_else(|| decode_ui_image_texture_request_node(request))
        .or_else(|| decode_3d_mesh_request_node(request))
        .or_else(|| decode_3d_material_request_node(request))
}

pub fn decode_render_request_node_from_event(event: &RenderEvent) -> Option<NodeID> {
    let request = match event {
        RenderEvent::MeshCreated { request, .. }
        | RenderEvent::TextureCreated { request, .. }
        | RenderEvent::MaterialCreated { request, .. }
        | RenderEvent::Failed { request, .. } => *request,
        RenderEvent::TextureLoaded { .. }
        | RenderEvent::TextureTexelsUpdated { .. }
        | RenderEvent::MaterialLoaded { .. }
        | RenderEvent::MeshDropped { .. }
        | RenderEvent::TextureDropped { .. }
        | RenderEvent::MaterialDropped { .. }
        | RenderEvent::WaterSamples { .. }
        | RenderEvent::WaterBodySamples { .. } => return None,
    };
    decode_render_request_node(request)
}

fn collect_tree_traversal<I, A, F>(
    traversal_ids: &mut Vec<NodeID>,
    child_scratch: &mut Vec<NodeID>,
    seed_ids: I,
    all_ids: A,
    include_all: bool,
    children_of: F,
) where
    I: IntoIterator<Item = NodeID>,
    A: IntoIterator<Item = NodeID>,
    F: FnMut(NodeID, &mut Vec<NodeID>),
{
    let mut seen = AHashSet::<NodeID>::default();
    collect_tree_traversal_with_seen(
        traversal_ids,
        &mut seen,
        child_scratch,
        seed_ids,
        all_ids,
        include_all,
        children_of,
    );
}

fn collect_tree_traversal_with_seen<I, A, F>(
    traversal_ids: &mut Vec<NodeID>,
    seen: &mut AHashSet<NodeID>,
    child_scratch: &mut Vec<NodeID>,
    seed_ids: I,
    all_ids: A,
    include_all: bool,
    mut children_of: F,
) where
    I: IntoIterator<Item = NodeID>,
    A: IntoIterator<Item = NodeID>,
    F: FnMut(NodeID, &mut Vec<NodeID>),
{
    traversal_ids.clear();
    seen.clear();
    for id in seed_ids {
        if seen.insert(id) {
            traversal_ids.push(id);
        }
    }
    if include_all {
        for id in all_ids {
            if seen.insert(id) {
                traversal_ids.push(id);
            }
        }
    }

    child_scratch.clear();
    let mut traversal_cursor = 0usize;
    while traversal_cursor < traversal_ids.len() {
        let node = traversal_ids[traversal_cursor];
        traversal_cursor += 1;
        child_scratch.clear();
        children_of(node, child_scratch);
        for child in child_scratch.iter().copied() {
            if seen.insert(child) {
                traversal_ids.push(child);
            }
        }
    }
}

pub struct Render2DState {
    pub traversal_ids: Vec<NodeID>,
    pub traversal_seen: AHashSet<NodeID>,
    pub traversal_child_scratch: Vec<NodeID>,
    pub dirty_ids_scratch: Vec<NodeID>,
    pub visible_now: AHashSet<NodeID>,
    pub prev_visible: AHashSet<NodeID>,
    pub retained_sprites: AHashMap<NodeID, Sprite2DCommand>,
    pub particle_path_cache: AHashMap<u64, perro_render_bridge::ParticleProfile2D>,
    pub particle_path_cache_order: VecDeque<u64>,
    pub pending_particle_path_loads: AHashSet<u64>,
    pub particle_path_load_tx:
        mpsc::Sender<(String, Option<perro_render_bridge::ParticleProfile2D>)>,
    pub particle_path_load_rx:
        mpsc::Receiver<(String, Option<perro_render_bridge::ParticleProfile2D>)>,
    pub tileset_cache: AHashMap<u64, std::sync::Arc<perro_render_bridge::TileSet2D>>,
    pub pending_tileset_loads: AHashSet<u64>,
    pub tileset_load_tx: mpsc::Sender<(u64, Option<perro_render_bridge::TileSet2D>)>,
    pub tileset_load_rx: mpsc::Receiver<(u64, Option<perro_render_bridge::TileSet2D>)>,
    pub texture_sources: AHashMap<NodeID, String>,
    pub last_camera: Option<Camera2DState>,
    pub last_button_pointer: Option<(Vector2, bool)>,
    pub removed_nodes: Vec<NodeID>,
    pub force_full_scan_once: bool,
}

pub type Render2dSystem = Render2DState;

impl Render2DState {
    pub fn new() -> Self {
        let (particle_path_load_tx, particle_path_load_rx) = mpsc::channel();
        let (tileset_load_tx, tileset_load_rx) = mpsc::channel();
        Self {
            traversal_ids: Vec::new(),
            traversal_seen: AHashSet::default(),
            traversal_child_scratch: Vec::new(),
            dirty_ids_scratch: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            retained_sprites: AHashMap::default(),
            particle_path_cache: AHashMap::default(),
            particle_path_cache_order: VecDeque::new(),
            pending_particle_path_loads: AHashSet::default(),
            particle_path_load_tx,
            particle_path_load_rx,
            tileset_cache: AHashMap::default(),
            pending_tileset_loads: AHashSet::default(),
            tileset_load_tx,
            tileset_load_rx,
            texture_sources: AHashMap::default(),
            last_camera: None,
            last_button_pointer: None,
            removed_nodes: Vec::new(),
            force_full_scan_once: false,
        }
    }

    pub fn note_removed_node(&mut self, node: NodeID) {
        self.removed_nodes.push(node);
    }

    pub fn request_full_scan_once(&mut self) {
        self.force_full_scan_once = true;
    }

    pub fn full_scan_pending(&self) -> bool {
        self.force_full_scan_once
    }

    pub fn clear_full_scan_pending(&mut self) {
        self.force_full_scan_once = false;
    }

    pub fn collect_traversal<I, A, F>(
        &mut self,
        dirty_ids: I,
        all_ids: A,
        bootstrap_scan: bool,
        children_of: F,
    ) -> Vec<NodeID>
    where
        I: IntoIterator<Item = NodeID>,
        A: IntoIterator<Item = NodeID>,
        F: FnMut(NodeID, &mut Vec<NodeID>),
    {
        let include_all = self.force_full_scan_once || bootstrap_scan;
        self.force_full_scan_once = false;
        let mut traversal_ids = std::mem::take(&mut self.traversal_ids);
        let mut seen = std::mem::take(&mut self.traversal_seen);
        let mut child_scratch = std::mem::take(&mut self.traversal_child_scratch);
        collect_tree_traversal_with_seen(
            &mut traversal_ids,
            &mut seen,
            &mut child_scratch,
            dirty_ids,
            all_ids,
            include_all,
            children_of,
        );
        child_scratch.clear();
        seen.clear();
        self.traversal_child_scratch = child_scratch;
        self.traversal_seen = seen;
        traversal_ids
    }

    pub fn take_dirty_ids_scratch(&mut self) -> Vec<NodeID> {
        std::mem::take(&mut self.dirty_ids_scratch)
    }

    pub fn restore_dirty_ids_scratch(&mut self, mut dirty_ids: Vec<NodeID>) {
        dirty_ids.clear();
        self.dirty_ids_scratch = dirty_ids;
    }

    pub fn collect_traversal_with_scratch<A, F>(
        &mut self,
        dirty_ids: &[NodeID],
        all_ids: A,
        bootstrap_scan: bool,
        children_of: F,
    ) -> Vec<NodeID>
    where
        A: IntoIterator<Item = NodeID>,
        F: FnMut(NodeID, &mut Vec<NodeID>),
    {
        let include_all = self.force_full_scan_once || bootstrap_scan;
        self.force_full_scan_once = false;
        let mut traversal_ids = std::mem::take(&mut self.traversal_ids);
        let mut seen = std::mem::take(&mut self.traversal_seen);
        let mut child_scratch = std::mem::take(&mut self.traversal_child_scratch);
        collect_tree_traversal_with_seen(
            &mut traversal_ids,
            &mut seen,
            &mut child_scratch,
            dirty_ids.iter().copied(),
            all_ids,
            include_all,
            children_of,
        );
        child_scratch.clear();
        seen.clear();
        self.traversal_child_scratch = child_scratch;
        self.traversal_seen = seen;
        traversal_ids
    }

    pub fn begin_visible_pass(&mut self) -> AHashSet<NodeID> {
        let mut visible_now = std::mem::take(&mut self.visible_now);
        visible_now.clear();
        visible_now.extend(self.prev_visible.iter().copied());
        for node in self.removed_nodes.drain(..) {
            visible_now.remove(&node);
        }
        visible_now
    }

    pub fn collect_removed_visible_nodes(&self, visible_now: &AHashSet<NodeID>) -> Vec<NodeID> {
        self.prev_visible
            .iter()
            .copied()
            .filter(|node| !visible_now.contains(node))
            .collect()
    }

    pub fn finish_visible_pass(
        &mut self,
        mut traversal_ids: Vec<NodeID>,
        mut visible_now: AHashSet<NodeID>,
    ) {
        std::mem::swap(&mut self.prev_visible, &mut visible_now);
        visible_now.clear();
        self.visible_now = visible_now;
        traversal_ids.clear();
        self.traversal_ids = traversal_ids;
    }
}

impl Default for Render2DState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct RenderUiState {
    pub traversal_ids: Vec<NodeID>,
    pub traversal_seen: AHashSet<NodeID>,
    pub command_ids: Vec<NodeID>,
    pub command_seen: AHashSet<NodeID>,
    pub dirty_entries_scratch: Vec<(NodeID, u16)>,
    pub all_ids_scratch: Vec<NodeID>,
    pub parent_siblings_scratch: AHashMap<NodeID, Vec<NodeID>>,
    pub traversal_child_scratch: Vec<NodeID>,
    pub visible_now: AHashSet<NodeID>,
    pub prev_visible: AHashSet<NodeID>,
    pub computed_rects: AHashMap<NodeID, ComputedUiRect>,
    pub size_clamp_baselines: RefCell<AHashMap<NodeID, UiSizeClampBaseline>>,
    pub computed_scales: AHashMap<NodeID, Vector2>,
    pub auto_layout_computed: AHashSet<NodeID>,
    pub retained_commands: AHashMap<NodeID, UiCommand>,
    pub retained_rects: AHashMap<NodeID, UiRectState>,
    pub button_states: AHashMap<NodeID, UiButtonVisualState>,
    pub interactive_scan_seen: AHashSet<NodeID>,
    pub visible_buttons: Vec<NodeID>,
    pub visible_text_edits: Vec<NodeID>,
    pub focusable_nodes: Vec<NodeID>,
    pub hovered_text_edit: Option<NodeID>,
    pub focused_ui_node: Option<NodeID>,
    pub nav_pressed_button: Option<NodeID>,
    pub ui_nav_repeat_dir: Option<[i8; 2]>,
    pub ui_nav_repeat_timer: f32,
    pub focused_text_edit: Option<NodeID>,
    pub pressed_text_edit: Option<NodeID>,
    pub pressed_ui_button: Option<NodeID>,
    pub active_scrollbar: Option<NodeID>,
    pub scrollbar_drag_offset: f32,
    pub text_edit_repeat_key: Option<perro_input_api::KeyCode>,
    pub text_edit_repeat_timer: f32,
    pub last_ui_pointer: Option<(Vector2, bool)>,
    pub pointer_screen_point: Option<Vector2>,
    pub cursor_icon: perro_ui::CursorIcon,
    pub cursor_icon_2d: perro_ui::CursorIcon,
    pub cursor_icon_ui: perro_ui::CursorIcon,
    pub cursor_icon_script: perro_ui::CursorIcon,
    pub removed_nodes: Vec<NodeID>,
    pub event_signal_scratch: Vec<SignalID>,
    pub event_signal_name_scratch: String,
    /// Dirty marks made during the post-layout input phase of extraction
    /// (clicks opening dropdowns, toggling tree rows). The frame-end dirty
    /// clear would drop them before the next layout pass, so they are
    /// re-applied after the clear.
    pub deferred_dirty: Vec<(NodeID, u16)>,
    pub defer_dirty_marks: bool,
}

pub type RenderUiSystem = RenderUiState;

#[derive(Clone, Copy, Debug)]
pub struct UiDirtyMask {
    pub layout_mask: u16,
    pub layout_parent: u16,
    pub commands: u16,
    pub default_flags: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct UiExtractionOptions {
    pub mask: UiDirtyMask,
    pub bootstrap_scan: bool,
    pub input_changed: bool,
}

pub struct UiExtractionPlan {
    pub traversal_ids: Vec<NodeID>,
    pub command_ids: Vec<NodeID>,
    pub command_seen: AHashSet<NodeID>,
    pub dirty_nodes: u32,
    pub affected_nodes: u32,
}

impl RenderUiState {
    pub fn new() -> Self {
        Self {
            traversal_ids: Vec::new(),
            traversal_seen: AHashSet::default(),
            command_ids: Vec::new(),
            command_seen: AHashSet::default(),
            dirty_entries_scratch: Vec::new(),
            all_ids_scratch: Vec::new(),
            parent_siblings_scratch: AHashMap::default(),
            traversal_child_scratch: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            computed_rects: AHashMap::default(),
            size_clamp_baselines: RefCell::new(AHashMap::default()),
            computed_scales: AHashMap::default(),
            auto_layout_computed: AHashSet::default(),
            retained_commands: AHashMap::default(),
            retained_rects: AHashMap::default(),
            button_states: AHashMap::default(),
            interactive_scan_seen: AHashSet::default(),
            visible_buttons: Vec::new(),
            visible_text_edits: Vec::new(),
            focusable_nodes: Vec::new(),
            hovered_text_edit: None,
            focused_ui_node: None,
            nav_pressed_button: None,
            ui_nav_repeat_dir: None,
            ui_nav_repeat_timer: 0.0,
            focused_text_edit: None,
            pressed_text_edit: None,
            pressed_ui_button: None,
            active_scrollbar: None,
            scrollbar_drag_offset: 0.0,
            text_edit_repeat_key: None,
            text_edit_repeat_timer: 0.0,
            last_ui_pointer: None,
            pointer_screen_point: None,
            cursor_icon: perro_ui::CursorIcon::Default,
            cursor_icon_2d: perro_ui::CursorIcon::Default,
            cursor_icon_ui: perro_ui::CursorIcon::Default,
            cursor_icon_script: perro_ui::CursorIcon::Default,
            removed_nodes: Vec::new(),
            event_signal_scratch: Vec::new(),
            event_signal_name_scratch: String::new(),
            deferred_dirty: Vec::new(),
            defer_dirty_marks: false,
        }
    }

    pub fn note_removed_node(&mut self, node: NodeID) {
        self.removed_nodes.push(node);
    }

    pub fn collect_extraction_plan<I, A, FP, FC>(
        &mut self,
        dirty_entries: I,
        all_ids: A,
        options: UiExtractionOptions,
        mut parent_layout_siblings: FP,
        mut children_of: FC,
    ) -> UiExtractionPlan
    where
        I: IntoIterator<Item = (NodeID, u16)>,
        A: IntoIterator<Item = NodeID>,
        FP: FnMut(NodeID, &mut Vec<NodeID>),
        FC: FnMut(NodeID, &mut Vec<NodeID>),
    {
        let mut traversal_ids = std::mem::take(&mut self.traversal_ids);
        let mut traversal_seen = std::mem::take(&mut self.traversal_seen);
        let mut command_ids = std::mem::take(&mut self.command_ids);
        let mut command_seen = std::mem::take(&mut self.command_seen);
        traversal_ids.clear();
        traversal_seen.clear();
        command_ids.clear();
        command_seen.clear();

        let mask = options.mask;
        let mut dirty_count = 0u32;
        for (node, mut flags) in dirty_entries {
            dirty_count = dirty_count.saturating_add(1);
            if flags == 0 {
                flags = mask.default_flags;
            }
            if (flags & mask.layout_mask) != 0 && traversal_seen.insert(node) {
                traversal_ids.push(node);
            }
            if (flags & mask.commands) != 0 && command_seen.insert(node) {
                command_ids.push(node);
            }
            if (flags & mask.layout_parent) != 0 {
                let mut sibling_scratch = std::mem::take(&mut self.traversal_child_scratch);
                sibling_scratch.clear();
                parent_layout_siblings(node, &mut sibling_scratch);
                for sibling in sibling_scratch.iter().copied() {
                    if traversal_seen.insert(sibling) {
                        traversal_ids.push(sibling);
                    }
                    if command_seen.insert(sibling) {
                        command_ids.push(sibling);
                    }
                }
                sibling_scratch.clear();
                self.traversal_child_scratch = sibling_scratch;
            }
        }

        if traversal_ids.is_empty() && options.bootstrap_scan {
            for id in all_ids {
                traversal_ids.push(id);
            }
        }
        traversal_seen.extend(traversal_ids.iter().copied());

        let mut child_scratch = std::mem::take(&mut self.traversal_child_scratch);
        let mut traversal_cursor = 0usize;
        while traversal_cursor < traversal_ids.len() {
            let node = traversal_ids[traversal_cursor];
            traversal_cursor += 1;
            child_scratch.clear();
            children_of(node, &mut child_scratch);
            for child in child_scratch.iter().copied() {
                if traversal_seen.insert(child) {
                    traversal_ids.push(child);
                }
            }
        }
        child_scratch.clear();
        self.traversal_child_scratch = child_scratch;

        for &node in &traversal_ids {
            if command_seen.insert(node) {
                command_ids.push(node);
            }
        }
        if options.input_changed || options.bootstrap_scan {
            for node in self.retained_commands.keys().copied() {
                if command_seen.insert(node) {
                    command_ids.push(node);
                }
            }
        }

        let affected_nodes = traversal_ids.len().min(u32::MAX as usize) as u32;
        traversal_seen.clear();
        self.traversal_seen = traversal_seen;

        UiExtractionPlan {
            traversal_ids,
            command_ids,
            command_seen,
            dirty_nodes: dirty_count,
            affected_nodes,
        }
    }

    pub fn restore_extraction_plan(
        &mut self,
        mut traversal_ids: Vec<NodeID>,
        mut command_ids: Vec<NodeID>,
        mut command_seen: AHashSet<NodeID>,
    ) {
        traversal_ids.clear();
        command_ids.clear();
        command_seen.clear();
        self.traversal_ids = traversal_ids;
        self.command_ids = command_ids;
        self.command_seen = command_seen;
    }
}

impl Default for RenderUiState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocaleTextBinding {
    pub node: NodeID,
    pub field: LocaleTextField,
    pub key: String,
    pub key_hash: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocaleTextField {
    LabelText,
    TextEditText,
    TextEditPlaceholder,
}

pub struct LocaleTextState {
    pub bindings: Vec<LocaleTextBinding>,
    pub last_epoch: u64,
}

impl LocaleTextState {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            last_epoch: 0,
        }
    }

    pub fn remove_node_bindings(&mut self, node: NodeID) {
        self.bindings.retain(|binding| binding.node != node);
    }
}

impl Default for LocaleTextState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiButtonVisualState {
    #[default]
    Neutral,
    Hover,
    Pressed,
}

#[derive(Clone, Copy)]
pub struct UiSizeClampBaseline {
    pub size: Vector2,
    pub size_def: UiVector2,
    pub h_mode: UiSizeMode,
    pub v_mode: UiSizeMode,
}

pub struct Render3DState {
    pub traversal_ids: Vec<NodeID>,
    pub traversal_child_scratch: Vec<NodeID>,
    pub dirty_ids_scratch: Vec<NodeID>,
    pub all_ids_scratch: Vec<NodeID>,
    pub visible_now: AHashSet<NodeID>,
    pub prev_visible: AHashSet<NodeID>,
    pub mesh_sources: AHashMap<NodeID, String>,
    pub material_surface_sources: AHashMap<NodeID, Vec<Option<String>>>,
    pub material_surface_overrides: AHashMap<NodeID, Vec<Option<Material3D>>>,
    pub collision_debug_state: AHashMap<NodeID, CollisionDebugState>,
    pub particle_path_cache: AHashMap<u64, perro_render_bridge::ParticleProfile3D>,
    pub particle_path_cache_order: VecDeque<u64>,
    pub pending_particle_path_loads: AHashSet<u64>,
    pub particle_path_load_tx:
        mpsc::Sender<(String, Option<perro_render_bridge::ParticleProfile3D>)>,
    pub particle_path_load_rx:
        mpsc::Receiver<(String, Option<perro_render_bridge::ParticleProfile3D>)>,
    pub last_camera: Option<Camera3DState>,
    pub retained_ambient_lights: AHashMap<NodeID, AmbientLight3DState>,
    pub retained_skies: AHashMap<NodeID, Sky3DState>,
    pub retained_ray_lights: AHashMap<NodeID, RayLight3DState>,
    pub retained_point_lights: AHashMap<NodeID, PointLight3DState>,
    pub retained_spot_lights: AHashMap<NodeID, SpotLight3DState>,
    pub retained_decals: AHashMap<NodeID, Decal3DState>,
    pub text_decal_texture_cache: AHashMap<NodeID, TextDecalTextureCache>,
    pub retained_mesh_draws: AHashMap<NodeID, RetainedMeshDrawState>,
    pub camera_activation_order: AHashMap<NodeID, u64>,
    pub next_camera_activation_order: u64,
    pub dense_instance_pose_cache: AHashMap<NodeID, DenseInstancePoseCache>,
    pub traversal_seen: AHashSet<NodeID>,
    pub dirty_skeletons_scratch: AHashSet<NodeID>,
    // Reverse index skeleton NodeID -> mesh instances bound to it, so a dirty
    // skeleton re-extracts its (few) skinned meshes without a full arena scan.
    // mesh_skeleton_map is the inverse (mesh -> its indexed skeleton) used to
    // reconcile rebinds. `built` gates the fast path; false => full-scan fallback.
    pub skeleton_mesh_map: AHashMap<NodeID, AHashSet<NodeID>>,
    pub mesh_skeleton_map: AHashMap<NodeID, NodeID>,
    pub skeleton_mesh_index_built: bool,
    pub skeleton_cache_scratch: AHashMap<NodeID, SkeletonPalette>,
    pub skeleton_global_scratch: Vec<glam::Mat4>,
    pub skeleton_palette_scratch: Vec<[[f32; 4]; 3]>,
    pub dense_instance_pose_scratch: Vec<DenseInstancePose3D>,
    pub removed_nodes: Vec<NodeID>,
    pub force_full_scan_once: bool,
}

pub type Render3dSystem = Render3DState;

impl Render3DState {
    pub fn new() -> Self {
        let (particle_path_load_tx, particle_path_load_rx) = mpsc::channel();
        Self {
            traversal_ids: Vec::new(),
            traversal_child_scratch: Vec::new(),
            dirty_ids_scratch: Vec::new(),
            all_ids_scratch: Vec::new(),
            visible_now: AHashSet::default(),
            prev_visible: AHashSet::default(),
            mesh_sources: AHashMap::default(),
            material_surface_sources: AHashMap::default(),
            material_surface_overrides: AHashMap::default(),
            collision_debug_state: AHashMap::default(),
            particle_path_cache: AHashMap::default(),
            particle_path_cache_order: VecDeque::new(),
            pending_particle_path_loads: AHashSet::default(),
            particle_path_load_tx,
            particle_path_load_rx,
            last_camera: None,
            retained_ambient_lights: AHashMap::default(),
            retained_skies: AHashMap::default(),
            retained_ray_lights: AHashMap::default(),
            retained_point_lights: AHashMap::default(),
            retained_spot_lights: AHashMap::default(),
            retained_decals: AHashMap::default(),
            text_decal_texture_cache: AHashMap::default(),
            retained_mesh_draws: AHashMap::default(),
            camera_activation_order: AHashMap::default(),
            next_camera_activation_order: 1,
            dense_instance_pose_cache: AHashMap::default(),
            traversal_seen: AHashSet::default(),
            dirty_skeletons_scratch: AHashSet::default(),
            skeleton_mesh_map: AHashMap::default(),
            mesh_skeleton_map: AHashMap::default(),
            skeleton_mesh_index_built: false,
            skeleton_cache_scratch: AHashMap::default(),
            skeleton_global_scratch: Vec::new(),
            skeleton_palette_scratch: Vec::new(),
            dense_instance_pose_scratch: Vec::new(),
            removed_nodes: Vec::new(),
            force_full_scan_once: false,
        }
    }

    pub fn note_removed_node(&mut self, node: NodeID) {
        self.mesh_sources.remove(&node);
        self.material_surface_sources.remove(&node);
        self.material_surface_overrides.remove(&node);
        self.text_decal_texture_cache.remove(&node);
        self.retained_mesh_draws.remove(&node);
        // Drop from the skeleton reverse index: node may be a bound mesh instance
        // and/or a skeleton whose bucket must go.
        self.unindex_mesh_skeleton(node);
        self.skeleton_mesh_map.remove(&node);
        self.removed_nodes.push(node);
    }

    // Reconcile a single mesh instance's skeleton binding in the reverse index.
    // `skeleton` nil => unbound. No-op when the recorded binding already matches.
    pub fn index_mesh_skeleton(&mut self, mesh: NodeID, skeleton: NodeID) {
        let want = (!skeleton.is_nil()).then_some(skeleton);
        if self.mesh_skeleton_map.get(&mesh).copied() == want {
            return;
        }
        self.unindex_mesh_skeleton(mesh);
        if let Some(skeleton) = want {
            self.mesh_skeleton_map.insert(mesh, skeleton);
            self.skeleton_mesh_map
                .entry(skeleton)
                .or_default()
                .insert(mesh);
        }
    }

    fn unindex_mesh_skeleton(&mut self, mesh: NodeID) {
        if let Some(old) = self.mesh_skeleton_map.remove(&mesh)
            && let Some(bucket) = self.skeleton_mesh_map.get_mut(&old)
        {
            bucket.remove(&mesh);
            if bucket.is_empty() {
                self.skeleton_mesh_map.remove(&old);
            }
        }
    }

    pub fn clear_skeleton_mesh_index(&mut self) {
        self.skeleton_mesh_map.clear();
        self.mesh_skeleton_map.clear();
        self.skeleton_mesh_index_built = false;
    }

    pub fn request_full_scan_once(&mut self) {
        self.force_full_scan_once = true;
    }

    pub fn full_scan_pending(&self) -> bool {
        self.force_full_scan_once
    }

    pub fn clear_full_scan_pending(&mut self) {
        self.force_full_scan_once = false;
    }

    pub fn collect_traversal<I, A, F>(
        &mut self,
        dirty_ids: I,
        all_ids: A,
        bootstrap_scan: bool,
        children_of: F,
    ) -> Vec<NodeID>
    where
        I: IntoIterator<Item = NodeID>,
        A: IntoIterator<Item = NodeID>,
        F: FnMut(NodeID, &mut Vec<NodeID>),
    {
        let include_all = self.force_full_scan_once || bootstrap_scan;
        self.force_full_scan_once = false;
        let mut traversal_ids = std::mem::take(&mut self.traversal_ids);
        let mut child_scratch = std::mem::take(&mut self.traversal_child_scratch);
        collect_tree_traversal(
            &mut traversal_ids,
            &mut child_scratch,
            dirty_ids,
            all_ids,
            include_all,
            children_of,
        );
        child_scratch.clear();
        self.traversal_child_scratch = child_scratch;
        traversal_ids
    }

    pub fn begin_visible_pass(&mut self) -> AHashSet<NodeID> {
        let mut visible_now = std::mem::take(&mut self.visible_now);
        visible_now.clear();
        visible_now.extend(self.prev_visible.iter().copied());
        for node in self.removed_nodes.drain(..) {
            visible_now.remove(&node);
        }
        visible_now
    }

    pub fn collect_removed_visible_nodes(&self, visible_now: &AHashSet<NodeID>) -> Vec<NodeID> {
        self.prev_visible
            .iter()
            .copied()
            .filter(|node| !visible_now.contains(node))
            .collect()
    }

    pub fn finish_visible_pass(
        &mut self,
        mut traversal_ids: Vec<NodeID>,
        mut visible_now: AHashSet<NodeID>,
    ) {
        std::mem::swap(&mut self.prev_visible, &mut visible_now);
        visible_now.clear();
        self.visible_now = visible_now;
        traversal_ids.clear();
        self.traversal_ids = traversal_ids;
    }
}

impl Default for Render3DState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DenseInstancePoseCache {
    pub signature: u64,
    pub poses: Arc<[DenseInstancePose3D]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDecalTextureCache {
    pub signature: u64,
    pub texture: TextureID,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetainedMeshDrawState {
    pub mesh: MeshID,
    pub surfaces: Arc<[MeshSurfaceBinding3D]>,
    pub instances: RetainedMeshInstanceState,
    pub skeleton: Option<SkeletonPalette>,
    pub blend_shape_weights: Arc<[f32]>,
    pub meshlet_override: Option<bool>,
    pub lod: LODOptions3D,
    pub blend: MeshBlendOptions3D,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
}

#[derive(Debug, Clone)]
pub enum RetainedMeshInstanceState {
    Single([[f32; 4]; 4]),
    Matrices(Arc<[[[f32; 4]; 4]]>),
    Dense {
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        poses: Arc<[DenseInstancePose3D]>,
    },
}

impl PartialEq for RetainedMeshInstanceState {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Single(a), Self::Single(b)) => a == b,
            (Self::Matrices(a), Self::Matrices(b)) => Arc::ptr_eq(a, b) || a == b,
            (
                Self::Dense {
                    node_model: node_model_a,
                    instance_scale: instance_scale_a,
                    poses: poses_a,
                },
                Self::Dense {
                    node_model: node_model_b,
                    instance_scale: instance_scale_b,
                    poses: poses_b,
                },
            ) => {
                node_model_a == node_model_b
                    && instance_scale_a == instance_scale_b
                    && (Arc::ptr_eq(poses_a, poses_b) || poses_a == poses_b)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CollisionDebugState {
    pub signature: u64,
    pub edge_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(raw: u64) -> NodeID {
        NodeID::from_u64(raw)
    }

    fn node_set(ids: &[NodeID]) -> AHashSet<NodeID> {
        ids.iter().copied().collect()
    }

    #[test]
    fn render_request_ids_round_trip_supported_kinds() {
        let sprite_node = node(11);
        let mesh_node = node(12);
        let material_node = node(13);
        let tilemap_node = node(14);
        let ui_node = node(15);

        let sprite = sprite_2d_texture_request(sprite_node);
        let mesh = mesh_3d_request(mesh_node);
        let material = material_3d_request(material_node, 3);
        let tilemap = tilemap_2d_texture_request(tilemap_node);
        let ui = ui_image_texture_request(ui_node);

        assert_eq!(decode_2d_texture_request_node(sprite), Some(sprite_node));
        assert_eq!(decode_3d_mesh_request_node(mesh), Some(mesh_node));
        assert_eq!(
            decode_3d_material_request_node(material),
            Some(material_node)
        );
        assert_eq!(decode_render_request_node(sprite), Some(sprite_node));
        assert_eq!(decode_render_request_node(mesh), Some(mesh_node));
        assert_eq!(decode_render_request_node(material), Some(material_node));
        assert_eq!(decode_render_request_node(tilemap), Some(tilemap_node));
        assert_eq!(decode_render_request_node(ui), Some(ui_node));
        assert_eq!(decode_2d_texture_request_node(mesh), None);
        assert_eq!(decode_3d_mesh_request_node(sprite), None);
    }

    #[test]
    fn render_request_event_decode_skips_non_request_events() {
        let request_node = node(21);
        let request = mesh_3d_request(request_node);

        assert_eq!(
            decode_render_request_node_from_event(&RenderEvent::MeshCreated {
                request,
                id: MeshID::new(1),
                mesh: None,
            }),
            Some(request_node)
        );
        assert_eq!(
            decode_render_request_node_from_event(&RenderEvent::TextureLoaded {
                id: TextureID::new(2),
            }),
            None
        );
    }

    #[test]
    fn render_2d_traversal_walks_dirty_roots_and_children_once() {
        let mut state = Render2DState::new();

        let traversal = state.collect_traversal([node(1)], [node(99)], false, |id, children| {
            match id.as_u64() {
                1 => children.extend([node(2), node(3)]),
                2 => children.push(node(4)),
                3 => children.push(node(4)),
                _ => {}
            }
        });
        assert_eq!(traversal, vec![node(1), node(2), node(3), node(4)]);

        state.request_full_scan_once();
        assert!(state.full_scan_pending());
        let traversal =
            state.collect_traversal(Vec::<NodeID>::new(), [node(8), node(9)], false, |_, _| {});
        assert_eq!(traversal, vec![node(8), node(9)]);
        assert!(!state.full_scan_pending());
    }

    #[test]
    fn render_2d_visible_pass_drops_removed_nodes() {
        let mut state = Render2DState::new();
        state.prev_visible.extend([node(1), node(2), node(3)]);
        state.note_removed_node(node(2));

        let visible_now = state.begin_visible_pass();
        assert_eq!(visible_now, node_set(&[node(1), node(3)]));

        let removed = state.collect_removed_visible_nodes(&visible_now);
        assert_eq!(removed, vec![node(2)]);

        state.finish_visible_pass(vec![node(7), node(8)], visible_now);
        assert_eq!(state.prev_visible, node_set(&[node(1), node(3)]));
        assert!(state.visible_now.is_empty());
        assert!(state.traversal_ids.is_empty());
    }

    #[test]
    fn ui_extraction_plan_adds_layout_children_commands_and_retained_input() {
        let mut state = RenderUiState::new();
        let retained = node(50);
        state
            .retained_commands
            .insert(retained, UiCommand::RemoveNode { node: retained });

        let options = UiExtractionOptions {
            mask: UiDirtyMask {
                layout_mask: 0b0001,
                layout_parent: 0b0010,
                commands: 0b0100,
                default_flags: 0b0001,
            },
            bootstrap_scan: false,
            input_changed: true,
        };
        let plan = state.collect_extraction_plan(
            [
                (node(1), 0b0001),
                (node(2), 0b0010),
                (node(3), 0b0100),
                (node(4), 0),
            ],
            Vec::<NodeID>::new(),
            options,
            |id, out| {
                if id == node(2) {
                    out.extend([node(20), node(21)]);
                }
            },
            |id, children| match id.as_u64() {
                1 => children.push(node(10)),
                20 => children.push(node(200)),
                _ => {}
            },
        );

        assert_eq!(plan.dirty_nodes, 4);
        assert_eq!(plan.affected_nodes, 6);
        assert_eq!(
            node_set(&plan.traversal_ids),
            node_set(&[node(1), node(20), node(21), node(4), node(10), node(200)])
        );
        assert!(plan.command_ids.contains(&node(3)));
        assert!(plan.command_ids.contains(&retained));
        for id in plan.traversal_ids.iter() {
            assert!(plan.command_ids.contains(id));
        }

        state.restore_extraction_plan(plan.traversal_ids, plan.command_ids, plan.command_seen);
        assert!(state.traversal_ids.is_empty());
        assert!(state.command_ids.is_empty());
        assert!(state.command_seen.is_empty());
    }
}
