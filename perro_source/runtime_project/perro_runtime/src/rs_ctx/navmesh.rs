use super::core::RuntimeResourceApi;
use perro_ids::{NavMeshID, string_to_u64};
use perro_resource_api::sub_apis::{NavMesh3D, NavMeshAPI, parse_pnav_bytes};

impl NavMeshAPI for RuntimeResourceApi {
    fn load_navmesh(&self, source: &str) -> NavMeshID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_navmesh_hashed(hash, None)
        } else {
            self.load_navmesh_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn reserve_navmesh(&self, source: &str) -> NavMeshID {
        self.load_navmesh(source)
    }

    fn load_navmesh_hashed(&self, source_hash: u64, source: Option<&str>) -> NavMeshID {
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.navmesh_by_source.get(&source_hash).copied()
            && state.has_navmesh_id(id)
        {
            return id;
        }
        let Some(source) = source.map(str::trim).filter(|v| !v.is_empty()) else {
            return NavMeshID::nil();
        };
        let source = normalize_source_slashes(source);
        let source_hash = string_to_u64(source.as_ref());
        if let Some(id) = state.navmesh_by_source.get(&source_hash).copied()
            && state.has_navmesh_id(id)
        {
            return id;
        }
        let Ok(bytes) = perro_io::load_asset(source.as_ref()) else {
            return NavMeshID::nil();
        };
        let Ok(navmesh) = parse_pnav_bytes(&bytes) else {
            return NavMeshID::nil();
        };
        let id = state.allocate_navmesh_id();
        state.navmesh_by_source.insert(source_hash, id);
        state.navmesh_source_by_id.insert(id, source.into_owned());
        state.navmesh_data_by_id.insert(id, navmesh);
        state.navmesh_loaded_by_id.insert(id);
        id
    }

    fn reserve_navmesh_hashed(&self, source_hash: u64, source: Option<&str>) -> NavMeshID {
        self.load_navmesh_hashed(source_hash, source)
    }

    fn create_navmesh_data(&self, data: NavMesh3D) -> NavMeshID {
        if data.is_empty() {
            return NavMeshID::nil();
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let id = state.allocate_navmesh_id();
        let source = format!("runtime://navmesh/{}:{}", id.index(), id.generation());
        state.navmesh_by_source.insert(string_to_u64(&source), id);
        state.navmesh_source_by_id.insert(id, source);
        state.navmesh_data_by_id.insert(id, data);
        state.navmesh_loaded_by_id.insert(id);
        id
    }

    fn create_navmesh_from_bytes(&self, bytes: &[u8]) -> NavMeshID {
        let Ok(navmesh) = parse_pnav_bytes(bytes) else {
            return NavMeshID::nil();
        };
        self.create_navmesh_data(navmesh)
    }

    fn get_navmesh_data(&self, id: NavMeshID) -> Option<NavMesh3D> {
        if id.is_nil() {
            return None;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.navmesh_data_by_id.get(&id).cloned()
    }

    fn write_navmesh_data(&self, id: NavMeshID, data: NavMesh3D) -> bool {
        if id.is_nil() || data.is_empty() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if !state.has_navmesh_id(id) {
            return false;
        }
        state.navmesh_data_by_id.insert(id, data);
        state.navmesh_loaded_by_id.insert(id);
        true
    }

    fn is_navmesh_loaded(&self, id: NavMeshID) -> bool {
        if id.is_nil() {
            return false;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state.navmesh_loaded_by_id.contains(&id)
    }

    fn drop_navmesh(&self, id: NavMeshID) -> bool {
        if id.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if !state.free_navmesh_id(id) {
            return false;
        }
        state
            .navmesh_by_source
            .retain(|_, existing| *existing != id);
        state.navmesh_source_by_id.remove(&id);
        state.navmesh_data_by_id.remove(&id);
        state.navmesh_loaded_by_id.remove(&id);
        true
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
mod tests {
    use super::RuntimeResourceApi;
    use perro_resource_api::ResourceWindow;

    #[test]
    fn navmesh_create_from_pnav_bytes_loads() {
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let res = ResourceWindow::new(api.as_ref());
        let id = res.NavMeshes().create_from_bytes(
            b"pnav 1
v 0 0 0
v 1 0 0
v 0 0 1
tri 0 1 2 layers=1
",
        );
        assert!(!id.is_nil());
        assert!(res.NavMeshes().is_loaded(id));
        assert_eq!(
            res.NavMeshes().get_data(id).unwrap().triangles[0]
                .layers
                .bits(),
            1
        );
    }
}
