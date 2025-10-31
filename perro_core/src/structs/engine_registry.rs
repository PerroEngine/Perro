use std::collections::HashMap;

use crate::{
    engine_structs::EngineStruct,
    lang::ast::{NumberKind, StructDef, StructField, Type}, node_registry::NodeType,
};

#[derive(Debug, Clone)]
pub struct EngineRegistry {
    pub node_defs: HashMap<NodeType, StructDef>,
    pub struct_defs: HashMap<EngineStruct, StructDef>,

    // small maps to store inheritance chains
    pub node_bases: HashMap<NodeType, NodeType>,
    pub struct_bases: HashMap<EngineStruct, EngineStruct>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            node_defs: HashMap::new(),
            struct_defs: HashMap::new(),
            node_bases: HashMap::new(),
            struct_bases: HashMap::new(),
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

        //--------------------------------
        // Nodes
        //--------------------------------
        reg.register_node(
            NodeType::Node,
            None,
            vec![
                ("name", Type::String),
                ("id", Type::Custom("Uuid".into())),
            ],
        );

        reg.register_node(
            NodeType::Node2D,
            Some(NodeType::Node),
            vec![("transform", Type::EngineStruct(EngineStruct::Transform2D))],
        );

        reg.register_node(
            NodeType::Sprite2D,
            Some(NodeType::Node2D),
            vec![
                ("texture", Type::EngineStruct(EngineStruct::ImageTexture)),
                ("color", Type::EngineStruct(EngineStruct::Color)),
            ],
        );

        reg.register_node(
            NodeType::UINode,
            Some(NodeType::Node),
            vec![
                ("visible", Type::Bool),
                ("fur_path", Type::Custom("Option<String>".into())),
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
                })
                .collect(),
            methods: vec![],
        };
        self.node_defs.insert(kind, def);
    }

    // ---------------------------------------------------
    // Reflection lookups
    // ---------------------------------------------------

    pub fn get_field_type_struct(
        &self,
        struct_kind: &EngineStruct,
        field: &str,
    ) -> Option<Type> {
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

    pub fn get_field_type_node(
        &self,
        node_kind: &NodeType,
        field: &str,
    ) -> Option<Type> {
        let mut current = Some(node_kind.clone());
        while let Some(kind) = current {
            if let Some(def) = self.node_defs.get(&kind) {
                if let Some(f) = def.fields.iter().find(|f| f.name == field) {
                    return Some(f.typ.clone());
                }
            }
            current = self.node_bases.get(&kind).cloned();
        }
        None
    }

    /// Walk through nested fields like Node2D.transform.position.x
    pub fn resolve_chain_from_node(
        &self,
        node_kind: &NodeType,
        chain: &[String],
    ) -> Option<Type> {
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
}