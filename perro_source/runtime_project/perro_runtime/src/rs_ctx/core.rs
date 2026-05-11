use super::state::{RuntimeLocalizationState, RuntimeResourceState};
use crate::runtime_project::{
    StaticAnimationLookup, StaticAnimationTreeLookup, StaticAudioLookup, StaticLocalizationLookup,
    StaticMaterialLookup, StaticSkeletonLookup,
};
use perro_bark::AudioController;
use perro_ids::string_to_u64;
use perro_project::LocalizationConfig;
use perro_render_bridge::{RenderCommand, RenderEvent};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug)]
pub(crate) enum QueuedSpatialAudioPos {
    TwoD(perro_structs::Vector2),
    ThreeD(perro_structs::Vector3),
}

#[derive(Clone, Debug)]
pub(crate) struct QueuedSpatialAudio {
    pub source: String,
    pub bus_id: Option<perro_ids::AudioBusID>,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub from_start: f32,
    pub from_end: f32,
    pub range: f32,
    pub pos: QueuedSpatialAudioPos,
}

pub struct RuntimeResourceApi {
    pub(super) state: Mutex<RuntimeResourceState>,
    pub(super) localization: std::sync::RwLock<RuntimeLocalizationState>,
    pub(crate) bark: Mutex<Option<AudioController>>,
    pub(crate) spatial_audio_queue: Mutex<Vec<QueuedSpatialAudio>>,
    pub(crate) audio_listener_2d: Mutex<Option<perro_bark::AudioListener2D>>,
    pub(crate) audio_listener_3d: Mutex<Option<perro_bark::AudioListener3D>>,
    pub(super) static_material_lookup: Option<StaticMaterialLookup>,
    pub(super) static_skeleton_lookup: Option<StaticSkeletonLookup>,
    pub(super) static_animation_lookup: Option<StaticAnimationLookup>,
    pub(super) static_animation_tree_lookup: Option<StaticAnimationTreeLookup>,
    pub(super) static_localization_lookup: Option<StaticLocalizationLookup>,
    pub(super) skeleton_bones_2d_cache:
        Mutex<HashMap<String, Vec<perro_nodes::skeleton_2d::Bone2D>>>,
    pub(super) skeleton_bones_3d_cache:
        Mutex<HashMap<String, Vec<perro_nodes::skeleton_3d::Bone3D>>>,
    pub(super) viewport_size: Mutex<(u32, u32)>,
}

impl RuntimeResourceApi {
    pub(crate) fn new(
        static_material_lookup: Option<StaticMaterialLookup>,
        static_audio_lookup: Option<StaticAudioLookup>,
        static_skeleton_lookup: Option<StaticSkeletonLookup>,
        static_animation_lookup: Option<StaticAnimationLookup>,
        static_animation_tree_lookup: Option<StaticAnimationTreeLookup>,
        static_localization_lookup: Option<StaticLocalizationLookup>,
        localization_config: Option<LocalizationConfig>,
    ) -> Arc<Self> {
        let api = Arc::new(Self {
            state: Mutex::new(RuntimeResourceState::new()),
            localization: std::sync::RwLock::new(RuntimeLocalizationState::new(
                localization_config.as_ref(),
            )),
            bark: Mutex::new(AudioController::new(static_audio_lookup).ok()),
            spatial_audio_queue: Mutex::new(Vec::new()),
            audio_listener_2d: Mutex::new(None),
            audio_listener_3d: Mutex::new(None),
            static_material_lookup,
            static_skeleton_lookup,
            static_animation_lookup,
            static_animation_tree_lookup,
            static_localization_lookup,
            skeleton_bones_2d_cache: Mutex::new(HashMap::new()),
            skeleton_bones_3d_cache: Mutex::new(HashMap::new()),
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

    pub(crate) fn set_audio_listener_2d(&self, position: [f32; 2], rotation_radians: f32) {
        let mut listener = self
            .audio_listener_2d
            .lock()
            .expect("resource api audio 2d listener mutex poisoned");
        *listener = Some(perro_bark::AudioListener2D {
            position,
            rotation_radians,
        });
    }

    pub(crate) fn set_audio_listener_3d(&self, position: [f32; 3], rotation: [f32; 4]) {
        let mut listener = self
            .audio_listener_3d
            .lock()
            .expect("resource api audio 3d listener mutex poisoned");
        *listener = Some(perro_bark::AudioListener3D { position, rotation });
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
                    let source_hash = string_to_u64(&source);
                    state.texture_pending_by_source.remove(&source_hash);
                    let pending_id = state.texture_pending_id_by_request.remove(request);
                    if state.texture_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropTexture { id: *id },
                        ));
                        state.texture_by_source.remove(&source_hash);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_texture_id(pending_id);
                        }
                    } else {
                        state.texture_by_source.insert(source_hash, *id);
                        if state.texture_reserve_pending.remove(&source_hash) {
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
            RenderEvent::MeshCreated { request, id, mesh } => {
                if id.is_nil() {
                    if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                        let source_hash = string_to_u64(&source);
                        state.mesh_pending_by_source.remove(&source_hash);
                        if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                            let _ = state.free_mesh_id(pending_id);
                            state.mesh_source_by_id.remove(&pending_id);
                        }
                        state.mesh_by_source.remove(&source_hash);
                        state.mesh_reserve_pending.remove(&source_hash);
                        state.mesh_drop_pending.remove(&source_hash);
                    }
                    return;
                }
                let _ = state.occupy_mesh_id(*id);
                if let Some(mesh) = mesh {
                    state.mesh_data_by_id.insert(*id, mesh.clone());
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.mesh_pending_by_source.remove(&source_hash);
                    let pending_id = state.mesh_pending_id_by_request.remove(request);
                    if let Some(pending_id) = pending_id
                        && pending_id != *id
                    {
                        // Backend resolved this request to an existing mesh id.
                        // Free the temporary pending slot to avoid mesh-id leaks/churn.
                        let _ = state.free_mesh_id(pending_id);
                        state.mesh_id_alias.insert(pending_id, *id);
                        state.mesh_source_by_id.remove(&pending_id);
                    }
                    if state.mesh_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMesh { id: *id },
                        ));
                        state.mesh_by_source.remove(&source_hash);
                        state.mesh_source_by_id.remove(id);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_mesh_id(pending_id);
                        }
                    } else {
                        state.mesh_by_source.insert(source_hash, *id);
                        state.mesh_source_by_id.insert(*id, source);
                        if state.mesh_reserve_pending.remove(&source_hash) {
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
                let pending_id = state.material_pending_id_by_request.remove(request);
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.material_pending_by_source.remove(&source_hash);
                    if state.material_drop_pending.remove(&source_hash) {
                        state.queued_commands.push(RenderCommand::Resource(
                            perro_render_bridge::ResourceCommand::DropMaterial { id: *id },
                        ));
                        state.material_by_source.remove(&source_hash);
                        if let Some(pending_id) = pending_id {
                            let _ = state.free_material_id(pending_id);
                        }
                    } else {
                        state.material_by_source.insert(source_hash, *id);
                        if state.material_reserve_pending.remove(&source_hash) {
                            state.queued_commands.push(RenderCommand::Resource(
                                perro_render_bridge::ResourceCommand::SetMaterialReserved {
                                    id: *id,
                                    reserved: true,
                                },
                            ));
                        }
                    }
                }
                if let Some(pending_id) = pending_id
                    && pending_id != *id
                {
                    if let Some(data) = state.material_data_by_id.remove(&pending_id) {
                        state.material_data_by_id.insert(*id, data);
                    }
                    let _ = state.free_material_id(pending_id);
                }
            }
            RenderEvent::Failed { request, .. } => {
                if let Some(source) = state.texture_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.texture_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.texture_pending_id_by_request.remove(request) {
                        let _ = state.free_texture_id(pending_id);
                    }
                    state.texture_by_source.remove(&source_hash);
                    state.texture_reserve_pending.remove(&source_hash);
                    state.texture_drop_pending.remove(&source_hash);
                }
                if let Some(source) = state.mesh_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.mesh_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.mesh_pending_id_by_request.remove(request) {
                        let _ = state.free_mesh_id(pending_id);
                        state.mesh_data_by_id.remove(&pending_id);
                        state.mesh_source_by_id.remove(&pending_id);
                    }
                    state.mesh_by_source.remove(&source_hash);
                    state.mesh_reserve_pending.remove(&source_hash);
                    state.mesh_drop_pending.remove(&source_hash);
                }
                if let Some(source) = state.material_pending_source_by_request.remove(request) {
                    let source_hash = string_to_u64(&source);
                    state.material_pending_by_source.remove(&source_hash);
                    if let Some(pending_id) = state.material_pending_id_by_request.remove(request) {
                        let _ = state.free_material_id(pending_id);
                        state.material_data_by_id.remove(&pending_id);
                    }
                    state.material_by_source.remove(&source_hash);
                    state.material_reserve_pending.remove(&source_hash);
                    state.material_drop_pending.remove(&source_hash);
                }
                if let Some(pending_id) = state.material_pending_id_by_request.remove(request) {
                    let _ = state.free_material_id(pending_id);
                    state.material_data_by_id.remove(&pending_id);
                }
            }
        }
    }
}
