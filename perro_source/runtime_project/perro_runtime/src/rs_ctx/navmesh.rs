use super::core::RuntimeResourceApi;
use perro_ids::{NavMeshID, string_to_u64};
use perro_resource_api::sub_apis::{
    NavMesh3D, NavMeshAPI, NavMeshResource3D, parse_pnav_resource_bytes,
};
use perro_structs::BitMask;
use std::sync::Arc;

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
        let Ok(navmesh) = parse_pnav_resource_bytes(&bytes) else {
            return NavMeshID::nil();
        };
        let id = state.allocate_navmesh_id();
        state.navmesh_by_source.insert(source_hash, id);
        state.navmesh_source_by_id.insert(id, source.into_owned());
        let navmesh = Arc::new(navmesh);
        let graph = Arc::new(crate::runtime::navmesh::SearchGraph::new(
            &navmesh,
            BitMask::ALL,
        ));
        state.navmesh_data_by_id.insert(id, navmesh);
        state
            .navmesh_graph_by_id_and_layers
            .insert((id, BitMask::ALL.bits()), graph);
        state.navmesh_loaded_by_id.insert(id);
        id
    }

    fn reserve_navmesh_hashed(&self, source_hash: u64, source: Option<&str>) -> NavMeshID {
        self.load_navmesh_hashed(source_hash, source)
    }

    fn create_navmesh_data(&self, data: NavMesh3D) -> NavMeshID {
        self.create_navmesh_resource_data(NavMeshResource3D::from_mesh(data))
    }

    fn create_navmesh_resource_data(&self, data: NavMeshResource3D) -> NavMeshID {
        if data.validate().is_err() {
            return NavMeshID::nil();
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let id = state.allocate_navmesh_id();
        let source = format!("runtime://navmesh/{}:{}", id.index(), id.generation());
        state.navmesh_by_source.insert(string_to_u64(&source), id);
        state.navmesh_source_by_id.insert(id, source);
        let data = Arc::new(data);
        let graph = Arc::new(crate::runtime::navmesh::SearchGraph::new(
            &data,
            BitMask::ALL,
        ));
        state.navmesh_data_by_id.insert(id, data);
        state
            .navmesh_graph_by_id_and_layers
            .insert((id, BitMask::ALL.bits()), graph);
        state.navmesh_loaded_by_id.insert(id);
        id
    }

    fn create_navmesh_from_bytes(&self, bytes: &[u8]) -> NavMeshID {
        let Ok(navmesh) = parse_pnav_resource_bytes(bytes) else {
            return NavMeshID::nil();
        };
        self.create_navmesh_resource_data(navmesh)
    }

    fn get_navmesh_data(&self, id: NavMeshID) -> Option<NavMesh3D> {
        if id.is_nil() {
            return None;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .navmesh_data_by_id
            .get(&id)
            .map(|data| data.mesh.clone())
    }

    fn get_navmesh_resource_data(&self, id: NavMeshID) -> Option<NavMeshResource3D> {
        if id.is_nil() {
            return None;
        }
        let state = self.state.lock().expect("resource api mutex poisoned");
        state
            .navmesh_data_by_id
            .get(&id)
            .map(|data| data.as_ref().clone())
    }

    fn write_navmesh_data(&self, id: NavMeshID, data: NavMesh3D) -> bool {
        self.write_navmesh_resource_data(id, NavMeshResource3D::from_mesh(data))
    }

    fn write_navmesh_resource_data(&self, id: NavMeshID, data: NavMeshResource3D) -> bool {
        if id.is_nil() || data.validate().is_err() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if !state.has_navmesh_id(id) {
            return false;
        }
        let data = Arc::new(data);
        let graph = Arc::new(crate::runtime::navmesh::SearchGraph::new(
            &data,
            BitMask::ALL,
        ));
        state.navmesh_data_by_id.insert(id, data);
        state
            .navmesh_graph_by_id_and_layers
            .retain(|(existing, _), _| *existing != id);
        state
            .navmesh_graph_by_id_and_layers
            .insert((id, BitMask::ALL.bits()), graph);
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
        state
            .navmesh_graph_by_id_and_layers
            .retain(|(existing, _), _| *existing != id);
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

impl RuntimeResourceApi {
    pub(crate) fn navmesh_query_data(
        &self,
        id: NavMeshID,
        layers: BitMask,
    ) -> Option<(
        Arc<NavMeshResource3D>,
        Arc<crate::runtime::navmesh::SearchGraph>,
    )> {
        if id.is_nil() || layers.is_empty() {
            return None;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let data = state.navmesh_data_by_id.get(&id)?.clone();
        let key = (id, layers.bits());
        let graph = if let Some(graph) = state.navmesh_graph_by_id_and_layers.get(&key) {
            graph.clone()
        } else {
            let graph = Arc::new(crate::runtime::navmesh::SearchGraph::new(&data, layers));
            let cached_for_id = state
                .navmesh_graph_by_id_and_layers
                .keys()
                .filter(|(existing, _)| *existing == id)
                .count();
            if cached_for_id >= 8
                && let Some(stale_key) = state
                    .navmesh_graph_by_id_and_layers
                    .keys()
                    .find(|(existing, bits)| *existing == id && *bits != BitMask::ALL.bits())
                    .copied()
            {
                state.navmesh_graph_by_id_and_layers.remove(&stale_key);
            }
            state
                .navmesh_graph_by_id_and_layers
                .insert(key, graph.clone());
            graph
        };
        Some((data, graph))
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeResourceApi;
    use perro_resource_api::{
        ResourceWindow,
        sub_apis::{NavMesh3D, NavMeshTriangle3D},
    };
    use perro_structs::{BitMask, Vector3};

    #[test]
    fn navmesh_create_from_pnav_bytes_loads() {
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let res = ResourceWindow::new(api.as_ref());
        let id = res.NavMeshes().create_from_bytes(
            b"pnav 1
v 0 0 0
v 1 0 0
v 0 0 1
tri 0 1 2 layers=1 area=3
link 0.1 0 0.1 0.2 0 0.2 cost=2
",
        );
        assert!(!id.is_nil());
        assert!(res.NavMeshes().is_loaded(id));
        assert_eq!(
            res.NavMeshes()
                .get_data(id)
                .expect("test or bench setup must succeed")
                .triangles[0]
                .layers
                .bits(),
            1
        );
        let resource = res
            .NavMeshes()
            .get_resource(id)
            .expect("test or bench setup must succeed");
        assert_eq!(resource.triangle_areas, vec![3]);
        assert_eq!(resource.links.len(), 1);
        assert_eq!(resource.links[0].cost, 2.0);
    }

    #[test]
    fn navmesh_create_and_write_reject_invalid_data() {
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let res = ResourceWindow::new(api.as_ref());
        let invalid = NavMesh3D {
            vertices: vec![Vector3::new(0.0, 0.0, 0.0)],
            triangles: vec![NavMeshTriangle3D {
                vertices: [0, 1, 2],
                layers: BitMask::ALL,
            }],
        };
        assert!(res.NavMeshes().create(invalid.clone()).is_nil());

        let valid = NavMesh3D::try_new(
            vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 1.0),
            ],
            vec![NavMeshTriangle3D {
                vertices: [0, 1, 2],
                layers: BitMask::ALL,
            }],
        )
        .expect("test or bench setup must succeed");
        let id = res.NavMeshes().create(valid.clone());
        assert!(!id.is_nil());
        assert!(!res.NavMeshes().write(id, invalid));
        assert_eq!(res.NavMeshes().get_data(id), Some(valid));
    }
}
