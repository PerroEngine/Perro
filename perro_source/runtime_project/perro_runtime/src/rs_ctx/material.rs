use super::core::RuntimeResourceApi;
use crate::material_schema;
use perro_ids::MaterialID;
use perro_render_bridge::{Material3D, RenderCommand, ResourceCommand};
use perro_resource_context::sub_apis::MaterialAPI;

impl MaterialAPI for RuntimeResourceApi {
    fn load_material_source(&self, source: &str) -> MaterialID {
        let material = self.load_material_source_data(source).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            return id;
        }
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_by_source.insert(source.to_string(), id);
        state
            .material_pending_by_source
            .insert(source.to_string(), request);
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
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::CreateMaterial {
                request,
                id: MaterialID::nil(),
                material,
                source: None,
                reserved: false,
            }));
        MaterialID::nil()
    }

    fn reserve_material_source(&self, source: &str) -> MaterialID {
        let material = self.load_material_source_data(source).unwrap_or_default();
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.material_by_source.get(source).copied() {
            if state.material_pending_by_source.contains_key(source) {
                state.material_reserve_pending.insert(source.to_string());
                return id;
            }
            state.queued_commands.push(RenderCommand::Resource(
                ResourceCommand::SetMaterialReserved { id, reserved: true },
            ));
            return id;
        }
        state.material_drop_pending.remove(source);
        state.material_reserve_pending.insert(source.to_string());
        let request = state.allocate_request();
        let id = state.allocate_material_id();
        state.material_by_source.insert(source.to_string(), id);
        state
            .material_pending_by_source
            .insert(source.to_string(), request);
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

    fn drop_material_source(&self, source: &str) -> bool {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.material_reserve_pending.remove(source);
        if state.material_pending_by_source.contains_key(source) {
            state.material_drop_pending.insert(source.to_string());
            return true;
        }
        if let Some(id) = state.material_by_source.remove(source) {
            let _ = state.free_material_id(id);
            state
                .queued_commands
                .push(RenderCommand::Resource(ResourceCommand::DropMaterial {
                    id,
                }));
            return true;
        }
        false
    }
}

impl RuntimeResourceApi {
    fn load_material_source_data(&self, source: &str) -> Option<Material3D> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        let normalized = normalize_source_slashes(source);
        if let Some(lookup) = self.static_material_lookup {
            if let Some(found) = lookup(source).cloned() {
                return Some(found);
            }
            if normalized.as_ref() != source
                && let Some(found) = lookup(normalized.as_ref()).cloned()
            {
                return Some(found);
            }
            if let Some(alias) = normalized_static_material_lookup_alias(source)
                && let Some(found) = lookup(alias.as_str()).cloned()
            {
                return Some(found);
            }
            if normalized.as_ref() != source
                && let Some(alias) = normalized_static_material_lookup_alias(normalized.as_ref())
                && let Some(found) = lookup(alias.as_str()).cloned()
            {
                return Some(found);
            }
        }
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
