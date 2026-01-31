use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use tree_sitter::Parser;

use crate::{
    ast::*,
    call_modules::CallModule,
    lang::typescript::api::TypeScriptAPI,
    lang::typescript::resource_api::{TypeScriptArray, TypeScriptMap},
};

pub struct TypeScriptParser {
    source: String,
    parser: Parser,
    pub parsed_structs: Vec<StructDef>,
    debug_enabled: bool,
    /// Variable name → inferred type (for local scope/type inference during parsing)
    type_env: HashMap<String, Type>,
}

impl TypeScriptParser {
    pub fn new(input: &str) -> Self {
        let mut parser = Parser::new();
        // tree-sitter-typescript uses LANGUAGE constant similar to C#
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .expect("Error loading TypeScript grammar");

        Self {
            source: input.to_string(),
            parser,
            parsed_structs: Vec::new(),
            debug_enabled: false, // Disable debug for production
            type_env: HashMap::new(),
        }
    }

    fn debug_node(&self, prefix: &str, node: tree_sitter::Node) {
        if self.debug_enabled {
            let node_text = self.get_node_text(node);
            let s_expr = node.to_sexp();

            let debug_line = format!(
                "DEBUG | {} | Kind: {:<25} | Text: {:<30.30} | S-Expr: {}\n",
                prefix,
                node.kind(),
                format!("{:?}", node_text),
                s_expr
            );

            // Write to debug file
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open("typescript_parser_debug.log")
            {
                let _ = file.write_all(debug_line.as_bytes());
            }
        }
    }

    pub fn parse_script(&mut self) -> Result<Script, String> {
        // Clear debug file at start of parsing
        if self.debug_enabled {
            let _ = std::fs::write("typescript_parser_debug.log", "");
        }

        let tree = self
            .parser
            .parse(&self.source, None)
            .ok_or("Failed to parse TypeScript source")?;

        let root = tree.root_node();
        self.debug_node("PARSE_SCRIPT", root);

        // First, parse all interface declarations at root level (these become struct definitions)
        // This must happen before parsing the class so structs are available when referenced
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "interface_declaration" {
                if let Ok(struct_def) = self.parse_interface_as_struct(child) {
                    self.parsed_structs.push(struct_def);
                }
            }
        }

        // Find the class declaration
        let class_node =
            Self::find_class_declaration_helper(root).ok_or("No class declaration found")?;

        self.debug_node("CLASS_FOUND", class_node);

        let mut script = self.parse_class_as_script(class_node)?;

        // Add the parsed interfaces to the script's structs (prepend so they're available)
        // Interfaces are defined first, then nested classes from the class body
        let mut all_structs = self.parsed_structs.clone();
        all_structs.extend(script.structs);
        script.structs = all_structs;

        Ok(script)
    }

    fn find_class_declaration_helper(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
        if node.kind() == "class_declaration" {
            return Some(node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = Self::find_class_declaration_helper(child) {
                return Some(found);
            }
        }

        None
    }

    fn parse_class_as_script(&mut self, class_node: tree_sitter::Node) -> Result<Script, String> {
        self.debug_node("PARSE_CLASS_START", class_node);
        let mut node_type = String::new();
        let mut script_vars = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        // Get base class if present (TypeScript uses "extends" keyword)
        let mut cursor = class_node.walk();
        for child in class_node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                let mut extends_cursor = child.walk();
                for extends_child in child.children(&mut extends_cursor) {
                    if extends_child.kind() == "type_identifier"
                        || extends_child.kind() == "identifier"
                    {
                        node_type = self.get_node_text(extends_child);
                        break;
                    }
                }
            }
        }

        // Find the class body
        if let Some(body) = self.get_child_by_kind(class_node, "class_body") {
            let mut body_cursor = body.walk();
            for member in body.children(&mut body_cursor) {
                self.debug_node("CLASS_MEMBER", member);
                match member.kind() {
                    "property_signature"
                    | "public_field_definition"
                    | "private_field_definition"
                    | "protected_field_definition"
                    | "field_definition" => {
                        // The member itself is the field definition, parse it directly
                        if let Ok(var) = self.parse_field_declaration(member) {
                            script_vars.push(var);
                        }
                    }
                    "method_definition" | "method_signature" => {
                        if let Ok(func) = self.parse_method_declaration(member) {
                            functions.push(func);
                        }
                    }
                    "class_declaration" => {
                        if let Ok(struct_def) = self.parse_nested_class(member) {
                            structs.push(struct_def);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Build attributes HashMap from variables, functions, and struct fields
        let mut attributes = HashMap::new();
        for var in &script_vars {
            if !var.attributes.is_empty() {
                attributes.insert(var.name.clone(), var.attributes.clone());
            }
        }
        for func in &functions {
            if !func.attributes.is_empty() {
                attributes.insert(func.name.clone(), func.attributes.clone());
            }
        }
        // Include struct field attributes with qualified names (StructName.fieldName)
        for struct_def in &structs {
            for field in &struct_def.fields {
                if !field.attributes.is_empty() {
                    let qualified_name = format!("{}.{}", struct_def.name, field.name);
                    attributes.insert(qualified_name, field.attributes.clone());
                }
            }
        }

        Ok(Script {
            script_name: None,
            node_type,
            variables: script_vars,
            functions,
            language: Some("typescript".to_string()),
            source_file: None, // Will be set by transpiler
            structs,
            verbose: true,
            attributes,
            module_names: std::collections::HashSet::new(), // Will be set by transpiler
            module_name_to_identifier: std::collections::HashMap::new(), // Will be set by transpiler
            module_functions: std::collections::HashMap::new(), // Will be set by transpiler
            module_variables: std::collections::HashMap::new(), // Will be set by transpiler
            module_scope_variables: None,
            is_global: false,
            global_names: std::collections::HashSet::new(),
            global_name_to_node_id: std::collections::HashMap::new(),
            rust_struct_name: None,
        })
    }

    fn parse_field_declaration(&mut self, node: tree_sitter::Node) -> Result<Variable, String> {
        self.debug_node("FIELD_DECL_START", node);
        let mut is_public = false;
        let mut is_exposed = false;
        let mut attributes = Vec::new();
        let mut typ = None;
        let mut name = String::new();
        let mut value = None;

        // Extract name - in TypeScript tree-sitter, property_identifier is a direct child
        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        // Strategy 1: Look for property_identifier directly (TypeScript uses this, not property_name)
        for child in &children {
            if child.kind() == "property_identifier" {
                name = self.get_node_text(*child);
                break;
            }
        }

        // Strategy 2: Fallback to property_name -> identifier (for compatibility)
        if name.is_empty() {
            for child in &children {
                if child.kind() == "property_name" {
                    if let Some(identifier) = self.get_child_by_kind(*child, "identifier") {
                        name = self.get_node_text(identifier);
                    } else {
                        name = self.get_node_text(*child);
                    }
                    break;
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.debug_node("FIELD_CHILD", child);
            match child.kind() {
                "decorator" => {
                    // Parse decorator as attribute
                    let attr_name = self.parse_decorator(child);
                    if !attr_name.is_empty() {
                        attributes.push(attr_name.clone());
                        // Check for @expose decorator
                        if attr_name.to_lowercase() == "expose" {
                            is_exposed = true;
                        }
                    }
                }
                "accessibility_modifier" => {
                    let mod_text = self.get_node_text(child);
                    if mod_text == "public" {
                        is_public = true;
                    }
                }
                "property_identifier" => {
                    // Property identifier is the name in TypeScript
                    if name.is_empty() {
                        name = self.get_node_text(child);
                    }
                }
                "property_name" => {
                    // Fallback for property_name (if it exists)
                    if name.is_empty() {
                        if let Some(identifier) = self.get_child_by_kind(child, "identifier") {
                            name = self.get_node_text(identifier);
                        } else {
                            name = self.get_node_text(child);
                        }
                    }
                }
                "type_annotation" => {
                    // TypeScript type_annotation contains the type directly or via "type" node
                    if let Some(type_node) = self.get_child_by_kind(child, "type") {
                        typ = Some(self.parse_type(type_node));
                    } else {
                        // Try parsing the type_annotation itself (might contain predefined_type directly)
                        typ = Some(self.parse_type(child));
                    }
                }
                _ => {}
            }
        }

        // Second pass: find the initializer expression (comes after =)
        if value.is_none() {
            let mut cursor = node.walk();
            let mut found_equals = false;
            for child in node.children(&mut cursor) {
                if found_equals {
                    // This should be the expression
                    // Check the type of expression and parse accordingly
                    let expr_result = if child.kind() == "array" {
                        // For array literals, pass the expected type so we can use Array vs FixedArray correctly
                        self.parse_array_literal(child, typ.as_ref())
                    } else if let Some(Type::Custom(struct_name)) = &typ {
                        if child.kind() == "object" {
                            // Parse with context so nested object literals are converted
                            self.parse_object_literal_with_context(child, Some(struct_name))
                        } else {
                            // Regular expression parsing, then convert if needed
                            self.parse_expression(child).and_then(|expr| {
                                if let Expr::ObjectLiteral(pairs) = expr {
                                    if self.parsed_structs.iter().any(|s| s.name == *struct_name) {
                                        self.convert_object_literal_to_struct_new(
                                            struct_name,
                                            &pairs,
                                        )
                                    } else {
                                        Ok(Expr::ObjectLiteral(pairs))
                                    }
                                } else {
                                    Ok(expr)
                                }
                            })
                        }
                    } else {
                        // Regular expression parsing
                        self.parse_expression(child)
                    };

                    if let Ok(expr) = expr_result {
                        value = Some(TypedExpr {
                            expr,
                            inferred_type: None,
                            span: None,
                        });
                        break;
                    }
                }
                if child.kind() == "=" {
                    found_equals = true;
                }
            }
        }

        // If no explicit type was found but we have an initializer, infer from the initializer
        if typ.is_none() && value.is_some() {
            typ = self.infer_type_from_expr(&value.as_ref().unwrap().expr);
        }

        // Store in type environment
        if let Some(ref t) = typ {
            self.type_env.insert(name.clone(), t.clone());
        }

        self.debug_node("FIELD_DECL_END", node);

        Ok(Variable {
            name,
            typ,
            value,
            is_exposed,
            is_public,
            is_const: false,
            attributes,
            span: None,
        })
    }

    fn infer_type_from_expr(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Literal(Literal::Number(_)) => {
                // In TypeScript, number is always f64, so default to f64
                // If it contains a decimal point, it's definitely f64
                // Otherwise, still default to f64 (TypeScript number is f64)
                Some(Type::Number(NumberKind::Float(64)))
            }
            Expr::Literal(Literal::String(_)) => Some(Type::String),
            Expr::Literal(Literal::Bool(_)) => Some(Type::Bool),
            Expr::ContainerLiteral(kind, _) => match kind {
                ContainerKind::Array => {
                    Some(Type::Container(ContainerKind::Array, vec![Type::Object]))
                }
                ContainerKind::Map => Some(Type::Container(
                    ContainerKind::Map,
                    vec![Type::String, Type::Object],
                )),
                ContainerKind::FixedArray(sz) => Some(Type::Container(
                    ContainerKind::FixedArray(*sz),
                    vec![Type::Object],
                )),
            },
            Expr::ObjectLiteral(_) => Some(Type::Object),
            Expr::Cast(_, target) => Some(target.clone()),
            Expr::Ident(var_name) => self.type_env.get(var_name).cloned(),
            Expr::Call(inner, _) => {
                if let Expr::MemberAccess(base, method) = &**inner {
                    if let Expr::Ident(type_name) = &**base {
                        match (type_name.as_str(), method.as_str()) {
                            ("BigInt", "parseInt") | ("Number", "parseInt") => {
                                Some(Type::Number(NumberKind::BigInt))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Expr::StructNew(type_name, _) => {
                if self.parsed_structs.iter().any(|s| s.name == *type_name) {
                    Some(Type::Custom(type_name.clone()))
                } else {
                    Some(self.map_type(type_name.clone()))
                }
            }
            Expr::BinaryOp(left, _op, right) => {
                let left_type = self.infer_type_from_expr(left);
                let right_type = self.infer_type_from_expr(right);

                match (&left_type, &right_type) {
                    (Some(l), Some(r)) if l == r => Some(l.clone()),
                    (Some(l), Some(r)) => self.promote_types_simple(l, r),
                    (Some(l), None) => Some(l.clone()),
                    (None, Some(r)) => Some(r.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn promote_types_simple(&self, left: &Type, right: &Type) -> Option<Type> {
        use crate::scripting::ast::NumberKind;
        use crate::scripting::ast::Type::*;

        if left == right {
            return Some(left.clone());
        }

        match (left, right) {
            (Number(NumberKind::BigInt), Number(_)) | (Number(_), Number(NumberKind::BigInt)) => {
                Some(Number(NumberKind::BigInt))
            }
            (Number(NumberKind::Decimal), Number(_)) | (Number(_), Number(NumberKind::Decimal)) => {
                Some(Number(NumberKind::Decimal))
            }
            (Number(NumberKind::Float(w1)), Number(NumberKind::Float(w2))) => {
                Some(Number(NumberKind::Float(*w1.max(w2))))
            }
            (Number(NumberKind::Float(w)), Number(_))
            | (Number(_), Number(NumberKind::Float(w))) => Some(Number(NumberKind::Float(*w))),
            (Number(NumberKind::Signed(w1)), Number(NumberKind::Unsigned(w2)))
            | (Number(NumberKind::Unsigned(w2)), Number(NumberKind::Signed(w1))) => {
                Some(Number(NumberKind::Signed(u8::max(*w1, *w2))))
            }
            (Number(NumberKind::Signed(w1)), Number(NumberKind::Signed(w2))) => {
                Some(Number(NumberKind::Signed(*w1.max(w2))))
            }
            (Number(NumberKind::Unsigned(w1)), Number(NumberKind::Unsigned(w2))) => {
                Some(Number(NumberKind::Unsigned(*w1.max(w2))))
            }
            _ => Some(left.clone()),
        }
    }

    fn parse_method_declaration(&mut self, node: tree_sitter::Node) -> Result<Function, String> {
        self.debug_node("METHOD_DECL_START", node);
        let mut return_type = Type::Void;
        let mut name = String::new();
        let mut params = Vec::new();
        let mut body = Vec::new();
        let mut attributes = Vec::new();

        // Extract method name - in TypeScript tree-sitter, property_identifier is a direct child
        let mut method_cursor = node.walk();
        let method_children: Vec<tree_sitter::Node> = node.children(&mut method_cursor).collect();

        // Strategy 1: Look for property_identifier directly (TypeScript uses this)
        for child in &method_children {
            if child.kind() == "property_identifier" {
                name = self.get_node_text(*child);
                break;
            }
        }

        // Strategy 2: Fallback to property_name -> identifier (for compatibility)
        if name.is_empty() {
            for child in &method_children {
                if child.kind() == "property_name" {
                    if let Some(identifier) = self.get_child_by_kind(*child, "identifier") {
                        name = self.get_node_text(identifier);
                    } else {
                        name = self.get_node_text(*child);
                    }
                    break;
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.debug_node("METHOD_CHILD", child);
            match child.kind() {
                "decorator" => {
                    // Parse decorator as attribute
                    let attr_name = self.parse_decorator(child);
                    if !attr_name.is_empty() {
                        attributes.push(attr_name);
                    }
                }
                "property_identifier" => {
                    // Property identifier is the method name in TypeScript
                    if name.is_empty() {
                        name = self.get_node_text(child);
                    }
                }
                "property_name" => {
                    // Fallback for property_name (if it exists)
                    if name.is_empty() {
                        if let Some(id) = self.get_child_by_kind(child, "identifier") {
                            name = self.get_node_text(id);
                        } else {
                            name = self.get_node_text(child);
                        }
                    }
                }
                "formal_parameters" => {
                    params = self.parse_parameter_list(child)?;
                }
                "statement_block" => {
                    body = self.parse_block(child)?;
                }
                "type_annotation" => {
                    // TypeScript type_annotation contains the type directly or via "type" node
                    if let Some(type_node) = self.get_child_by_kind(child, "type") {
                        return_type = self.parse_type(type_node);
                    } else {
                        // Try parsing the type_annotation itself (might contain predefined_type directly)
                        return_type = self.parse_type(child);
                    }
                }
                _ => {}
            }
        }

        let is_trait_method = name.to_lowercase() == "init"
            || name.to_lowercase() == "update"
            || name.to_lowercase() == "fixed_update";
        let locals = self.collect_locals(&body);

        self.debug_node("METHOD_DECL_END", node);

        Ok(Function {
            name,
            params,
            locals,
            body,
            is_trait_method,
            uses_self: false,
            cloned_child_nodes: Vec::new(), // Will be populated during analyze_self_usage
            return_type,
            span: None,
            attributes,
            is_on_signal: false,
            signal_name: None,
            is_lifecycle_method: false,
        })
    }

    fn collect_interface_members(
        &mut self,
        node: tree_sitter::Node,
        fields: &mut Vec<StructField>,
        methods: &mut Vec<Function>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            // Skip braces, commas, semicolons, and other punctuation
            if child.kind() == "{"
                || child.kind() == "}"
                || child.kind() == ","
                || child.kind() == ";"
            {
                continue;
            }
            self.debug_node(&format!("COLLECT_MEMBER: {}", child.kind()), child);
            match child.kind() {
                "property_signature" => {
                    self.debug_node("COLLECT_PROP_SIG", child);
                    match self.parse_interface_property(child) {
                        Ok(field) => {
                            self.debug_node(
                                &format!("COLLECT_PROP_SUCCESS: {}: {:?}", field.name, field.typ),
                                child,
                            );
                            fields.push(field);
                        }
                        Err(e) => {
                            self.debug_node(&format!("COLLECT_PROP_FAILED: {}", e), child);
                        }
                    }
                }
                "method_signature" => {
                    if let Ok(func) = self.parse_method_declaration(child) {
                        methods.push(func);
                    }
                }
                "interface_body" | "object_type" => {
                    // Nested interface body or object type - recurse
                    self.collect_interface_members(child, fields, methods);
                }
                _ => {
                    // Debug unknown members
                    self.debug_node(&format!("COLLECT_UNKNOWN: {}", child.kind()), child);
                }
            }
        }
    }

    fn parse_interface_property(&self, node: tree_sitter::Node) -> Result<StructField, String> {
        // Parse a property_signature from an interface (no initializer, just name: type)
        self.debug_node("PARSE_INTERFACE_PROP", node);
        let mut name = String::new();
        let mut typ = Type::Object;
        let mut attributes = Vec::new();

        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        // Extract name and attributes - look for property_identifier, identifier, or property_name
        for child in &children {
            self.debug_node("PROP_CHILD", *child);
            if child.kind() == "decorator" {
                // Parse decorator as attribute
                let attr_name = self.parse_decorator(*child);
                if !attr_name.is_empty() {
                    attributes.push(attr_name);
                }
            } else if child.kind() == "property_identifier" {
                name = self.get_node_text(*child);
            } else if child.kind() == "identifier" {
                name = self.get_node_text(*child);
            } else if child.kind() == "property_name" {
                // property_name might contain an identifier
                if let Some(id) = self.get_child_by_kind(*child, "identifier") {
                    name = self.get_node_text(id);
                } else {
                    name = self.get_node_text(*child);
                }
            }
        }

        // Extract type from type_annotation
        for child in &children {
            if child.kind() == "type_annotation" {
                self.debug_node("TYPE_ANNOT", *child);
                if let Some(type_node) = self.get_child_by_kind(*child, "type") {
                    typ = self.parse_type(type_node);
                } else if let Some(predefined) = self.get_child_by_kind(*child, "predefined_type") {
                    typ = self.parse_type(predefined);
                } else {
                    typ = self.parse_type(*child);
                }
                break;
            }
        }

        if name.is_empty() {
            let node_text = self.get_node_text(node);
            return Err(format!(
                "Property signature missing name. Node text: '{}', Kind: '{}'",
                node_text,
                node.kind()
            ));
        }

        self.debug_node(&format!("PROP_PARSED: {}: {:?}", name, typ), node);
        Ok(StructField {
            name,
            typ,
            attributes,
        })
    }

    fn parse_interface_as_struct(&mut self, node: tree_sitter::Node) -> Result<StructDef, String> {
        self.debug_node("INTERFACE_START", node);
        let mut name = String::new();
        let mut base = None;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.debug_node("INTERFACE_CHILD", child);
            match child.kind() {
                "type_identifier" | "identifier" => {
                    if name.is_empty() {
                        name = self.get_node_text(child);
                        self.debug_node(&format!("INTERFACE_NAME: {}", name), child);
                    }
                }
                "extends_type_clause" | "extends_clause" => {
                    let mut extends_cursor = child.walk();
                    for extends_child in child.children(&mut extends_cursor) {
                        if extends_child.kind() == "type_identifier"
                            || extends_child.kind() == "identifier"
                        {
                            base = Some(self.get_node_text(extends_child));
                            self.debug_node(&format!("INTERFACE_BASE: {:?}", base), extends_child);
                            break;
                        } else if extends_child.kind() == "type" {
                            // Sometimes the type is wrapped in a type node
                            if let Some(type_id) =
                                self.get_child_by_kind(extends_child, "type_identifier")
                            {
                                base = Some(self.get_node_text(type_id));
                                self.debug_node(&format!("INTERFACE_BASE: {:?}", base), type_id);
                                break;
                            }
                        }
                    }
                }
                "interface_body" | "object_type" => {
                    self.debug_node("INTERFACE_BODY_FOUND", child);
                    // Interface body contains property signatures
                    // Recursively collect all property_signature nodes
                    self.collect_interface_members(child, &mut fields, &mut methods);
                }
                // Also check for direct property_signature children (some interface structures)
                "property_signature" => {
                    self.debug_node("DIRECT_PROP_SIG", child);
                    if let Ok(field) = self.parse_interface_property(child) {
                        fields.push(field);
                    }
                }
                _ => {
                    self.debug_node(&format!("UNKNOWN_INTERFACE_CHILD: {}", child.kind()), child);
                }
            }
        }

        self.debug_node(
            &format!("INTERFACE_END: {} with {} fields", name, fields.len()),
            node,
        );
        Ok(StructDef {
            name,
            fields,
            methods,
            base,
        })
    }

    fn parse_nested_class(&mut self, node: tree_sitter::Node) -> Result<StructDef, String> {
        self.debug_node("NESTED_CLASS_START", node);
        let mut name = String::new();
        let mut base = None;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "identifier" => {
                    if name.is_empty() {
                        name = self.get_node_text(child);
                    }
                }
                "extends_clause" => {
                    let mut extends_cursor = child.walk();
                    for extends_child in child.children(&mut extends_cursor) {
                        if extends_child.kind() == "type_identifier"
                            || extends_child.kind() == "identifier"
                        {
                            base = Some(self.get_node_text(extends_child));
                            break;
                        }
                    }
                }
                "class_body" => {
                    let mut body_cursor = child.walk();
                    for member in child.children(&mut body_cursor) {
                        match member.kind() {
                            "property_signature"
                            | "public_field_definition"
                            | "private_field_definition"
                            | "protected_field_definition"
                            | "field_definition" => {
                                if let Ok(var) = self.parse_field_declaration(member) {
                                    fields.push(StructField {
                                        name: var.name,
                                        typ: var.typ.unwrap_or(Type::Object),
                                        attributes: var.attributes,
                                    });
                                }
                            }
                            "method_definition" | "method_signature" => {
                                if let Ok(func) = self.parse_method_declaration(member) {
                                    methods.push(func);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        self.debug_node("NESTED_CLASS_END", node);
        Ok(StructDef {
            name,
            fields,
            methods,
            base,
        })
    }

    fn parse_parameter_list(&self, node: tree_sitter::Node) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "required_parameter" || child.kind() == "optional_parameter" {
                if let Ok(param) = self.parse_parameter(child) {
                    params.push(param);
                }
            }
        }

        Ok(params)
    }

    fn parse_parameter(&self, node: tree_sitter::Node) -> Result<Param, String> {
        let mut typ = Type::Object;
        let mut name = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_annotation" => {
                    // TypeScript type_annotation contains the type directly or via "type" node
                    if let Some(type_node) = self.get_child_by_kind(child, "type") {
                        typ = self.parse_type(type_node);
                    } else {
                        // Try parsing the type_annotation itself (might contain predefined_type directly)
                        typ = self.parse_type(child);
                    }
                }
                "identifier" => {
                    name = self.get_node_text(child);
                }
                _ => {}
            }
        }

        Ok(Param {
            name,
            typ,
            span: None,
        })
    }

    fn parse_block(&mut self, node: tree_sitter::Node) -> Result<Vec<Stmt>, String> {
        self.debug_node("BLOCK_START", node);
        let mut statements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "{" || child.kind() == "}" {
                continue;
            }

            match self.parse_statement(child) {
                Ok(Stmt::Pass) => {}
                Ok(stmt) => {
                    // Debug: check if this is a MemberAssignOp
                    if let Stmt::MemberAssignOp(_, _, _) = &stmt {
                        self.debug_node("FOUND_MEMBER_ASSIGN_OP", child);
                    }
                    statements.push(stmt);
                }
                Err(e) => {
                    // Log the error but also try to parse as expression as fallback
                    self.debug_node(&format!("STMT_PARSE_ERR: {}", e), child);
                    // If it's an expression_statement, try to parse the child as assignment
                    if child.kind() == "expression_statement" {
                        if let Some(expr_child) = child.child(0) {
                            let kind = expr_child.kind();
                            if kind == "assignment_expression"
                                || kind == "augmented_assignment_expression"
                            {
                                if let Ok(stmt) = self.parse_assignment(expr_child) {
                                    statements.push(stmt);
                                    continue;
                                }
                            }
                        }
                    }
                    // Try parsing as expression statement as fallback
                    if let Ok(expr) = self.parse_expression(child) {
                        statements.push(Stmt::Expr(TypedExpr {
                            expr,
                            inferred_type: None,
                            span: None,
                        }));
                    } else {
                        // If expression parsing also fails, try parsing as assignment directly
                        // (maybe it's an assignment_expression or augmented_assignment_expression at statement level)
                        let child_kind = child.kind();
                        if child_kind == "assignment_expression"
                            || child_kind == "augmented_assignment_expression"
                        {
                            if let Ok(stmt) = self.parse_assignment(child) {
                                statements.push(stmt);
                            }
                        }
                    }
                }
            }
        }

        self.debug_node("BLOCK_END", node);
        Ok(statements)
    }

    fn parse_statement(&mut self, node: tree_sitter::Node) -> Result<Stmt, String> {
        self.debug_node("STMT_START", node);
        let result = match node.kind() {
            "lexical_declaration" | "variable_declaration" => self.parse_local_declaration(node),
            "expression_statement" => {
                // Get the expression child (which might be an assignment_expression or augmented_assignment_expression)
                if let Some(expr_node) = node.child(0) {
                    // Check if the child is an assignment_expression or augmented_assignment_expression
                    let kind = expr_node.kind();
                    if kind == "assignment_expression" || kind == "augmented_assignment_expression"
                    {
                        self.parse_assignment(expr_node)
                    } else {
                        self.parse_expression_statement(expr_node)
                    }
                } else {
                    Err("Empty expression statement".into())
                }
            }
            "assignment_expression" | "augmented_assignment_expression" => {
                // Handle assignment_expression or augmented_assignment_expression directly (might be at statement level)
                self.parse_assignment(node)
            }
            "if_statement" | "while_statement" | "for_statement" | "return_statement" => {
                Ok(Stmt::Pass)
            }
            "{" | "}" | ";" => Ok(Stmt::Pass),
            _ => {
                if let Ok(expr) = self.parse_expression(node) {
                    Ok(Stmt::Expr(TypedExpr {
                        expr,
                        inferred_type: None,
                        span: None,
                    }))
                } else {
                    Ok(Stmt::Pass)
                }
            }
        };

        if result.is_ok() {
            self.debug_node("STMT_END", node);
        } else {
            self.debug_node("STMT_FAILED", node);
        }
        result
    }

    fn parse_local_declaration(&mut self, node: tree_sitter::Node) -> Result<Stmt, String> {
        self.debug_node("LOCAL_DECL_START", node);
        let mut typ = None;
        let mut name = String::new();
        let mut value = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declarator" {
                // IMPORTANT: Parse type annotation FIRST (before parsing the value)
                // so that explicit types take precedence over inferred types

                // Look for type_annotation inside variable_declarator
                if let Some(type_annot) = self.get_child_by_kind(child, "type_annotation") {
                    if let Some(inner_type) = self.get_child_by_kind(type_annot, "type") {
                        typ = Some(self.parse_type(inner_type));
                    } else {
                        // Try parsing the type_annotation itself
                        typ = Some(self.parse_type(type_annot));
                    }
                }

                // Get the variable name
                if let Some(id) = self.get_child_by_kind(child, "identifier") {
                    name = self.get_node_text(id);
                }

                // Find the expression after =
                let mut init_cursor = child.walk();
                let mut found_equals = false;
                for init_child in child.children(&mut init_cursor) {
                    if found_equals {
                        if let Ok(expr) = self.parse_expression(init_child) {
                            value = Some(TypedExpr {
                                expr,
                                inferred_type: None,
                                span: None,
                            });
                            break;
                        }
                    }
                    if init_child.kind() == "=" {
                        found_equals = true;
                    }
                }
            }
        }

        // If no explicit type was found but we have an initializer, infer from the initializer
        // IMPORTANT: Only infer if explicit type is missing - explicit types take precedence
        if typ.is_none() && value.is_some() {
            typ = self.infer_type_from_expr(&value.as_ref().unwrap().expr);
        }

        if let Some(ref t) = typ {
            self.type_env.insert(name.clone(), t.clone());
        }

        Ok(Stmt::VariableDecl(Variable {
            name,
            typ,
            value,
            is_exposed: false,
            is_public: false,
            is_const: false,
            attributes: Vec::new(),
            span: None,
        }))
    }

    fn parse_expression_statement(&self, node: tree_sitter::Node) -> Result<Stmt, String> {
        // Check if this is an assignment expression
        if node.kind() == "assignment_expression" {
            return self.parse_assignment(node);
        }

        // Otherwise, parse as a regular expression
        if let Ok(expr) = self.parse_expression(node) {
            Ok(Stmt::Expr(TypedExpr {
                expr,
                inferred_type: None,
                span: None,
            }))
        } else {
            Err("Failed to parse expression".into())
        }
    }

    fn parse_assignment(&self, node: tree_sitter::Node) -> Result<Stmt, String> {
        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        // Tree-sitter-typescript structures assignment_expression as: [left, operator, right]
        // Use the same simple approach as C# parser
        if children.len() >= 3 {
            let lhs_expr = self.parse_expression(children[0])?;

            // Parse operator
            let op_text = self.get_node_text(children[1]);
            let op = match op_text.as_str() {
                "=" => Some(None),
                "+=" => Some(Some(Op::Add)),
                "-=" => Some(Some(Op::Sub)),
                "*=" => Some(Some(Op::Mul)),
                "/=" => Some(Some(Op::Div)),
                _ => None,
            };

            let rhs_expr = self.parse_expression(children[2])?;

            if let Some(op_val) = op {
                self.make_assign_stmt(lhs_expr, op_val, rhs_expr)
            } else {
                Err(format!("Invalid assignment operator: {}", op_text))
            }
        } else {
            Err(format!(
                "Invalid assignment: expected 3 children, got {}",
                children.len()
            ))
        }
    }

    fn make_assign_stmt(&self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
        let typed_rhs = TypedExpr {
            expr: rhs,
            inferred_type: None,
            span: None,
        };

        match lhs {
            Expr::Ident(name) => Ok(match op {
                None => Stmt::Assign(name, typed_rhs),
                Some(op) => Stmt::AssignOp(name, op, typed_rhs),
            }),
            Expr::MemberAccess(obj, field) => {
                let typed_lhs = TypedExpr {
                    expr: Expr::MemberAccess(obj, field),
                    inferred_type: None,
                    span: None,
                };
                Ok(match op {
                    None => Stmt::MemberAssign(typed_lhs, typed_rhs),
                    Some(op) => Stmt::MemberAssignOp(typed_lhs, op, typed_rhs),
                })
            }
            Expr::Index(obj, key) => Ok(match op {
                None => Stmt::IndexAssign(obj, key, typed_rhs),
                Some(op) => Stmt::IndexAssignOp(obj, key, op, typed_rhs),
            }),
            _ => Err("Invalid LHS for assignment".into()),
        }
    }

    fn parse_expression(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        self.debug_node("EXPR_START", node);
        let result = match node.kind() {
            "identifier" => Ok(Expr::Ident(self.get_node_text(node))),

            "this" => Ok(Expr::SelfAccess),

            "super" => Ok(Expr::BaseAccess),

            "number" => {
                let text = self.get_node_text(node);
                Ok(Expr::Literal(Literal::Number(text)))
            }

            "string" => {
                let text = self.get_node_text(node);
                let unquoted = text.trim_matches('"').trim_matches('\'');
                Ok(Expr::Literal(Literal::String(unquoted.to_string())))
            }

            "true" => Ok(Expr::Literal(Literal::Bool(true))),

            "false" => Ok(Expr::Literal(Literal::Bool(false))),

            "parenthesized_expression" => {
                if let Some(inner) = node.child(1) {
                    self.parse_expression(inner)
                } else {
                    Err("Empty parenthesized expression".into())
                }
            }

            "binary_expression" => self.parse_binary_expression(node),

            "call_expression" => self.parse_invocation(node),

            "as_expression" => self.parse_as_expression(node),

            "non_null_expression" => {
                // TypeScript non-null assertion operator: expr!
                // Just unwrap the inner expression (ignore the !)
                if let Some(inner) = node.child(0) {
                    self.parse_expression(inner)
                } else {
                    Err("Empty non-null expression".into())
                }
            }

            "member_expression" => self.parse_member_access(node),

            "subscript_expression" => self.parse_element_access(node),

            "new_expression" => self.parse_object_creation(node),

            "array" => self.parse_array_literal(node, None),

            "object" => {
                // When parsing object literals, we don't have context about expected type here
                // The conversion will happen in parse_field_declaration or when used as values
                self.parse_object_literal(node)
            }

            _ => Err(format!("Unsupported expression kind: {}", node.kind())),
        };

        if result.is_ok() {
            self.debug_node("EXPR_END", node);
        } else {
            self.debug_node("EXPR_FAILED", node);
        }

        result
    }

    fn parse_binary_expression(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        if children.len() >= 3 {
            let left = self.parse_expression(children[0])?;
            let op_text = self.get_node_text(children[1]);
            let right = self.parse_expression(children[2])?;

            let op = match op_text.as_str() {
                "+" => Op::Add,
                "-" => Op::Sub,
                "*" => Op::Mul,
                "/" => Op::Div,
                _ => return Err(format!("Unsupported binary operator: {}", op_text)),
            };

            Ok(Expr::BinaryOp(Box::new(left), op, Box::new(right)))
        } else {
            Err("Invalid binary expression".into())
        }
    }

    fn parse_invocation(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut func_expr = None;
        let mut args = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "arguments" => {
                    args = self.parse_argument_list(child)?;
                }
                "member_expression" | "identifier" => {
                    if func_expr.is_none() {
                        func_expr = Some(self.parse_expression(child)?);
                    }
                }
                _ => {}
            }
        }

        if let Some(expr) = func_expr {
            // Check if this is a type constructor like BigInt(...) or Number(...)
            // Convert them to Cast expressions (similar to C# parser)
            if let Expr::Ident(type_name) = &expr {
                match type_name.as_str() {
                    "BigInt" => {
                        // BigInt(string) or BigInt(number) -> Cast to BigInt
                        if !args.is_empty() {
                            return Ok(Expr::Cast(
                                Box::new(args[0].clone()),
                                Type::Number(NumberKind::BigInt),
                            ));
                        }
                    }
                    "Number" => {
                        // Number(value) -> Cast to f64 (number)
                        if !args.is_empty() {
                            return Ok(Expr::Cast(
                                Box::new(args[0].clone()),
                                Type::Number(NumberKind::Float(64)),
                            ));
                        }
                    }
                    "parseInt" => {
                        // parseInt(string, radix?) -> Cast to f64 (number)
                        // For now, just cast the first argument to number
                        if !args.is_empty() {
                            return Ok(Expr::Cast(
                                Box::new(args[0].clone()),
                                Type::Number(NumberKind::Float(64)),
                            ));
                        }
                    }
                    _ => {}
                }
            }

            // Check if this is toString() - convert to cast to string
            if let Expr::MemberAccess(obj, method) = &expr {
                if method.to_lowercase() == "tostring" && args.is_empty() {
                    // toString() with no args -> cast to string
                    return Ok(Expr::Cast(obj.clone(), Type::String));
                }
            }

            // Check if this is an API call (member access like console.log)
            if let Expr::MemberAccess(obj, method) = &expr {
                // First, check if it's a module-level API call (like console.log)
                if let Expr::Ident(module) = &**obj {
                    // Try both the original case and lowercase for console
                    let module_lower = module.to_lowercase();
                    if let Some(api_sem) = TypeScriptAPI::resolve(&module_lower, method) {
                        return Ok(Expr::ApiCall(CallModule::Module(api_sem), args));
                    }
                    // Also try original case
                    if let Some(api_sem) = TypeScriptAPI::resolve(module, method) {
                        return Ok(Expr::ApiCall(CallModule::Module(api_sem), args));
                    }
                }

                // Second, check if it's a method call on an array or map (like array.push() or map.get())
                // For array/map methods, the base is the array/map expression, and the method is the API method
                // We need to check if the method matches array/map API methods
                let method_lower = method.to_lowercase();
                if let Some(resource) = TypeScriptArray::resolve_method(&method_lower) {
                    // This is an array method call - the base is the array, prepend it to args
                    // API expects: [array_expr, ...method_args]
                    // Note: obj is Box<Expr>, so we need to dereference and clone to get Expr
                    let base_expr = (**obj).clone();
                    let mut array_args = vec![base_expr];
                    array_args.extend(args);
                    return Ok(Expr::ApiCall(CallModule::Resource(resource), array_args));
                }
                if let Some(resource) = TypeScriptMap::resolve_method(&method_lower) {
                    // This is a map method call - the base is the map, prepend it to args
                    // API expects: [map_expr, ...method_args]
                    // Note: obj is Box<Expr>, so we need to dereference and clone to get Expr
                    let base_expr = (**obj).clone();
                    let mut map_args = vec![base_expr];
                    map_args.extend(args);
                    return Ok(Expr::ApiCall(CallModule::Resource(resource), map_args));
                }
            }

            Ok(Expr::Call(Box::new(expr), args))
        } else {
            Err("Invalid invocation".into())
        }
    }

    fn parse_argument_list(&self, node: tree_sitter::Node) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() != "(" && child.kind() != ")" && child.kind() != "," {
                args.push(self.parse_expression(child)?);
            }
        }

        Ok(args)
    }

    fn parse_as_expression(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        // as_expression has structure: expression as type
        let mut expr_node = None;
        let mut type_node = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "as" => {} // Skip the "as" keyword
                _ => {
                    if expr_node.is_none() {
                        // First non-"as" child is the expression
                        expr_node = Some(child);
                    } else if type_node.is_none() {
                        // Second non-"as" child is the type
                        type_node = Some(child);
                    }
                }
            }
        }

        if let (Some(expr), Some(type_n)) = (expr_node, type_node) {
            let expr = self.parse_expression(expr)?;
            let target_type = self.parse_type(type_n);
            Ok(Expr::Cast(Box::new(expr), target_type))
        } else {
            Err("Invalid as expression".into())
        }
    }

    fn parse_member_access(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        if children.len() >= 2 {
            let obj = self.parse_expression(children[0])?;
            let last_child = &children[children.len() - 1];
            let member = if last_child.kind() == "property_identifier" {
                self.get_node_text(*last_child)
            } else if last_child.kind() == "identifier" {
                self.get_node_text(*last_child)
            } else {
                self.get_node_text(*last_child)
            };

            // Check if this is array.length or map.size - convert to API call
            let member_lower = member.to_lowercase();
            if member_lower == "length" || member_lower == "len" {
                // array.length -> ArrayResource::Len
                if let Some(resource) = TypeScriptArray::resolve_method(&member_lower) {
                    return Ok(Expr::ApiCall(CallModule::Resource(resource), vec![obj]));
                }
            } else if member_lower == "size" {
                // map.size -> MapResource::Len
                if let Some(resource) = TypeScriptMap::resolve_method(&member_lower) {
                    return Ok(Expr::ApiCall(CallModule::Resource(resource), vec![obj]));
                }
            }

            Ok(Expr::MemberAccess(Box::new(obj), member))
        } else {
            Err("Invalid member access".into())
        }
    }

    fn parse_element_access(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut obj = None;
        let mut index = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "[" | "]" => {}
                _ => {
                    if obj.is_none() {
                        obj = Some(self.parse_expression(child)?);
                    } else if index.is_none() {
                        index = Some(self.parse_expression(child)?);
                    }
                }
            }
        }

        if let (Some(o), Some(i)) = (obj, index) {
            Ok(Expr::Index(Box::new(o), Box::new(i)))
        } else {
            Err("Invalid element access".into())
        }
    }

    fn parse_object_creation(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut type_name = String::new();
        let mut args = Vec::new();
        let mut named_args = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type_identifier" | "identifier" => {
                    type_name = self.get_node_text(child);
                }
                "arguments" => {
                    args = self.parse_argument_list(child)?;
                }
                "object" => {
                    // Object literal with named properties
                    if let Ok(Expr::ObjectLiteral(pairs)) = self.parse_object_literal(child) {
                        for (key, value) in pairs {
                            if let Some(k) = key {
                                named_args.push((k, value));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle Map and Array as container literals, not structs
        if type_name == "Map" {
            // new Map([...]) - parse the array argument as map entries
            if !args.is_empty() {
                // The first argument should be an array of [key, value] pairs
                // Handle both Array and FixedArray cases
                let elements = match &args[0] {
                    Expr::ContainerLiteral(
                        ContainerKind::Array,
                        ContainerLiteralData::Array(elems),
                    ) => Some(elems),
                    Expr::ContainerLiteral(
                        ContainerKind::FixedArray(_),
                        ContainerLiteralData::FixedArray(_, elems),
                    ) => Some(elems),
                    _ => None,
                };

                if let Some(elements) = elements {
                    // Convert array of pairs to map entries
                    let mut map_entries = Vec::new();
                    for elem in elements {
                        // Each element should be an array literal [key, value]
                        // Handle both Array and FixedArray cases for pairs
                        let pair = match elem {
                            Expr::ContainerLiteral(
                                ContainerKind::Array,
                                ContainerLiteralData::Array(p),
                            ) => Some(p),
                            Expr::ContainerLiteral(
                                ContainerKind::FixedArray(_),
                                ContainerLiteralData::FixedArray(_, p),
                            ) => Some(p),
                            _ => None,
                        };

                        if let Some(pair) = pair {
                            if pair.len() >= 2 {
                                map_entries.push((pair[0].clone(), pair[1].clone()));
                            }
                        }
                    }
                    return Ok(Expr::ContainerLiteral(
                        ContainerKind::Map,
                        ContainerLiteralData::Map(map_entries),
                    ));
                }
            }
            // Empty Map or invalid - return empty map
            return Ok(Expr::ContainerLiteral(
                ContainerKind::Map,
                ContainerLiteralData::Map(vec![]),
            ));
        } else if type_name == "Array" {
            // new Array(...) - parse arguments as array elements
            return Ok(Expr::ContainerLiteral(
                ContainerKind::Array,
                ContainerLiteralData::Array(args),
            ));
        }

        // For other types, treat as struct creation
        if !named_args.is_empty() {
            Ok(Expr::StructNew(type_name, named_args))
        } else if !args.is_empty() {
            Ok(Expr::StructNew(type_name, vec![]))
        } else {
            Ok(Expr::StructNew(type_name, vec![]))
        }
    }

    fn parse_array_literal(
        &self,
        node: tree_sitter::Node,
        expected_type: Option<&Type>,
    ) -> Result<Expr, String> {
        let mut elements = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "[" && child.kind() != "]" && child.kind() != "," {
                // Parse expression with element type as expected_type so literals get correct suffix
                if let Ok(expr) = self.parse_expression(child) {
                    // If we have an element type, we need to wrap the expression to pass the type
                    // For now, we'll let codegen handle the type conversion via expected_type
                    elements.push(expr);
                }
            }
        }

        // Check if the expected type is a dynamic array (Array with Object element type)
        // If so, use ContainerKind::Array instead of FixedArray
        let use_dynamic_array =
            if let Some(Type::Container(ContainerKind::Array, params)) = expected_type {
                // If element type is Object (any[] or object[]), use dynamic array
                params.first().map_or(false, |t| matches!(t, Type::Object))
            } else {
                false
            };

        let len = elements.len();
        if use_dynamic_array {
            // Use Array (Vec<Value>) for dynamic arrays like any[] or object[]
            Ok(Expr::ContainerLiteral(
                ContainerKind::Array,
                ContainerLiteralData::Array(elements),
            ))
        } else if len > 0 {
            // Use FixedArray for statically typed arrays
            // The element_type will be used by codegen to convert literals correctly
            Ok(Expr::ContainerLiteral(
                ContainerKind::FixedArray(len),
                ContainerLiteralData::FixedArray(len, elements),
            ))
        } else {
            Ok(Expr::ContainerLiteral(
                ContainerKind::Array,
                ContainerLiteralData::Array(vec![]),
            ))
        }
    }

    fn parse_object_literal(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        self.parse_object_literal_with_context(node, None)
    }

    fn find_field_type_in_struct(&self, struct_name: &str, field_name: &str) -> Option<Type> {
        // Recursively search through base classes to find the field type
        fn find_field_recursive(
            parser: &TypeScriptParser,
            struct_name: &str,
            field_name: &str,
        ) -> Option<Type> {
            if let Some(struct_def) = parser.parsed_structs.iter().find(|s| s.name == struct_name) {
                // Check fields in this struct
                if let Some(field) = struct_def.fields.iter().find(|f| f.name == field_name) {
                    return Some(field.typ.clone());
                }

                // Check base class recursively
                if let Some(base_name) = &struct_def.base {
                    return find_field_recursive(parser, base_name, field_name);
                }
            }
            None
        }

        find_field_recursive(self, struct_name, field_name)
    }

    fn convert_object_literal_to_struct_new(
        &self,
        struct_name: &str,
        pairs: &[(Option<String>, Expr)],
    ) -> Result<Expr, String> {
        // Verify struct exists
        if !self.parsed_structs.iter().any(|s| s.name == struct_name) {
            return Err(format!("Struct {} not found", struct_name));
        }

        // Convert pairs to named args, recursively converting nested object literals
        let mut named_args = Vec::new();
        for (k, v) in pairs {
            if let Some(key) = k {
                // Find the field type for this key (checking base classes too)
                let mut converted_value = v.clone();
                if let Some(field_type) = self.find_field_type_in_struct(struct_name, key) {
                    // If the field type is a custom struct and the value is an ObjectLiteral,
                    // convert it recursively
                    if let Type::Custom(nested_struct_name) = &field_type {
                        if let Expr::ObjectLiteral(nested_pairs) = &converted_value {
                            if self
                                .parsed_structs
                                .iter()
                                .any(|s| s.name == *nested_struct_name)
                            {
                                // Recursively convert nested object literal
                                converted_value = self.convert_object_literal_to_struct_new(
                                    nested_struct_name,
                                    nested_pairs,
                                )?;
                            }
                        }
                    }
                }
                named_args.push((key.clone(), converted_value));
            }
        }
        Ok(Expr::StructNew(struct_name.to_string(), named_args))
    }

    fn parse_object_literal_with_context(
        &self,
        node: tree_sitter::Node,
        expected_struct: Option<&str>,
    ) -> Result<Expr, String> {
        let mut pairs = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "pair" {
                let mut key = None;
                let mut value = None;
                let mut field_type = None;

                // If we have an expected struct, find the field type for this key
                // (field type will be found after we get the key)

                let mut pair_cursor = child.walk();
                for pair_child in child.children(&mut pair_cursor) {
                    match pair_child.kind() {
                        "property_identifier" | "string" => {
                            let key_text =
                                self.get_node_text(pair_child).trim_matches('"').to_string();
                            key = Some(key_text.clone());

                            // If we have an expected struct, find the field type (checking base classes too)
                            if let Some(struct_name) = expected_struct {
                                if let Some(ft) =
                                    self.find_field_type_in_struct(struct_name, &key_text)
                                {
                                    field_type = Some(ft);
                                }
                            }
                        }
                        ":" => {}
                        _ => {
                            if value.is_none() {
                                // If this is an object literal and we know the field type, parse it with context
                                let expr = if pair_child.kind() == "object" {
                                    if let Some(Type::Custom(nested_struct_name)) = &field_type {
                                        // Parse the nested object literal with the expected struct type
                                        // Pass &str instead of String
                                        if let Ok(nested_expr) = self
                                            .parse_object_literal_with_context(
                                                pair_child,
                                                Some(nested_struct_name.as_str()),
                                            )
                                        {
                                            nested_expr
                                        } else {
                                            // Fallback to regular parsing
                                            self.parse_expression(pair_child)
                                                .unwrap_or_else(|_| Expr::ObjectLiteral(vec![]))
                                        }
                                    } else {
                                        // Regular object literal parsing
                                        self.parse_expression(pair_child)
                                            .unwrap_or_else(|_| Expr::ObjectLiteral(vec![]))
                                    }
                                } else {
                                    // Regular expression parsing
                                    self.parse_expression(pair_child)
                                        .unwrap_or_else(|_| Expr::ObjectLiteral(vec![]))
                                };

                                // If the value is an ObjectLiteral and we know the field type is a custom struct,
                                // convert it to StructNew (fallback conversion for non-object nodes)
                                let final_expr =
                                    if let Some(Type::Custom(nested_struct_name)) = &field_type {
                                        if let Expr::ObjectLiteral(nested_pairs) = &expr {
                                            if self
                                                .parsed_structs
                                                .iter()
                                                .any(|s| s.name == *nested_struct_name)
                                            {
                                                // Use the helper function to recursively convert
                                                self.convert_object_literal_to_struct_new(
                                                    nested_struct_name,
                                                    nested_pairs,
                                                )
                                                .unwrap_or(expr)
                                            } else {
                                                expr
                                            }
                                        } else {
                                            expr
                                        }
                                    } else {
                                        expr
                                    };

                                value = Some(final_expr);
                            }
                        }
                    }
                }

                if let (Some(k), Some(v)) = (key, value) {
                    pairs.push((Some(k), v));
                }
            }
        }

        // If we have an expected struct type, convert the entire object literal to StructNew
        if let Some(struct_name) = expected_struct {
            if self.parsed_structs.iter().any(|s| s.name == *struct_name) {
                let named_args: Vec<(String, Expr)> = pairs
                    .iter()
                    .filter_map(|(k, v)| k.as_ref().map(|key| (key.clone(), v.clone())))
                    .collect();
                return Ok(Expr::StructNew(struct_name.to_string(), named_args));
            }
        }

        Ok(Expr::ObjectLiteral(pairs))
    }

    fn parse_type(&self, node: tree_sitter::Node) -> Type {
        // Handle array types (TypeScript uses type[])
        if node.kind() == "array_type" {
            if let Some(base_type_node) = self.get_child_by_kind(node, "type") {
                let base_type = self.parse_type(base_type_node);
                Type::Container(ContainerKind::Array, vec![base_type])
            } else {
                let text = self.get_node_text(node);
                let base_type_str = text.replace("[]", "").trim().to_string();
                let element_type = self.map_type(base_type_str);
                Type::Container(ContainerKind::Array, vec![element_type])
            }
        } else if node.kind() == "predefined_type" {
            // TypeScript predefined types: number, string, boolean, any, void, etc.
            let type_text = self.get_node_text(node);
            self.map_type(type_text)
        } else if node.kind() == "type_annotation" {
            // If we're given a type_annotation node, extract the inner type
            if let Some(type_node) = self.get_child_by_kind(node, "type") {
                self.parse_type(type_node)
            } else if let Some(predefined) = self.get_child_by_kind(node, "predefined_type") {
                self.parse_type(predefined)
            } else {
                // Fallback: try to parse the annotation itself
                // Strip leading colon and whitespace if present (e.g., ": bigint" -> "bigint")
                let mut type_text = self.get_node_text(node);
                type_text = type_text.trim_start_matches(':').trim().to_string();
                self.map_type(type_text)
            }
        } else {
            // Check for generic types like Map<string, any> or Array<number>
            // In tree-sitter-typescript, these are parsed as type_identifier with type_arguments
            let type_name = if let Some(id) = self.get_child_by_kind(node, "type_identifier") {
                self.get_node_text(id)
            } else if let Some(id) = self.get_child_by_kind(node, "identifier") {
                self.get_node_text(id)
            } else if node.kind() == "predefined_type" {
                self.get_node_text(node)
            } else if node.kind() == "type_identifier" {
                self.get_node_text(node)
            } else {
                self.get_node_text(node)
            };

            // Check if this node has type_arguments (generic type parameters)
            if self.get_child_by_kind(node, "type_arguments").is_some() {
                // This is a generic type like Map<string, any> or Array<number>
                // Map the base type name (Map, Array, etc.) which will handle the generic syntax
                let full_text = self.get_node_text(node);
                self.map_type(full_text)
            } else {
                self.map_type(type_name)
            }
        }
    }

    fn map_type(&self, t: String) -> Type {
        match t.as_str() {
            "void" => Type::Void,
            "number" => Type::Number(NumberKind::Float(64)),
            "string" => Type::String,
            "boolean" => Type::Bool,
            "any" => Type::Any, // Dynamic any type
            "unknown" | "object" | "Object" | "Value" => Type::Object, // JSON/Value representation
            "bigint" => Type::Number(NumberKind::BigInt),
            // Handle Map and Array as standalone types (without generics)
            "Map" => Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
            "Array" => Type::Container(ContainerKind::Array, vec![Type::Object]),
            ty if ty.ends_with("[]") => {
                let base_type_str = ty.trim_end_matches("[]").to_string();
                let element_type = self.map_type(base_type_str);
                Type::Container(ContainerKind::Array, vec![element_type])
            }
            ty if ty.starts_with("Array<") => {
                // Parse Array<T> - extract the element type if possible
                // For now, default to Object, but could be improved to parse the generic parameter
                Type::Container(ContainerKind::Array, vec![Type::Object])
            }
            ty if ty.starts_with("Map<") || ty.starts_with("Record<") => {
                // Parse Map<K, V> or Record<K, V>
                // For now, default to Map<string, Object>, but could be improved to parse the generic parameters
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object])
            }
            _ => Type::Custom(t),
        }
    }

    fn collect_locals(&self, body: &[Stmt]) -> Vec<Variable> {
        body.iter()
            .filter_map(|stmt| match stmt {
                Stmt::VariableDecl(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    fn get_child_by_kind<'a>(
        &self,
        node: tree_sitter::Node<'a>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
    }

    fn get_node_text(&self, node: tree_sitter::Node) -> String {
        node.utf8_text(self.source.as_bytes())
            .unwrap_or("Node")
            .to_string()
    }

    fn parse_decorator(&self, node: tree_sitter::Node) -> String {
        // Decorator structure: @identifier or @identifier(...)
        // Extract the identifier name after @
        let text = self.get_node_text(node);
        // Remove @ symbol and any parentheses/arguments
        let attr_name = text
            .trim_start_matches('@')
            .split('(')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        attr_name
    }
}
