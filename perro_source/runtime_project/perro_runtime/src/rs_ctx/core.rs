use super::state::{RuntimeLocalizationState, RuntimeResourceState};
use crate::runtime_project::{
    StaticAnimationLookup, StaticAudioLookup, StaticLocalizationLookup, StaticMaterialLookup,
    StaticSkeletonLookup,
};
use perro_bark::AudioController;
use perro_project::LocalizationConfig;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
    pub(super) localization: std::sync::RwLock<RuntimeLocalizationState>,
    pub(super) bark: Mutex<Option<AudioController>>,
    pub(super) static_material_lookup: Option<StaticMaterialLookup>,
    pub(super) static_skeleton_lookup: Option<StaticSkeletonLookup>,
    pub(super) static_animation_lookup: Option<StaticAnimationLookup>,
    pub(super) static_localization_lookup: Option<StaticLocalizationLookup>,
    pub(super) skeleton_bones_cache: Mutex<HashMap<String, Vec<perro_nodes::skeleton_3d::Bone3D>>>,
    pub(super) viewport_size: Mutex<(u32, u32)>,
}

impl RuntimeResourceApi {
    pub(crate) fn new(
        static_material_lookup: Option<StaticMaterialLookup>,
        static_audio_lookup: Option<StaticAudioLookup>,
        static_skeleton_lookup: Option<StaticSkeletonLookup>,
        static_animation_lookup: Option<StaticAnimationLookup>,
        static_localization_lookup: Option<StaticLocalizationLookup>,
        localization_config: Option<LocalizationConfig>,
    ) -> Arc<Self> {
        let api = Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
            localization: std::sync::RwLock::new(RuntimeLocalizationState::new(
                localization_config.as_ref(),
            )),
            bark: Mutex::new(AudioController::new(static_audio_lookup).ok()),
            static_material_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            static_localization_lookup,
            skeleton_bones_cache: Mutex::new(HashMap::new()),
            viewport_size: Mutex::new((1, 1)),
        });
        api.initialize_localization();
        api
    }

    pub(crate) fn set_viewport_size(&self, width: u32, height: u32) {
        let mut viewport = self
            .viewport_size
            .lock()
            .expect("resource api viewport mutex poisoned");
        *viewport = (width.max(1), height.max(1));
    }

    pub(crate) fn drain_commands(&self, out: &mut Vec<RenderCommand>) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        out.append(&mut state.queued_commands);
    }

    pub(crate) fn apply_render_event(&self, event: &RenderEvent) {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        match event {
            RenderEvent::TextureCreated { request, id } => {
                let _ = state.occupy_texture_id(*id);
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
                if id.is_nil() {
                    if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                        state.mesh_pending_by_source.remove(&source);
                        if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                            let _ = state.free_mesh_id(pending_id);
                        }
                        state.mesh_by_source.remove(&source);
                        state.mesh_reserve_pending.remove(&source);
                        state.mesh_drop_pending.remove(&source);
                    }
                    return;
                }
                let _ = state.occupy_mesh_id(*id);
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    state.mesh_pending_by_source.remove(&source);
                    let pending_id = state.mesh_pending_id_by_request.remove(request);
                    if let Some(pending_id) = pending_id
                        && pending_id != *id
                    {
                        // Backend resolved this request to an existing mesh id.
                        // Free the temporary pending slot to avoid mesh-id leaks/churn.
                        let _ = state.free_mesh_id(pending_id);
                        state.mesh_id_alias.insert(pending_id, *id);
                    }
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
                let _ = state.occupy_material_id(*id);
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
