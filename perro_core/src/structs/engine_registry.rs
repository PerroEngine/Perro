use std::collections::{HashMap, HashSet};
use once_cell::sync::Lazy;
use crate::{ast::*, engine_structs::EngineStruct, node_registry::NodeType};

#[derive(Debug, Clone)]
pub struct EngineRegistry {
    pub node_defs: HashMap<NodeType, StructDef>,
    pub struct_defs: HashMap<EngineStruct, StructDef>,

    // small maps to store inheritance chains
    pub node_bases: HashMap<NodeType, NodeType>,
    pub struct_bases: HashMap<EngineStruct, EngineStruct>,
    
    // Field name mapping: (NodeType, script_field_name) -> rust_field_name
    // Allows scripts to use "texture" while Rust uses "texture_id"
    pub field_name_map: HashMap<(NodeType, String), String>,
}

/// Global engine registry - initialized once, used during transpilation
pub static ENGINE_REGISTRY: Lazy<EngineRegistry> = Lazy::new(|| EngineRegistry::new());

impl EngineRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            node_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            node_bases: HashMap::new(),
            struct_bases: HashMap::new(),
            field_name_map: HashMap::new(),
        };

        //--------------------------------
        // Engine Structs
        //--------------------------------
        reg.register_struct(
            EngineStruct::Vector2,
            None,
            vec![
                ("x", Type::Number(NumberKind::Float(32))),
                ("y", Type::Number(NumberKind::Float(32))),
            ],
        );

        reg.register_struct(
            EngineStruct::Transform2D,
            None,
            vec![
                ("position", Type::EngineStruct(EngineStruct::Vector2)),
                ("rotation", Type::Number(NumberKind::Float(32))),
                ("scale", Type::EngineStruct(EngineStruct::Vector2)),
            ],
        );

        reg.register_struct(
            EngineStruct::Color,
            None,
            vec![
                ("r", Type::Number(NumberKind::Float(32))),
                ("g", Type::Number(NumberKind::Float(32))),
                ("b", Type::Number(NumberKind::Float(32))),
                ("a", Type::Number(NumberKind::Float(32))),
            ],
        );

        reg.register_struct(
            EngineStruct::Rect,
            None,
            vec![
                ("x", Type::Number(NumberKind::Float(32))),
                ("y", Type::Number(NumberKind::Float(32))),
                ("w", Type::Number(NumberKind::Float(32))),
                ("h", Type::Number(NumberKind::Float(32))),
            ],
        );

        // ShapeType2D is an enum - enums can't have fields like structs
        // We register it as an EngineStruct for type checking, but it has no fields
        // The enum variants are: Rectangle { width, height }, Circle { radius }, Square { size }, Triangle { base, height }

        // 3D structs
        reg.register_struct(
            EngineStruct::Vector3,
            None,
            vec![
                ("x", Type::Number(NumberKind::Float(32))),
                ("y", Type::Number(NumberKind::Float(32))),
                ("z", Type::Number(NumberKind::Float(32))),
            ],
        );

        reg.register_struct(
            EngineStruct::Quaternion,
            None,
            vec![
                ("x", Type::Number(NumberKind::Float(32))),
                ("y", Type::Number(NumberKind::Float(32))),
                ("z", Type::Number(NumberKind::Float(32))),
                ("w", Type::Number(NumberKind::Float(32))),
            ],
        );

        reg.register_struct(
            EngineStruct::Transform3D,
            None,
            vec![
                ("position", Type::EngineStruct(EngineStruct::Vector3)),
                ("rotation", Type::EngineStruct(EngineStruct::Quaternion)),
                ("scale", Type::EngineStruct(EngineStruct::Vector3)),
            ],
        );

        // Texture - represents a Uuid handle to a texture in TextureManager
        // No fields - it's just a Type::Uuid handle (like nodes)
        reg.register_struct(
            EngineStruct::Texture,
            None,
            vec![], // No fields - it's a Uuid handle
        );

        //--------------------------------
        // Nodes
        //--------------------------------
        // Base Node type - all nodes inherit from this
        // Note: local_id, dirty, script_exp_vars, metadata are internal and not registered
        reg.register_node(
            NodeType::Node,
            None,
            vec![
                ("name", Type::CowStr), // Cow<'static, str>
                ("id", Type::Uuid),
                ("parent", Type::Option(Box::new(Type::Custom("ParentType".into())))), // Option<ParentType> with id and node_type
                ("children", Type::Container(ContainerKind::Array, vec![Type::Uuid])),
                ("script_path", Type::Option(Box::new(Type::CowStr))),
                ("is_root_of", Type::Option(Box::new(Type::CowStr))),
            ],
        );

        // Node2D - inherits from Node, adds transform
        // Note: global_transform and transform_dirty are runtime-only and not registered
        reg.register_node(
            NodeType::Node2D,
            Some(NodeType::Node),
            vec![
                ("transform", Type::EngineStruct(EngineStruct::Transform2D)),
                ("pivot", Type::EngineStruct(EngineStruct::Vector2)),
                ("visible", Type::Bool),
                ("z_index", Type::Number(NumberKind::Signed(32))),
            ],
        );

        // Sprite2D - inherits from Node2D
        reg.register_node(
            NodeType::Sprite2D,
            Some(NodeType::Node2D),
            vec![
                ("texture_id", Type::Option(Box::new(Type::Uuid))), // Script-accessible texture handle
                ("texture_path", Type::Option(Box::new(Type::CowStr))), // Internal: for scene serialization
                ("region", Type::Option(Box::new(Type::Container(ContainerKind::FixedArray(4), vec![Type::Number(NumberKind::Float(32))])))),
            ],
        );
        
        // Add field name mapping: scripts see "texture", Rust uses "texture_id"
        reg.field_name_map.insert((NodeType::Sprite2D, "texture".to_string()), "texture_id".to_string());

        // Area2D - inherits from Node2D
        reg.register_node(
            NodeType::Area2D,
            Some(NodeType::Node2D),
            vec![],
        );

        // CollisionShape2D - inherits from Node2D
        reg.register_node(
            NodeType::CollisionShape2D,
            Some(NodeType::Node2D),
            vec![
                ("shape", Type::Option(Box::new(Type::Custom("ColliderShape".into())))),
            ],
        );

        // Shape2D - inherits from Node2D
        reg.register_node(
            NodeType::Shape2D,
            Some(NodeType::Node2D),
            vec![
                ("shape_type", Type::Option(Box::new(Type::EngineStruct(EngineStruct::ShapeType2D)))),
                ("color", Type::Option(Box::new(Type::EngineStruct(EngineStruct::Color)))),
                ("filled", Type::Bool),
            ],
        );

        // Camera2D - inherits from Node2D
        reg.register_node(
            NodeType::Camera2D,
            Some(NodeType::Node2D),
            vec![
                ("zoom", Type::Number(NumberKind::Float(32))),
                ("active", Type::Bool),
            ],
        );

        // UINode - inherits from Node
        reg.register_node(
            NodeType::UINode,
            Some(NodeType::Node),
            vec![
                ("visible", Type::Bool),
                ("fur_path", Type::Option(Box::new(Type::CowStr))),
            ],
        );

        // Node3D - inherits from Node
        reg.register_node(
            NodeType::Node3D,
            Some(NodeType::Node),
            vec![
                ("transform", Type::EngineStruct(EngineStruct::Transform3D)),
                ("pivot", Type::EngineStruct(EngineStruct::Vector3)),
                ("visible", Type::Bool),
            ],
        );

        // MeshInstance3D - inherits from Node3D
        reg.register_node(
            NodeType::MeshInstance3D,
            Some(NodeType::Node3D),
            vec![
                ("mesh_path", Type::Option(Box::new(Type::CowStr))),
                ("material_path", Type::Option(Box::new(Type::CowStr))),
                ("material_id", Type::Option(Box::new(Type::Number(NumberKind::Unsigned(32))))),
            ],
        );

        // Camera3D - inherits from Node3D
        reg.register_node(
            NodeType::Camera3D,
            Some(NodeType::Node3D),
            vec![
                ("fov", Type::Option(Box::new(Type::Number(NumberKind::Float(32))))),
                ("near", Type::Option(Box::new(Type::Number(NumberKind::Float(32))))),
                ("far", Type::Option(Box::new(Type::Number(NumberKind::Float(32))))),
                ("active", Type::Bool),
            ],
        );

        // DirectionalLight3D - inherits from Node3D
        reg.register_node(
            NodeType::DirectionalLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color)),
                ("intensity", Type::Number(NumberKind::Float(32))),
            ],
        );

        // OmniLight3D - inherits from Node3D
        reg.register_node(
            NodeType::OmniLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color)),
                ("intensity", Type::Number(NumberKind::Float(32))),
                ("range", Type::Number(NumberKind::Float(32))),
            ],
        );

        // SpotLight3D - inherits from Node3D
        reg.register_node(
            NodeType::SpotLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color)),
                ("intensity", Type::Number(NumberKind::Float(32))),
                ("range", Type::Number(NumberKind::Float(32))),
                ("inner_angle", Type::Number(NumberKind::Float(32))),
                ("outer_angle", Type::Number(NumberKind::Float(32))),
            ],
        );

        reg
    }

    // ---------------------------------------------------
    // Registration helpers
    // ---------------------------------------------------

    pub fn register_struct(
        &mut self,
        kind: EngineStruct,
        base: Option<EngineStruct>,
        fields: Vec<(&str, Type)>,
    ) {
        if let Some(b) = base {
            self.struct_bases.insert(kind.clone(), b);
        }

        let def = StructDef {
            name: format!("{:?}", kind),
            base: None,
            fields: fields
                .into_iter()
                .map(|(n, t)| StructField {
                    name: n.into(),
                    typ: t,
                    attributes: vec![],
                })
                .collect(),
            methods: vec![],
        };
        self.struct_defs.insert(kind, def);
    }

    pub fn register_node(
        &mut self,
        kind: NodeType,
        base: Option<NodeType>,
        fields: Vec<(&str, Type)>,
    ) {
        if let Some(b) = base {
            self.node_bases.insert(kind.clone(), b);
        }

        let def = StructDef {
            name: format!("{:?}", kind),
            base: None, // we don't use StructDef.base for engine nodes
            fields: fields
                .into_iter()
                .map(|(n, t)| StructField {
                    name: n.into(),
                    typ: t,
                    attributes: vec![],
                })
                .collect(),
            methods: vec![],
        };
        self.node_defs.insert(kind, def);
    }

    // ---------------------------------------------------
    // Reflection lookups
    // ---------------------------------------------------

    pub fn get_field_type_struct(&self, struct_kind: &EngineStruct, field: &str) -> Option<Type> {
        let mut current = Some(struct_kind.clone());
        while let Some(kind) = current {
            if let Some(def) = self.struct_defs.get(&kind) {
                if let Some(f) = def.fields.iter().find(|f| f.name == field) {
                    return Some(f.typ.clone());
                }
            }
            current = self.struct_bases.get(&kind).cloned();
        }
        None
    }

    pub fn get_field_type_node(&self, node_kind: &NodeType, field: &str) -> Option<Type> {
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            if let Some(def) = self.node_defs.get(&kind) {
                // First, try the field name as-is (in case it's already the Rust field name)
                if let Some(f) = def.fields.iter().find(|f| f.name == field) {
                    return Some(f.typ.clone());
                }
                // If not found, check if there's a field name mapping for this node type
                // This handles script field names like "texture" -> "texture_id"
                if let Some(mapped_field) = self.field_name_map.get(&(kind, field.to_string())) {
                    if let Some(f) = def.fields.iter().find(|f| &f.name == mapped_field) {
                        return Some(f.typ.clone());
                    }
                }
            }
            current = self.node_bases.get(&kind).cloned();
        }
        None
    }

    /// Walk through nested fields like Node2D.transform.position.x
    pub fn resolve_chain_from_node(&self, node_kind: &NodeType, chain: &[String]) -> Option<Type> {
        let mut current_type = Type::Node(node_kind.clone());
        for field in chain {
            current_type = match current_type {
                Type::Node(ref n) => self.get_field_type_node(n, field)?,
                Type::EngineStruct(ref s) => self.get_field_type_struct(s, field)?,
                _ => return None,
            };
        }
        Some(current_type)
    }

    /// Get all node types that have a specific field (including through inheritance)
    pub fn find_nodes_with_field(&self, field: &str) -> Vec<NodeType> {
        let mut result = Vec::new();
        for node_type in self.node_defs.keys() {
            if self.get_field_type_node(node_type, field).is_some() {
                result.push(*node_type);
            }
        }
        result
    }

    /// Get all descendants of a node type (including the type itself)
    pub fn get_descendants(&self, node_type: &NodeType) -> Vec<NodeType> {
        let mut result = vec![*node_type];
        // Find all nodes that have this node type in their ancestry chain
        for (child_type, base_type) in &self.node_bases {
            if *base_type == *node_type {
                // This is a direct child, recursively get its descendants
                result.extend(self.get_descendants(child_type));
            }
        }
        result
    }

    /// Narrow node types based on field access patterns
    /// Returns a list of node types that have the specified field path (handles nested paths)
    pub fn narrow_nodes_by_fields(&self, fields: &[String]) -> Vec<NodeType> {
        if fields.is_empty() {
            // No fields specified, return all node types
            return self.node_defs.keys().copied().collect();
        }

        // Check if the entire field path is valid for each node type
        // This properly handles nested paths like ["transform", "position", "x"]
        let mut candidates = Vec::new();
        for node_type in self.node_defs.keys() {
            if self.check_field_path(node_type, fields).is_some() {
                candidates.push(*node_type);
            }
        }

        // Expand to include all descendants of matching nodes
        let mut result_set = HashSet::new();
        for candidate in candidates {
            for descendant in self.get_descendants(&candidate) {
                result_set.insert(descendant);
            }
        }
        // Convert HashSet to Vec (order is not important)
        result_set.into_iter().collect()
    }

    /// Check if a field path (e.g., ["transform", "position", "z"]) is valid for a node type
    /// Returns Some(Type) if valid, None otherwise
    pub fn check_field_path(&self, node_type: &NodeType, path: &[String]) -> Option<Type> {
        self.resolve_chain_from_node(node_type, path)
    }

    /// Resolve a script field name to the actual Rust field name
    /// Returns the mapped field name if a mapping exists, otherwise returns the original name
    /// Example: "texture" -> "texture_id" for Sprite2D
    pub fn resolve_field_name(&self, node_type: &NodeType, script_field: &str) -> String {
        self.field_name_map
            .get(&(*node_type, script_field.to_string()))
            .cloned()
            .unwrap_or_else(|| script_field.to_string())
    }
}
