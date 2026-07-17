use super::core::RuntimeResourceApi;
use perro_ids::MaterialID;
use perro_render_bridge::{Material3D, RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::MaterialAPI;

impl MaterialAPI for RuntimeResourceApi {
    fn load_material_source(&self, source: &str) -> MaterialID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_material_source_hashed(hash, None)
        } else {
            self.load_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn reserve_material_source(&self, source: &str) -> MaterialID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.reserve_material_source_hashed(hash, None)
        } else {
            self.reserve_material_source_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn load_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID {
        let material = self.static_material(source_hash).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(&source_hash).copied() {
            return id;
        }
        let Some(source) = source else {
            return MaterialID::nil();
        };
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_data_by_id.insert(id, material.clone());
        state.material_by_source.insert(source_hash, id);
        state
            .material_pending_by_source
            .insert(source_hash, request);
        state
            .material_pending_source_by_request
            .insert(request, source.to_string());
        state.material_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: Some(source.to_string()),
                reserved: false,
            }));
        let source = source.to_string();
        drop(state);
        if self.static_material_lookup.is_none() {
            let mut state = self.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.insert(id);
            drop(state);
            self.queue_material_source_load(id, source);
        }
        id
    }

    fn create_material(&self, material: Material3D) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_pending_id_by_request.insert(request, id);
        state.material_data_by_id.insert(id, material.clone());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: None,
                reserved: false,
            }));
        id
    }

    fn create_material_from_bytes(&self, bytes: &[u8]) -> MaterialID {
        let Some(material) = crate::material_schema::load_from_bytes(bytes) else {
            return MaterialID::nil();
        };
        self.create_material(material)
    }

    fn get_material_data(&self, id: MaterialID) -> Option<Material3D> {
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.material_data_by_id.get(&id).cloned()
    }

    fn write_material_data(&self, id: MaterialID, material: Material3D) -> bool {
        if id.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_data_by_id.insert(id, material.clone());
        state.material_write_pending_by_id.insert(id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::WriteMaterialData { id, material },
        ));
        true
    }

    fn is_material_loaded(&self, id: MaterialID) -> bool {
        if id.is_nil() {
            return false;
        }
        self.poll_async_material_loads();
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.material_loaded_by_id.contains(&id)
            && state.material_data_by_id.contains_key(&id)
            && !state.material_load_pending_by_id.contains(&id)
            && !state
                .material_pending_id_by_request
                .values()
                .any(|pending| *pending == id)
    }

    fn reserve_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID {
        let material = self.static_material(source_hash).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(&source_hash).copied() {
            if state.material_pending_by_source.contains_key(&source_hash) {
                state.material_reserve_pending.insert(source_hash);
                return id;
            }
            state.queued_commands.push(RenderCommand::Resource(
                ResourceCommand::SetMaterialReserved { id, reserved: true },
            ));
            return id;
        }
        let Some(source) = source else {
            return MaterialID::nil();
        };
        state.material_drop_pending.remove(&source_hash);
        state.material_reserve_pending.insert(source_hash);
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_data_by_id.insert(id, material.clone());
        state.material_by_source.insert(source_hash, id);
        state
            .material_pending_by_source
            .insert(source_hash, request);
        state
            .material_pending_source_by_request
            .insert(request, source.to_string());
        state.material_pending_id_by_request.insert(request, id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: Some(source.to_string()),
                reserved: true,
            }));
        let source = source.to_string();
        drop(state);
        if self.static_material_lookup.is_none() {
            let mut state = self.state.lock().expect("resource api mutex poisoned");
            state.material_load_pending_by_id.insert(id);
            drop(state);
            self.queue_material_source_load(id, source);
        }
        id
    }

    fn reserve_material_id(&self, id: MaterialID) -> bool {
        if id.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let known = state.material_data_by_id.contains_key(&id)
            || state.material_loaded_by_id.contains(&id)
            || state
                .material_by_source
                .values()
                .any(|existing| *existing == id)
            || state
                .material_pending_id_by_request
                .values()
                .any(|pending| *pending == id);
        if !known {
            return false;
        }
        if let Some(source_hash) = state
            .material_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash))
            .or_else(|| {
                state
                    .material_pending_id_by_request
                    .iter()
                    .find_map(|(request, pending_id)| {
                        (*pending_id == id)
                            .then(|| state.material_pending_source_by_request.get(request))
                            .flatten()
                            .map(|source| perro_ids::string_to_u64(source))
                    })
            })
        {
            state.material_reserve_pending.insert(source_hash);
            state.material_drop_pending.remove(&source_hash);
        }
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::SetMaterialReserved { id, reserved: true },
        ));
        true
    }

    fn drop_material_source(&self, id: MaterialID) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_data_by_id.remove(&id);
        state.material_loaded_by_id.remove(&id);
        state.material_load_pending_by_id.remove(&id);
        state.material_write_pending_by_id.remove(&id);
        if state.default_material_id == Some(id) {
            state.default_material_id = None;
        }
        state
            .shared_material_by_data
            .retain(|(_, shared_id)| *shared_id != id);
        let source = state
            .material_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash));
        if let Some(source_hash) = source {
            state.material_reserve_pending.remove(&source_hash);
            if state.material_pending_by_source.contains_key(&source_hash) {
                state.material_drop_pending.insert(source_hash);
                return true;
            }
            state.material_by_source.remove(&source_hash);
        }
        let _ = state.free_material_id(id);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::DropMaterial {
                id,
            }));
        true
    }
}

impl RuntimeResourceApi {
    pub(crate) fn default_material_id(&self) -> MaterialID {
        self.shared_inline_material_id(Material3D::default())
    }

    pub(crate) fn shared_inline_material_id(&self, material: Material3D) -> MaterialID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if material == Material3D::default()
            && let Some(id) = state.default_material_id
            && !id.is_nil()
        {
            return id;
        }
        if let Some((_, id)) = state
            .shared_material_by_data
            .iter()
            .find(|(existing, _)| existing == &material)
        {
            return *id;
        }

        let request = state.allocate_request();
        let id = state.allocate_material_id();
        if material == Material3D::default() {
            state.default_material_id = Some(id);
        }
        state.shared_material_by_data.push((material.clone(), id));
        state.material_pending_id_by_request.insert(request, id);
        state.material_data_by_id.insert(id, material.clone());
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id,
                material,
                source: None,
                reserved: true,
            }));
        id
    }

    fn static_material(&self, source_hash: u64) -> Option<Material3D> {
        self.static_material_lookup
            .map(|lookup| lookup(source_hash).clone())
    }

    pub(crate) fn is_material_id_pending(&self, material: MaterialID) -> bool {
        if material.is_nil() {
            return false;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .material_pending_id_by_request
            .values()
            .any(|pending| *pending == material)
    }
}

// Material arms of the render-event stream; called from
// `RuntimeResourceApi::apply_render_event` under the state lock.
impl super::state::RuntimeResourceState {
    pub(super) fn apply_material_loaded(&mut self, id: MaterialID) {
        if !self.material_load_pending_by_id.contains(&id) {
            self.material_write_pending_by_id.remove(&id);
            self.material_loaded_by_id.insert(id);
            if super::core::asset_ready_log_enabled() {
                eprintln!("[perro][asset-ready] material loaded id={id:?}");
            }
        } else if super::core::asset_ready_log_enabled() {
            eprintln!("[perro][asset-ready] material backend ack before source task id={id:?}");
        }
    }

    pub(super) fn apply_material_created(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: MaterialID,
    ) {
        let _ = self.occupy_material_id(id);
        let pending_id = self.material_pending_id_by_request.remove(&request);
        if let Some(source) = self.material_pending_source_by_request.remove(&request) {
            let source_hash = perro_ids::string_to_u64(&source);
            self.material_pending_by_source.remove(&source_hash);
            if self.material_drop_pending.remove(&source_hash) {
                self.queued_commands
                    .push(RenderCommand::Resource(ResourceCommand::DropMaterial {
                        id,
                    }));
                self.material_by_source.remove(&source_hash);
                if let Some(pending_id) = pending_id {
                    let _ = self.free_material_id(pending_id);
                }
            } else {
                self.material_by_source.insert(source_hash, id);
                if self.material_reserve_pending.remove(&source_hash) {
                    self.queued_commands.push(RenderCommand::Resource(
                        ResourceCommand::SetMaterialReserved { id, reserved: true },
                    ));
                }
            }
        }
        if let Some(pending_id) = pending_id
            && pending_id != id
        {
            if self.material_load_pending_by_id.remove(&pending_id) {
                self.material_load_pending_by_id.insert(id);
            }
            if self.material_write_pending_by_id.remove(&pending_id) {
                self.material_write_pending_by_id.insert(id);
            }
            if let Some(data) = self.material_data_by_id.remove(&pending_id) {
                self.material_data_by_id.insert(id, data);
                if self.material_loaded_by_id.remove(&pending_id) {
                    self.material_loaded_by_id.insert(id);
                }
            }
            let _ = self.free_material_id(pending_id);
        }
        if !self.material_load_pending_by_id.contains(&id)
            && self.material_data_by_id.contains_key(&id)
        {
            self.material_loaded_by_id.insert(id);
            if super::core::asset_ready_log_enabled() {
                eprintln!("[perro][asset-ready] material created ready id={id:?}");
            }
        } else if super::core::asset_ready_log_enabled() {
            eprintln!(
                "[perro][asset-ready] material created wait id={id:?} source_pending={} has_data={}",
                self.material_load_pending_by_id.contains(&id),
                self.material_data_by_id.contains_key(&id)
            );
        }
    }

    pub(super) fn apply_material_dropped(&mut self, id: MaterialID) {
        self.material_data_by_id.remove(&id);
        self.material_loaded_by_id.remove(&id);
        self.material_load_pending_by_id.remove(&id);
        self.material_write_pending_by_id.remove(&id);
        if self.default_material_id == Some(id) {
            self.default_material_id = None;
        }
        self.shared_material_by_data
            .retain(|(_, shared_id)| *shared_id != id);
        let source = self
            .material_by_source
            .iter()
            .find_map(|(source_hash, existing)| (*existing == id).then_some(*source_hash));
        if let Some(source_hash) = source {
            self.material_by_source.remove(&source_hash);
            self.material_pending_by_source.remove(&source_hash);
            self.material_reserve_pending.remove(&source_hash);
            self.material_drop_pending.remove(&source_hash);
        }
        let _ = self.free_material_id(id);
    }

    pub(super) fn apply_material_failed(&mut self, request: perro_render_bridge::RenderRequestID) {
        if let Some(source) = self.material_pending_source_by_request.remove(&request) {
            let source_hash = perro_ids::string_to_u64(&source);
            self.material_pending_by_source.remove(&source_hash);
            if let Some(pending_id) = self.material_pending_id_by_request.remove(&request) {
                let _ = self.free_material_id(pending_id);
                self.material_data_by_id.remove(&pending_id);
                self.material_loaded_by_id.remove(&pending_id);
                self.material_load_pending_by_id.remove(&pending_id);
                self.material_write_pending_by_id.remove(&pending_id);
            }
            self.material_by_source.remove(&source_hash);
            self.material_reserve_pending.remove(&source_hash);
            self.material_drop_pending.remove(&source_hash);
        }
        if let Some(pending_id) = self.material_pending_id_by_request.remove(&request) {
            let _ = self.free_material_id(pending_id);
            self.material_data_by_id.remove(&pending_id);
            self.material_loaded_by_id.remove(&pending_id);
            self.material_load_pending_by_id.remove(&pending_id);
            self.material_write_pending_by_id.remove(&pending_id);
        }
    }
}

#[cfg(test)]
fn normalized_static_material_lookup_alias(source: &str) -> Option<String> {
    let (path, fragment) = split_source_fragment(source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    let Some(fragment) = fragment else {
        return Some(format!("{path}:mat[0]"));
    };
    if let Some(index) = parse_fragment_index(fragment, "material") {
        return Some(format!("{path}:mat[{index}]"));
    }
    if let Some(index) = parse_fragment_index(fragment, "mat") {
        return Some(format!("{path}:material[{index}]"));
    }
    None
}

#[cfg(test)]
fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

#[cfg(test)]
fn parse_fragment_index(fragment: &str, key: &str) -> Option<u32> {
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::normalized_static_material_lookup_alias;

    #[test]
    fn gltf_material_source_without_fragment_maps_to_mat_zero_alias() {
        assert_eq!(
            normalized_static_material_lookup_alias("res://models/hero.glb"),
            Some("res://models/hero.glb:mat[0]".to_string())
        );
    }

    #[test]
    fn gltf_material_selector_aliases_to_mat_selector() {
        assert_eq!(
            normalized_static_material_lookup_alias("res://models/hero.glb:material[2]"),
            Some("res://models/hero.glb:mat[2]".to_string())
        );
    }

    #[test]
    fn gltf_mat_selector_aliases_to_material_selector() {
        assert_eq!(
            normalized_static_material_lookup_alias("res://models/hero.glb:mat[1]"),
            Some("res://models/hero.glb:material[1]".to_string())
        );
    }
}
