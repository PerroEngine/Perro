use crate::cns::terrain_store::TerrainStore;
use perro_terrain::TerrainData;

#[test]
fn terrain_store_reuses_slot_with_bumped_generation() {
    let mut store = TerrainStore::new();
    let first = store.insert(TerrainData::new(64.0));
    assert!(store.get(first).is_some());
    let removed = store.remove(first);
    assert!(removed.is_some());
    assert!(store.get(first).is_none());

    let second = store.insert(TerrainData::new(64.0));
    assert_eq!(second.index(), first.index());
    assert_ne!(second.generation(), first.generation());
    assert!(store.get(second).is_some());
}

#[test]
fn terrain_store_clear_invalidates_existing_ids() {
    let mut store = TerrainStore::new();
    let id = store.insert(TerrainData::new(64.0));
    assert!(store.get(id).is_some());
    store.clear();
    assert!(store.get(id).is_none());
}
