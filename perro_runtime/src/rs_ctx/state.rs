use perro_ids::{MaterialID, MeshID, TextureID};
use perro_render_bridge::{RenderCommand, RenderRequestID};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(super) struct RuntimeResourceState {
    pub(super) next_request: u64,
    pub(super) queued_commands: Vec<RenderCommand>,
    pub(super) texture_by_source: HashMap<String, TextureID>,
    pub(super) texture_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) texture_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) texture_reserve_pending: HashSet<String>,
    pub(super) texture_drop_pending: HashSet<String>,
    pub(super) mesh_by_source: HashMap<String, MeshID>,
    pub(super) mesh_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) mesh_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) mesh_reserve_pending: HashSet<String>,
    pub(super) mesh_drop_pending: HashSet<String>,
    pub(super) material_by_source: HashMap<String, MaterialID>,
    pub(super) material_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) material_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) material_reserve_pending: HashSet<String>,
    pub(super) material_drop_pending: HashSet<String>,
}

impl RuntimeResourceState {
    const REQUEST_BASE: u64 = 0x1000_0000_0000_0000;

    pub(super) fn new() -> Self {
        Self {
            next_request: Self::REQUEST_BASE,
            ..Self::default()
        }
    }

    pub(super) fn allocate_request(&mut self) -> RenderRequestID {
        let request = RenderRequestID::new(self.next_request);
        self.next_request = self.next_request.wrapping_add(1);
        request
    }
}
