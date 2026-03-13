use super::state::RuntimeResourceState;
use crate::cns::TerrainStore;
use crate::runtime_project::StaticMaterialLookup;
use perro_bark::AudioController;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::sync::{Arc, Mutex};

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
    pub(super) bark: Mutex<Option<AudioController>>,
    pub(super) static_material_lookup: Option<StaticMaterialLookup>,
    pub(crate) terrain_store: Arc<Mutex<TerrainStore>>,
}

impl RuntimeResourceApi {
    pub(crate) fn new(
        static_material_lookup: Option<StaticMaterialLookup>,
        terrain_store: Arc<Mutex<TerrainStore>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
            bark: Mutex::new(AudioController::new().ok()),
            static_material_lookup,
            terrain_store,
        })
    }

    pub(crate) fn drain_commands(&self, out: &mut Vec<RenderCommand>) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        out.append(&mut state.queued_commands);
    }

    pub(crate) fn apply_render_event(&self, event: &RenderEvent) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        match event {
            RenderEvent::TextureCreated { request, id } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    state.texture_pending_by_source.remove(&source);
                    let pending_id = state.texture_pending_id_by_request.remove(request);
                    if state.texture_drop_pending.remove(&source) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropTexture { id: *id },
                        ));
                        state.texture_by_source.remove(&source);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_texture_id(pending_id);
                        }
                    } else {
                        state.texture_by_source.insert(source.clone(), *id);
                        if state.texture_reserve_pending.remove(&source) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetTextureReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
            }
            RenderEvent::MeshCreated { request, id } => {
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    state.mesh_pending_by_source.remove(&source);
                    let pending_id = state.mesh_pending_id_by_request.remove(request);
                    if state.mesh_drop_pending.remove(&source) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMesh { id: *id },
                        ));
                        state.mesh_by_source.remove(&source);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_mesh_id(pending_id);
                        }
                    } else {
                        state.mesh_by_source.insert(source.clone(), *id);
                        if state.mesh_reserve_pending.remove(&source) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetMeshReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
            }
            RenderEvent::MaterialCreated { request, id } => {
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    state.material_pending_by_source.remove(&source);
                    let pending_id = state.material_pending_id_by_request.remove(request);
                    if state.material_drop_pending.remove(&source) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMaterial { id: *id },
                        ));
                        state.material_by_source.remove(&source);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_material_id(pending_id);
                        }
                    } else {
                        state.material_by_source.insert(source.clone(), *id);
                        if state.material_reserve_pending.remove(&source) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetMaterialReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
            }
            RenderEvent::Failed { request, .. } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    state.texture_pending_by_source.remove(&source);
                    if let Some(pending_id) = state.texture_pending_id_by_request.remove(request) {
                        let _ = state.free_texture_id(pending_id);
                    }
                    state.texture_by_source.remove(&source);
                    state.texture_reserve_pending.remove(&source);
                    state.texture_drop_pending.remove(&source);
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    state.mesh_pending_by_source.remove(&source);
                    if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                        let _ = state.free_mesh_id(pending_id);
                    }
                    state.mesh_by_source.remove(&source);
                    state.mesh_reserve_pending.remove(&source);
                    state.mesh_drop_pending.remove(&source);
                }
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    state.material_pending_by_source.remove(&source);
                    if let Some(pending_id) = state.material_pending_id_by_request.remove(request) {
                        let _ = state.free_material_id(pending_id);
                    }
                    state.material_by_source.remove(&source);
                    state.material_reserve_pending.remove(&source);
                    state.material_drop_pending.remove(&source);
                }
            }
        }
    }
}
