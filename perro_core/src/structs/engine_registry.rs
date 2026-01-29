use crate::{ast::*, engine_structs::EngineStruct, node_registry::NodeType};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};

/// Strongly-typed reference to a field in engine_registry
/// This is generated when fields are registered in engine_registry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeFieldRef {
    // Node fields
    NodeName,
    NodeId,

    // Node2D fields
    Node2DTransform,
    Node2DPivot,
    Node2DVisible,
    Node2DZIndex,

    // Node3D fields
    Node3DTransform,
    Node3DPivot,
    Node3DVisible,

    // Sprite2D fields
    Sprite2DTextureId,
    Sprite2DRegion,

    // Camera2D fields
    Camera2DZoom,
    Camera2DActive,

    // Shape2D fields
    Shape2DShapeType,
    Shape2DColor,
    Shape2DFilled,
    // Add more as needed when registering nodes...
}

/// Strongly-typed reference to a method in engine_registry
/// This is generated when methods are registered in engine_registry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeMethodRef {
    // NodeSugar methods (available on all nodes)
    GetVar,
    SetVar,
    CallFunction,
    GetChildByName,
    GetParent,
    AddChild,
    ClearChildren,
    GetType,
    GetParentType,
    Remove,
    // Add more as needed when registering node methods...
}

impl NodeMethodRef {
    /// Get the node type this method belongs to
    pub fn node_type(&self) -> Option<NodeType> {
        ENGINE_REGISTRY
            .method_ref_reverse_map
            .get(self)
            .map(|(node_type, _)| *node_type)
    }

    /// Get the Rust method name from engine_registry
    pub fn rust_method_name(&self) -> Option<String> {
        ENGINE_REGISTRY
            .method_ref_reverse_map
            .get(self)
            .map(|(_, method_name)| method_name.clone())
    }

    /// Get the return type from engine_registry
    pub fn return_type(&self) -> Option<Type> {
        if let Some((node_type, method_name)) = ENGINE_REGISTRY.method_ref_reverse_map.get(self) {
            ENGINE_REGISTRY.get_method_return_type(node_type, method_name)
        } else {
            None
        }
    }

    /// Get the parameter types from engine_registry
    pub fn param_types(&self) -> Option<Vec<Type>> {
        if let Some((node_type, method_name)) = ENGINE_REGISTRY.method_ref_reverse_map.get(self) {
            ENGINE_REGISTRY.get_method_param_types(node_type, method_name)
        } else {
            None
        }
    }

    /// Get the parameter names from engine_registry
    /// Returns script-side parameter names (what PUP users see), not internal Rust names
    pub fn param_names(&self) -> Option<Vec<&'static str>> {
        if let Some((node_type, method_name)) = ENGINE_REGISTRY.method_ref_reverse_map.get(self) {
            ENGINE_REGISTRY.get_method_param_names(node_type, method_name)
        } else {
            None
        }
    }
}

impl NodeFieldRef {
    /// Get the node type this field belongs to
    /// Queries the engine registry for the actual mapping
    pub fn node_type(&self) -> Option<NodeType> {
        ENGINE_REGISTRY
            .field_ref_reverse_map
            .get(self)
            .map(|(node_type, _)| *node_type)
    }

    /// Get the Rust field name from engine_registry
    /// Queries the engine registry for the actual mapping
    pub fn rust_field_name(&self) -> Option<String> {
        ENGINE_REGISTRY
            .field_ref_reverse_map
            .get(self)
            .map(|(_, field_name)| field_name.clone())
    }

    /// Get the Rust type from engine_registry
    /// This looks up the actual field type from the registry
    pub fn rust_type(&self) -> Option<Type> {
        if let Some((node_type, field_name)) = ENGINE_REGISTRY.field_ref_reverse_map.get(self) {
            ENGINE_REGISTRY.get_field_type_node(node_type, field_name)
        } else {
            None
        }
    }
}

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

    // Field reference mapping: (NodeType, rust_field_name) -> NodeFieldRef
    // Maps engine registry fields to their strongly-typed references
    pub field_ref_map: HashMap<(NodeType, String), NodeFieldRef>,

    // Reverse mapping: NodeFieldRef -> (NodeType, rust_field_name)
    // Allows looking up the node type and field name from a NodeFieldRef
    pub field_ref_reverse_map: HashMap<NodeFieldRef, (NodeType, String)>,

    // Method name mapping: (NodeType, script_method_name) -> rust_method_name
    // Allows scripts to use "get_node" while Rust might use different names
    pub method_name_map: HashMap<(NodeType, String), String>,

    // Method reference mapping: (NodeType, rust_method_name) -> NodeMethodRef
    // Maps engine registry methods to their strongly-typed references
    pub method_ref_map: HashMap<(NodeType, String), NodeMethodRef>,

    // Reverse mapping: NodeMethodRef -> (NodeType, rust_method_name)
    // Allows looking up the node type and method name from a NodeMethodRef
    pub method_ref_reverse_map: HashMap<NodeMethodRef, (NodeType, String)>,

    // Method definitions: (NodeType, rust_method_name) -> (params, return_type, param_names)
    // Stores the actual method signature with script-side parameter names
    pub method_defs: HashMap<(NodeType, String), (Vec<Type>, Type, Vec<&'static str>)>,
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
            field_ref_map: HashMap::new(),
            field_ref_reverse_map: HashMap::new(),
            method_name_map: HashMap::new(),
            method_ref_map: HashMap::new(),
            method_ref_reverse_map: HashMap::new(),
            method_defs: HashMap::new(),
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

        // Shape2D is an enum - enums can't have fields like structs
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

        // Texture - represents a TextureID handle to a texture in TextureManager
        // No fields - it's just a TextureID handle (like nodes use NodeID)
        reg.register_struct(
            EngineStruct::Texture,
            None,
            vec![], // No fields - it's a NodeID (u64) handle
        );

        //--------------------------------
        // Nodes
        //--------------------------------
        // Base Node type - all nodes inherit from this
        // Note: local_id, dirty, script_exp_vars, metadata are internal and not registered
        // All fields here are internal Rust representation - what scripts see is defined in PupNodeApiRegistry
        // Base Node - only script-accessible fields
        // Internal fields like parent, children, script_path, is_root_of are not exposed to scripts
        reg.register_node(
            NodeType::Node,
            None,
            vec![
                ("name", Type::CowStr, Some(NodeFieldRef::NodeName)), // Rust type: CowStr (scripts see this directly, no conversion needed)
                ("id", Type::DynNode, Some(NodeFieldRef::NodeId)),
            ],
        );

        // Register NodeSugar methods (available on all nodes)
        // These are "built-in" methods that don't exist as real Rust methods but are exposed to scripts
        // Parameter names are script-side (what PUP users see), not internal Rust names
        reg.register_node_methods(
            NodeType::Node,
            vec![
                (
                    "get_var",
                    vec![Type::String],
                    Type::Any,
                    Some(NodeMethodRef::GetVar),
                    vec!["name"],
                ),
                (
                    "set_var",
                    vec![Type::String, Type::Object],
                    Type::Void,
                    Some(NodeMethodRef::SetVar),
                    vec!["name", "value"],
                ),
                (
                    "get_node",
                    vec![Type::String],
                    Type::DynNode,
                    Some(NodeMethodRef::GetChildByName),
                    vec!["name"],
                ),
                (
                    "get_parent",
                    vec![],
                    Type::DynNode,
                    Some(NodeMethodRef::GetParent),
                    vec![],
                ),
                (
                    "add_child",
                    vec![Type::Node(NodeType::Node)],
                    Type::Void,
                    Some(NodeMethodRef::AddChild),
                    vec!["node"],
                ),
                (
                    "clear_children",
                    vec![],
                    Type::Void,
                    Some(NodeMethodRef::ClearChildren),
                    vec![],
                ),
                (
                    "get_type",
                    vec![],
                    Type::NodeType,
                    Some(NodeMethodRef::GetType),
                    vec![],
                ),
                (
                    "get_parent_type",
                    vec![],
                    Type::NodeType,
                    Some(NodeMethodRef::GetParentType),
                    vec![],
                ),
                (
                    "remove",
                    vec![],
                    Type::Void,
                    Some(NodeMethodRef::Remove),
                    vec![],
                ),
            ],
        );

        // Node2D - inherits from Node, adds transform
        // Note: global_transform and transform_dirty are runtime-only and not registered
        reg.register_node(
            NodeType::Node2D,
            Some(NodeType::Node),
            vec![
                (
                    "transform",
                    Type::EngineStruct(EngineStruct::Transform2D),
                    Some(NodeFieldRef::Node2DTransform),
                ),
                (
                    "pivot",
                    Type::EngineStruct(EngineStruct::Vector2),
                    Some(NodeFieldRef::Node2DPivot),
                ),
                ("visible", Type::Bool, Some(NodeFieldRef::Node2DVisible)),
                (
                    "z_index",
                    Type::Number(NumberKind::Signed(32)),
                    Some(NodeFieldRef::Node2DZIndex),
                ),
            ],
        );

        // Sprite2D - inherits from Node2D
        // Only script-accessible fields are registered here
        // Internal fields like texture_path (for serialization) are not exposed to scripts
        reg.register_node(
            NodeType::Sprite2D,
            Some(NodeType::Node2D),
            vec![
                (
                    "texture_id",
                    Type::EngineStruct(EngineStruct::Texture),
                    Some(NodeFieldRef::Sprite2DTextureId),
                ), // Semantic type: Texture (Rust conversion: becomes Option<TextureID>)
                (
                    "region",
                    Type::Option(Box::new(Type::Container(
                        ContainerKind::FixedArray(4),
                        vec![Type::Number(NumberKind::Float(32))],
                    ))),
                    Some(NodeFieldRef::Sprite2DRegion),
                ),
            ],
        );

        // Area2D - inherits from Node2D
        reg.register_node(NodeType::Area2D, Some(NodeType::Node2D), vec![]);

        // CollisionShape2D - inherits from Node2D
        reg.register_node(
            NodeType::CollisionShape2D,
            Some(NodeType::Node2D),
            vec![(
                "shape",
                Type::Option(Box::new(Type::EngineStruct(EngineStruct::Shape2D))),
                None,
            )],
        );

        // ShapeInstance2D - inherits from Node2D
        reg.register_node(
            NodeType::ShapeInstance2D,
            Some(NodeType::Node2D),
            vec![
                (
                    "shape",
                    Type::Option(Box::new(Type::EngineStruct(EngineStruct::Shape2D))),
                    Some(NodeFieldRef::Shape2DShapeType),
                ),
                (
                    "color",
                    Type::Option(Box::new(Type::EngineStruct(EngineStruct::Color))),
                    Some(NodeFieldRef::Shape2DColor),
                ),
                ("filled", Type::Bool, Some(NodeFieldRef::Shape2DFilled)),
            ],
        );

        // Camera2D - inherits from Node2D
        reg.register_node(
            NodeType::Camera2D,
            Some(NodeType::Node2D),
            vec![
                (
                    "zoom",
                    Type::Number(NumberKind::Float(32)),
                    Some(NodeFieldRef::Camera2DZoom),
                ),
                ("active", Type::Bool, Some(NodeFieldRef::Camera2DActive)),
            ],
        );

        // UINode - inherits from Node
        reg.register_node(
            NodeType::UINode,
            Some(NodeType::Node),
            vec![
                ("visible", Type::Bool, None),
                ("fur_path", Type::Option(Box::new(Type::CowStr)), None),
            ],
        );

        // Node3D - inherits from Node
        reg.register_node(
            NodeType::Node3D,
            Some(NodeType::Node),
            vec![
                (
                    "transform",
                    Type::EngineStruct(EngineStruct::Transform3D),
                    None,
                ),
                ("pivot", Type::EngineStruct(EngineStruct::Vector3), None),
                ("visible", Type::Bool, None),
            ],
        );

        // MeshInstance3D - inherits from Node3D
        reg.register_node(
            NodeType::MeshInstance3D,
            Some(NodeType::Node3D),
            vec![
                ("mesh_path", Type::Option(Box::new(Type::CowStr)), None),
                ("material_path", Type::Option(Box::new(Type::CowStr)), None),
                (
                    "material_id",
                    Type::Option(Box::new(Type::Number(NumberKind::Unsigned(32)))),
                    None,
                ),
            ],
        );

        // Camera3D - inherits from Node3D
        reg.register_node(
            NodeType::Camera3D,
            Some(NodeType::Node3D),
            vec![
                (
                    "fov",
                    Type::Option(Box::new(Type::Number(NumberKind::Float(32)))),
                    None,
                ),
                (
                    "near",
                    Type::Option(Box::new(Type::Number(NumberKind::Float(32)))),
                    None,
                ),
                (
                    "far",
                    Type::Option(Box::new(Type::Number(NumberKind::Float(32)))),
                    None,
                ),
                ("active", Type::Bool, None),
            ],
        );

        // DirectionalLight3D - inherits from Node3D
        reg.register_node(
            NodeType::DirectionalLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color), None),
                ("intensity", Type::Number(NumberKind::Float(32)), None),
            ],
        );

        // OmniLight3D - inherits from Node3D
        reg.register_node(
            NodeType::OmniLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color), None),
                ("intensity", Type::Number(NumberKind::Float(32)), None),
                ("range", Type::Number(NumberKind::Float(32)), None),
            ],
        );

        // SpotLight3D - inherits from Node3D
        reg.register_node(
            NodeType::SpotLight3D,
            Some(NodeType::Node3D),
            vec![
                ("color", Type::EngineStruct(EngineStruct::Color), None),
                ("intensity", Type::Number(NumberKind::Float(32)), None),
                ("range", Type::Number(NumberKind::Float(32)), None),
                ("inner_angle", Type::Number(NumberKind::Float(32)), None),
                ("outer_angle", Type::Number(NumberKind::Float(32)), None),
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
        fields: Vec<(&str, Type)>, // (name, type) - all internal Rust representation
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
        fields: Vec<(&str, Type, Option<NodeFieldRef>)>, // (name, type, optional_field_ref) - all internal Rust representation
    ) {
        if let Some(b) = base {
            self.node_bases.insert(kind.clone(), b);
        }

        let def = StructDef {
            name: format!("{:?}", kind),
            base: None, // we don't use StructDef.base for engine nodes
            fields: fields
                .iter()
                .map(|(n, t, _)| StructField {
                    name: (*n).into(),
                    typ: t.clone(),
                    attributes: vec![],
                })
                .collect(),
            methods: vec![],
        };
        self.node_defs.insert(kind, def);

        // Create field reference mappings (both directions) - only for fields with NodeFieldRef
        for (field_name, _, field_ref_opt) in fields {
            if let Some(field_ref) = field_ref_opt {
                let field_name_str = field_name.to_string();
                // Forward: (NodeType, rust_field_name) -> NodeFieldRef
                self.field_ref_map
                    .insert((kind, field_name_str.clone()), field_ref);
                // Reverse: NodeFieldRef -> (NodeType, rust_field_name)
                self.field_ref_reverse_map
                    .insert(field_ref, (kind, field_name_str));
            }
        }
    }

    pub fn register_node_methods(
        &mut self,
        kind: NodeType,
        methods: Vec<(
            &str,
            Vec<Type>,
            Type,
            Option<NodeMethodRef>,
            Vec<&'static str>,
        )>, // (script_name, params, return_type, optional_method_ref, param_names)
    ) {
        for (script_name, params, return_type, method_ref_opt, param_names) in methods {
            // Store method definition: (NodeType, rust_method_name) -> (params, return_type, param_names)
            // For built-in methods, rust_method_name is the same as script_name
            // param_names are script-side names (what PUP users see), not internal Rust names
            let rust_method_name = script_name.to_string();
            self.method_defs.insert(
                (kind, rust_method_name.clone()),
                (params.clone(), return_type, param_names),
            );

            // Create method name mapping: (NodeType, script_name) -> rust_method_name
            self.method_name_map
                .insert((kind, script_name.to_string()), rust_method_name.clone());

            // Create method reference mappings (both directions) - only for methods with NodeMethodRef
            if let Some(method_ref) = method_ref_opt {
                // Forward: (NodeType, rust_method_name) -> NodeMethodRef
                self.method_ref_map
                    .insert((kind, rust_method_name.clone()), method_ref);
                // Reverse: NodeMethodRef -> (NodeType, rust_method_name)
                self.method_ref_reverse_map
                    .insert(method_ref, (kind, rust_method_name));
            }
        }
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
        // First, resolve the script field name to the Rust field name
        // This handles mappings like "texture" -> "texture_id"
        let rust_field = self.resolve_field_name(node_kind, field);

        // Now look up the type using the resolved Rust field name
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            if let Some(def) = self.node_defs.get(&kind) {
                if let Some(f) = def.fields.iter().find(|f| f.name == rust_field) {
                    return Some(f.typ.clone());
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
        // Convert to Vec and sort so 3D nodes come first (wider types), then 2D, then by name for stability
        let mut result: Vec<NodeType> = result_set.into_iter().collect();
        self.sort_node_types_3d_first(&mut result);
        result
    }

    /// Returns true if node_type has the given base in its ancestry (or is the base).
    pub fn has_base(&self, node_type: &NodeType, base: NodeType) -> bool {
        let mut current = Some(*node_type);
        while let Some(nt) = current {
            if nt == base {
                return true;
            }
            current = self.node_bases.get(&nt).copied();
        }
        false
    }

    /// Sort node types so 3D nodes come first, then 2D, then others; within each group sort by debug name for stability.
    pub fn sort_node_types_3d_first(&self, types: &mut [NodeType]) {
        types.sort_by(|a, b| {
            let key = |nt: &NodeType| {
                let is_3d = self.has_base(nt, NodeType::Node3D);
                let is_2d = self.has_base(nt, NodeType::Node2D);
                let order = match (is_3d, is_2d) {
                    (true, _) => 0u8,
                    (_, true) => 1,
                    _ => 2,
                };
                (order, format!("{:?}", nt))
            };
            key(a).cmp(&key(b))
        });
    }

    /// Find the intersection of node types that have multiple field paths
    /// This is used to narrow down types when accessing multiple fields on the same dynamic node
    /// Returns the intersection of all compatible types for all field paths
    pub fn intersect_nodes_by_fields(&self, field_paths: &[Vec<String>]) -> Vec<NodeType> {
        if field_paths.is_empty() {
            return self.node_defs.keys().copied().collect();
        }

        // Get compatible types for each field path
        let type_sets: Vec<HashSet<NodeType>> = field_paths
            .iter()
            .map(|fields| {
                let types = self.narrow_nodes_by_fields(fields);
                types.into_iter().collect()
            })
            .collect();

        // Find intersection of all sets
        if let Some(first_set) = type_sets.first() {
            let mut intersection = first_set.clone();
            for other_set in type_sets.iter().skip(1) {
                intersection = intersection.intersection(other_set).copied().collect();
            }
            intersection.into_iter().collect()
        } else {
            Vec::new()
        }
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
        // First check if we have a cached mapping
        if let Some(mapped) = self
            .field_name_map
            .get(&(*node_type, script_field.to_string()))
        {
            return mapped.clone();
        }

        // If not found in cache, check PUP_NODE_API directly
        // This is the source of truth for script field name -> Rust field name mappings
        use crate::scripting::lang::pup::node_api::PUP_NODE_API;

        // Get all fields for this node type (including inherited)
        let fields = PUP_NODE_API.get_fields(node_type);

        // Find the field with matching script_name
        if let Some(api_field) = fields.iter().find(|f| f.script_name == script_field) {
            // Get the Rust field name from the NodeFieldRef
            if let Some(rust_field_name) = api_field.rust_field.rust_field_name() {
                // Only return mapped name if it differs from script name
                if rust_field_name != script_field {
                    return rust_field_name;
                }
            }
        }

        // If no mapping found, return the original field name
        // This handles cases where the script field name matches the Rust field name
        script_field.to_string()
    }

    /// Resolve script method name to Rust method name
    /// Similar to resolve_field_name but for methods
    pub fn resolve_method_name(&self, node_type: &NodeType, script_method: &str) -> String {
        // First check if we have a cached mapping
        if let Some(mapped) = self
            .method_name_map
            .get(&(*node_type, script_method.to_string()))
        {
            return mapped.clone();
        }

        // If not found, return the original method name
        script_method.to_string()
    }

    /// Get method return type from engine_registry
    pub fn get_method_return_type(&self, node_kind: &NodeType, method: &str) -> Option<Type> {
        // Resolve script method name to Rust method name
        let rust_method = self.resolve_method_name(node_kind, method);

        // Look up in method_defs
        if let Some((_, return_type, _)) = self.method_defs.get(&(*node_kind, rust_method)) {
            return Some(return_type.clone());
        }

        // Walk up inheritance chain
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            let rust_method = self.resolve_method_name(&kind, method);
            if let Some((_, return_type, _)) = self.method_defs.get(&(kind, rust_method)) {
                return Some(return_type.clone());
            }
            current = self.node_bases.get(&kind).cloned();
        }
        None
    }

    /// Get method parameter types from engine_registry
    pub fn get_method_param_types(&self, node_kind: &NodeType, method: &str) -> Option<Vec<Type>> {
        // Resolve script method name to Rust method name
        let rust_method = self.resolve_method_name(node_kind, method);

        // Look up in method_defs
        if let Some((params, _, _)) = self.method_defs.get(&(*node_kind, rust_method)) {
            return Some(params.clone());
        }

        // Walk up inheritance chain
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            let rust_method = self.resolve_method_name(&kind, method);
            if let Some((params, _, _)) = self.method_defs.get(&(kind, rust_method)) {
                return Some(params.clone());
            }
            current = self.node_bases.get(&kind).cloned();
        }
        None
    }

    /// Get method parameter names from engine_registry
    /// Returns script-side parameter names (what PUP users see), not internal Rust names
    pub fn get_method_param_names(
        &self,
        node_kind: &NodeType,
        method: &str,
    ) -> Option<Vec<&'static str>> {
        // Resolve script method name to Rust method name
        let rust_method = self.resolve_method_name(node_kind, method);

        // Look up in method_defs
        if let Some((_, _, param_names)) = self.method_defs.get(&(*node_kind, rust_method)) {
            return Some(param_names.clone());
        }

        // Walk up inheritance chain
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            let rust_method = self.resolve_method_name(&kind, method);
            if let Some((_, _, param_names)) = self.method_defs.get(&(kind, rust_method)) {
                return Some(param_names.clone());
            }
            current = self.node_bases.get(&kind).cloned();
        }
        None
    }
}
