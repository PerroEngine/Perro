use super::core::RuntimeResourceApi;
use crate::material_schema;
use perro_ids::MaterialID;
use perro_render_bridge::{Material3D, RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MaterialAPI;

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
        let material = self
            .load_material_source_data(source_hash, source)
            .unwrap_or_default();
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
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::WriteMaterialData { id, material },
        ));
        true
    }

    fn reserve_material_source_hashed(&self, source_hash: u64, source: Option<&str>) -> MaterialID {
        let material = self
            .load_material_source_data(source_hash, source)
            .unwrap_or_default();
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
        id
    }

    fn drop_material_source(&self, id: MaterialID) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_data_by_id.remove(&id);
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
    fn load_material_source_data(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> Option<Material3D> {
        let source = source.map(str::trim);
        if source.is_some_and(|v| v.is_empty()) {
            return None;
        }
        if let Some(lookup) = self.static_material_lookup {
            return Some(lookup(source_hash).clone());
        }
        let source = source?;
        let normalized = normalize_source_slashes(source);
        material_schema::load_from_source(source)
            .or_else(|| material_schema::load_from_source(normalized.as_ref()))
    }
}

fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
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
