// uid_registry.rs - In-memory asset UID tracking for editor session
//
// The UID registry is session-scoped and ephemeral:
// 1. On editor startup, load all scenes and register asset paths
// 2. Assign each path a UID (deterministic hash of path)
// 3. During editing, nodes reference UIDs internally
// 4. On rename, update UID→path mapping in memory
// 5. On scene save, resolve UIDs back to current paths
// 6. External scene edits are fine - next session will re-discover paths
//
// This approach avoids stale UID mappings and works with external editors.

use std::collections::HashMap;
use std::io;
use std::sync::RwLock;
use once_cell::sync::Lazy;
use uuid::Uuid;

use crate::project::asset_io::{resolve_path, ResolvedPath};

/// Asset UID - a unique identifier for each file in the project
/// Generated deterministically from the asset path (via hash)
pub type AssetUid = Uuid;

/// Metadata stored for each asset in memory
#[derive(Debug, Clone)]
pub struct AssetMetadata {
    /// Unique identifier for this asset
    pub uid: AssetUid,
    /// Current path relative to project root (e.g., "res://textures/player.png")
    pub path: String,
}

/// In-memory UID registry for the current editor session
/// NOT persisted to disk - regenerated on each project load
#[derive(Debug, Clone, Default)]
pub struct UidRegistry {
    /// Map from UID to asset metadata
    assets: HashMap<AssetUid, AssetMetadata>,
    /// Reverse map from path to UID for quick lookups
    path_to_uid: HashMap<String, AssetUid>,
}

impl UidRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
            path_to_uid: HashMap::new(),
        }
    }

    /// Register a new asset or get existing UID
    /// Uses deterministic UUID based on path hash (v5 namespace UUID)
    /// Returns the UID for the asset
    pub fn register_asset(&mut self, path: impl AsRef<str>) -> AssetUid {
        let path = path.as_ref().to_string();
        
        // Check if asset already exists
        if let Some(uid) = self.path_to_uid.get(&path) {
            return *uid;
        }

        // Create deterministic UID from path hash (using v5 UUID)
        // This ensures the same path gets the same UID across sessions
        let namespace = Uuid::NAMESPACE_URL;
        let uid = Uuid::new_v5(&namespace, path.as_bytes());
        
        let metadata = AssetMetadata {
            uid,
            path: path.clone(),
        };

        self.assets.insert(uid, metadata);
        self.path_to_uid.insert(path, uid);
        uid
    }

    /// Get the UID for a given path, or None if not registered
    pub fn get_uid(&self, path: impl AsRef<str>) -> Option<AssetUid> {
        self.path_to_uid.get(path.as_ref()).copied()
    }

    /// Get the path for a given UID, or None if not found
    pub fn get_path(&self, uid: AssetUid) -> Option<&str> {
        self.assets.get(&uid).map(|m| m.path.as_str())
    }

    /// Get asset metadata by UID
    pub fn get_metadata(&self, uid: AssetUid) -> Option<&AssetMetadata> {
        self.assets.get(&uid)
    }

    /// Rename/move an asset (updates the path for a given UID)
    /// Returns Ok(()) if successful, Err if the UID doesn't exist
    pub fn rename_asset(&mut self, uid: AssetUid, new_path: impl AsRef<str>) -> Result<(), String> {
        let new_path = new_path.as_ref().to_string();
        
        // Get the old metadata
        let metadata = self.assets.get_mut(&uid)
            .ok_or_else(|| format!("Asset UID {} not found in registry", uid))?;

        let old_path = metadata.path.clone();
        
        // Update path in metadata
        metadata.path = new_path.clone();

        // Update reverse mapping
        self.path_to_uid.remove(&old_path);
        self.path_to_uid.insert(new_path, uid);

        Ok(())
    }

    /// Remove an asset from the registry
    pub fn remove_asset(&mut self, uid: AssetUid) -> Option<AssetMetadata> {
        if let Some(metadata) = self.assets.remove(&uid) {
            self.path_to_uid.remove(&metadata.path);
            Some(metadata)
        } else {
            None
        }
    }

    /// Remove an asset by path
    pub fn remove_asset_by_path(&mut self, path: impl AsRef<str>) -> Option<AssetMetadata> {
        if let Some(uid) = self.path_to_uid.remove(path.as_ref()) {
            self.assets.remove(&uid)
        } else {
            None
        }
    }

    /// Get all assets in the registry
    pub fn get_all_assets(&self) -> impl Iterator<Item = &AssetMetadata> {
        self.assets.values()
    }

    /// Clear all assets from the registry
    pub fn clear(&mut self) {
        self.assets.clear();
        self.path_to_uid.clear();
    }
}

// Global in-memory registry instance (session-scoped)
static GLOBAL_UID_REGISTRY: Lazy<RwLock<Option<UidRegistry>>> = Lazy::new(|| RwLock::new(None));

/// Initialize the global UID registry for the current editor session
/// Creates an empty registry - assets are registered as scenes are loaded
pub fn init_uid_registry() -> io::Result<()> {
    let registry = UidRegistry::new();
    *GLOBAL_UID_REGISTRY.write().unwrap() = Some(registry);
    println!("✅ Initialized in-memory UID registry (session-scoped)");
    Ok(())
}

/// Get a reference to the global UID registry
pub fn get_uid_registry() -> std::sync::RwLockReadGuard<'static, Option<UidRegistry>> {
    GLOBAL_UID_REGISTRY.read().unwrap()
}

/// Get a mutable reference to the global UID registry
pub fn get_uid_registry_mut() -> std::sync::RwLockWriteGuard<'static, Option<UidRegistry>> {
    GLOBAL_UID_REGISTRY.write().unwrap()
}

/// Clear the global UID registry (called when closing project)
pub fn clear_uid_registry() {
    let mut registry = GLOBAL_UID_REGISTRY.write().unwrap();
    if let Some(ref mut reg) = *registry {
        reg.clear();
    }
}

/// Helper: Get UID for a path, registering it if needed
pub fn get_or_create_uid(path: impl AsRef<str>) -> Option<AssetUid> {
    let mut registry = GLOBAL_UID_REGISTRY.write().unwrap();
    if let Some(ref mut reg) = *registry {
        Some(reg.register_asset(path))
    } else {
        None
    }
}

/// Helper: Get path from UID
pub fn uid_to_path(uid: AssetUid) -> Option<String> {
    let registry = GLOBAL_UID_REGISTRY.read().unwrap();
    if let Some(ref reg) = *registry {
        reg.get_path(uid).map(|s| s.to_string())
    } else {
        None
    }
}

/// Helper: Rename an asset and update the file system
pub fn rename_asset_with_fs(uid: AssetUid, new_path: impl AsRef<str>) -> io::Result<()> {
    let new_path = new_path.as_ref();
    
    // Get old path from registry
    let old_path = {
        let registry = GLOBAL_UID_REGISTRY.read().unwrap();
        let reg = registry.as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "UID registry not initialized"))?;
        reg.get_path(uid)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Asset UID not found"))?
            .to_string()
    };

    // Resolve both paths to file system paths
    let old_fs_path = match resolve_path(&old_path) {
        ResolvedPath::Disk(p) => p,
        ResolvedPath::Brk(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot rename assets in BRK archives",
            ));
        }
    };

    let new_fs_path = match resolve_path(new_path) {
        ResolvedPath::Disk(p) => p,
        ResolvedPath::Brk(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot rename assets in BRK archives",
            ));
        }
    };

    // Create parent directory if needed
    if let Some(parent) = new_fs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Rename the file
    std::fs::rename(&old_fs_path, &new_fs_path)?;

    // Update in-memory registry
    let mut registry = GLOBAL_UID_REGISTRY.write().unwrap();
    if let Some(ref mut reg) = *registry {
        reg.rename_asset(uid, new_path)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uid_registry_basic() {
        let mut registry = UidRegistry::new();
        
        // Register an asset
        let uid1 = registry.register_asset("res://test.png");
        assert_eq!(registry.get_path(uid1), Some("res://test.png"));
        assert_eq!(registry.get_uid("res://test.png"), Some(uid1));
        
        // Register same asset again - should return same UID
        let uid2 = registry.register_asset("res://test.png");
        assert_eq!(uid1, uid2);
    }

    #[test]
    fn test_uid_registry_deterministic() {
        let mut registry1 = UidRegistry::new();
        let mut registry2 = UidRegistry::new();
        
        // Same path should generate same UID across different registries
        let uid1 = registry1.register_asset("res://test.png");
        let uid2 = registry2.register_asset("res://test.png");
        assert_eq!(uid1, uid2);
    }

    #[test]
    fn test_uid_registry_rename() {
        let mut registry = UidRegistry::new();
        
        let uid = registry.register_asset("res://old.png");
        registry.rename_asset(uid, "res://new.png").unwrap();
        
        assert_eq!(registry.get_path(uid), Some("res://new.png"));
        assert_eq!(registry.get_uid("res://new.png"), Some(uid));
        assert_eq!(registry.get_uid("res://old.png"), None);
    }

    #[test]
    fn test_uid_registry_remove() {
        let mut registry = UidRegistry::new();
        
        let uid = registry.register_asset("res://test.png");
        let removed = registry.remove_asset(uid);
        
        assert!(removed.is_some());
        assert_eq!(registry.get_path(uid), None);
        assert_eq!(registry.get_uid("res://test.png"), None);
    }
}
