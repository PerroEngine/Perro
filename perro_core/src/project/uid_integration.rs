// uid_integration.rs - Integration helpers for UID registry with project/scene loading
//
// The UID registry is session-scoped and in-memory only.
// Workflow:
// 1. On editor startup: init_project_uid_registry() creates empty registry
// 2. Load scenes: register_scene_assets() discovers asset paths and assigns UIDs
// 3. During editing: Nodes reference UIDs internally
// 4. On save: Resolve UIDs → paths and write to scene files
// 5. On editor close: Registry is discarded (session ends)

use std::io;
use crate::project::uid_registry::{init_uid_registry, get_uid_registry_mut};
use crate::scene::{Scene, SceneData};
use crate::nodes::node_registry::{SceneNode, BaseNode};
use crate::script::ScriptProvider;

/// Initialize the in-memory UID registry for the editor session
/// This should be called once when opening a project in the editor
pub fn init_project_uid_registry() -> io::Result<()> {
    init_uid_registry()?;
    println!("📝 UID registry ready - will populate as scenes are loaded");
    Ok(())
}

/// Register all asset paths in a scene with the UID registry
/// This ensures that all referenced assets get UIDs
pub fn register_scene_assets(scene_data: &SceneData) {
    let mut registered = 0;
    let mut registry = get_uid_registry_mut();
    
    if let Some(ref mut reg) = *registry {
        for (_idx, node) in scene_data.nodes.iter() {
            // Register script paths
            if let Some(script_path) = node.get_script_path() {
                if !script_path.is_empty() && script_path.starts_with("res://") {
                    reg.register_asset(script_path);
                    registered += 1;
                }
            }
            
            // Register texture paths (for Sprite2D and similar)
            if let Some(texture_path) = get_texture_path(node) {
                if !texture_path.is_empty() && texture_path.starts_with("res://") {
                    reg.register_asset(texture_path);
                    registered += 1;
                }
            }
            
            // Register mesh paths (for MeshInstance3D)
            if let Some(mesh_path) = get_mesh_path(node) {
                if !mesh_path.is_empty() && mesh_path.starts_with("res://") {
                    reg.register_asset(mesh_path);
                    registered += 1;
                }
            }
            
            // Register material paths
            if let Some(material_path) = get_material_path(node) {
                if !material_path.is_empty() && material_path.starts_with("res://") {
                    reg.register_asset(material_path);
                    registered += 1;
                }
            }
            
            // Register FUR paths (for UINode)
            if let SceneNode::UINode(ui_node) = node {
                if let Some(ref fur_path) = ui_node.fur_path {
                    if !fur_path.is_empty() && fur_path.starts_with("res://") {
                        reg.register_asset(fur_path.as_ref());
                        registered += 1;
                    }
                }
            }
        }
    }
    
    if registered > 0 {
        println!("📝 Registered {} asset references from scene", registered);
    }
}

/// Helper to get texture path from a node (if it has one)
fn get_texture_path(node: &SceneNode) -> Option<&str> {
    match node {
        SceneNode::Sprite2D(sprite) => sprite.texture_path.as_ref().map(|p| p.as_ref()),
        _ => None,
    }
}

/// Helper to get mesh path from a node (if it has one)
fn get_mesh_path(node: &SceneNode) -> Option<&str> {
    match node {
        SceneNode::MeshInstance3D(mesh) => mesh.mesh_path.as_ref().map(|p| p.as_ref()),
        _ => None,
    }
}

/// Helper to get material path from a node (if it has one)
fn get_material_path(node: &SceneNode) -> Option<&str> {
    match node {
        SceneNode::MeshInstance3D(mesh) => mesh.material_path.as_ref().map(|p| p.as_ref()),
        _ => None,
    }
}

/// Hook to be called after loading a scene
/// Registers all asset paths in the scene with the UID registry
pub fn on_scene_loaded(scene_data: &SceneData) {
    register_scene_assets(scene_data);
}

/// Hook to be called after loading a runtime scene
pub fn on_runtime_scene_loaded<P: ScriptProvider>(scene: &Scene<P>) {
    let scene_data = scene.to_scene_data();
    register_scene_assets(&scene_data);
}

/// Clear the UID registry (called when closing the project)
pub fn clear_project_uid_registry() {
    crate::project::uid_registry::clear_uid_registry();
    println!("🗑️  Cleared UID registry (session ended)");
}

/// Resolve a path using the UID registry if it's a UID reference
/// This allows using "uid://[uuid]" syntax in paths
pub fn resolve_uid_path(path: &str) -> String {
    if let Some(uid_str) = path.strip_prefix("uid://") {
        if let Ok(uid) = uuid::Uuid::parse_str(uid_str) {
            if let Some(resolved_path) = crate::project::uid_registry::uid_to_path(uid) {
                return resolved_path;
            }
        }
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_uid_path() {
        // Regular path should pass through
        assert_eq!(resolve_uid_path("res://test.png"), "res://test.png");
        
        // Invalid UID syntax should pass through
        assert_eq!(resolve_uid_path("uid://invalid"), "uid://invalid");
    }
}
