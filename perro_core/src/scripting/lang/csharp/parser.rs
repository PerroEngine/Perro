use std::collections::HashMap;
use tree_sitter::Parser;

// Assuming these are defined in your crate

use crate::{ast::*, lang::csharp::api::CSharpAPI};

pub struct CsParser {
    source: String,
    parser: Parser,
    pub parsed_structs: Vec<StructDef>,
    // Add a field to control debugging verbosity
    debug_enabled: bool,
    /// Variable name → inferred type (for local scope/type inference during parsing)
    type_env: HashMap<String, Type>,
}

impl CsParser {
    pub fn new(input: &str) -> Self {
        // Modified 'new' to accept debug flag
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
            .expect("Error loading C# grammar");

        Self {
            source: input.to_string(),
            parser,
            parsed_structs: Vec::new(),
            debug_enabled: false, // Initialize debug flag
            type_env: HashMap::new(),
        }
    }

    /// Helper to print debug info about a node if debugging is enabled.
    fn debug_node(&self, prefix: &str, node: tree_sitter::Node) {
        if self.debug_enabled {
            let node_text = self.get_node_text(node);
            let s_expr = node.to_sexp();

            eprintln!(
                "DEBUG | {} | Kind: {:<25} | Text: {:<30.30} | S-Expr: {}",
                prefix,
                node.kind(),
                format!("{:?}", node_text),
                s_expr
            );
        }
    }

    pub fn parse_script(&mut self) -> Result<Script, String> {
        let source_ref = &self.source;

        // 2. Perform the parsing operation. The result (the root node) is saved.
        // This is where 'self.parser' is borrowed immutably.
        let binding = self.parser.parse(source_ref, None).unwrap();
        let root_node = binding.root_node();

        // 3. Now that the borrows of 'self.parser' and 'self.source' are complete
        // (or held as simple references by the result), we can call the function
        // that requires a potentially MUTABLE borrow of 'self'.
        self.debug_node("PARSE_SCRIPT", root_node);

        let tree = self
            .parser
            .parse(&self.source, None)
            .ok_or("Failed to parse C# source")?;

        let root = tree.root_node();

        // Find the class declaration
        let class_node =
            Self::find_class_declaration_helper(root).ok_or("No class declaration found")?;

        self.debug_node("CLASS_FOUND", class_node);

        self.parse_class_as_script(class_node)
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

        // Get base class if present
        if let Some(base_list) = self.get_child_by_kind(class_node, "base_list") {
            let mut cursor = base_list.walk();

            // Iterate children to find the base type identifier (Node2D)
            for child in base_list.children(&mut cursor) {
                // Your debug output showed that "Node2D" is represented by a
                // direct child of kind "identifier" under "base_list".
                if child.kind() == "identifier" {
                    node_type = self.get_node_text(child);
                    // At this point, node_type should be "Node2D"
                    break;
                }
            }
        }

        // Find the declaration list (body of the class)
        if let Some(body) = self.get_child_by_kind(class_node, "declaration_list") {
            let mut cursor = body.walk();

            for member in body.children(&mut cursor) {
                self.debug_node("CLASS_MEMBER", member); // Debug each member
                match member.kind() {
                    "field_declaration" => {
                        if let Ok(var) = self.parse_field_declaration(member) {
                            script_vars.push(var);
                        }
                    }
                    "method_declaration" => {
                        if let Ok(func) = self.parse_method_declaration(member) {
                            functions.push(func);
                        }
                    }
                    "class_declaration" | "struct_declaration" => {
                        if let Ok(struct_def) = self.parse_nested_class(member) {
                            structs.push(struct_def);
                        }
                    }
                    _ => {}
                }
            }
        }
        println! {"AHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHH I AM THE NODE TYPE:  {}", node_type};

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
            structs,
            verbose: true,
            attributes,
        })
    }

    fn parse_field_declaration(&mut self, node: tree_sitter::Node) -> Result<Variable, String> {
        self.debug_node("FIELD_DECL_START", node); // Debug field declaration start
        let mut is_public = false;
        let mut is_exposed = false;
        let mut attributes = Vec::new();
        let mut typ = None;
        let mut name = String::new();
        let mut value = None;

        // Check for modifiers and attributes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.debug_node("FIELD_CHILD", child); // Debug field children
            match child.kind() {
                "attribute_list" => {
                    is_exposed = self.has_expose_attribute(child);
                    attributes = self.parse_attributes(child);
                }
                "modifier" => {
                    if self.get_node_text(child) == "public" {
                        is_public = true;
                    }
                }
                "variable_declaration" => {
                    // Get type - try multiple node types for array types, generic types, etc.
                    // Check for array_type first since it's more specific
                    // IMPORTANT: We need to find the type BEFORE parsing the initializer,
                    // so that explicit types take precedence over inferred types

                    // First, try to find array_type (might be nested in a type node)
                    let mut found_array_type = false;
                    if let Some(array_type_node) = self.get_child_by_kind(child, "array_type") {
                        typ = Some(self.parse_type(array_type_node));
                        found_array_type = true;
                    } else if let Some(type_node) = self.get_child_by_kind(child, "type") {
                        // Check if the type node itself is an array_type or contains one
                        if type_node.kind() == "array_type" {
                            typ = Some(self.parse_type(type_node));
                            found_array_type = true;
                        } else if let Some(nested_array_type) =
                            self.get_child_by_kind(type_node, "array_type")
                        {
                            typ = Some(self.parse_type(nested_array_type));
                            found_array_type = true;
                        } else {
                            // Check if type node has array_rank_specifier (indicates array)
                            let mut cursor = type_node.walk();
                            let has_array_spec = type_node
                                .children(&mut cursor)
                                .any(|c| c.kind() == "array_rank_specifier" || c.kind() == "[");
                            if has_array_spec {
                                typ = Some(self.parse_type(type_node));
                                found_array_type = true;
                            } else {
                                typ = Some(self.parse_type(type_node));
                            }
                        }
                    }

                    if !found_array_type {
                        if let Some(type_node) = self.get_child_by_kind(child, "predefined_type") {
                            typ = Some(self.parse_type(type_node));
                        } else if let Some(type_node) =
                            self.get_child_by_kind(child, "generic_name")
                        {
                            typ = Some(self.parse_type(type_node));
                        } else {
                            // Fallback: try to extract type from variable_declaration text
                            // For "BigInteger typed_big_int = ..." or "object[] arr = ..." or "TestPlayer[] arr = ...", extract the type
                            let decl_text = self.get_node_text(child);

                            // First, try to find array_type by checking for [] in the text
                            // This handles both "object[]" and "TestPlayer[]" cases
                            if decl_text.contains("[]") {
                                // Find the part that ends with [] - could be "object[]", "int[]", "TestPlayer[]", etc.
                                // Split by whitespace and look for the type part
                                let parts: Vec<&str> = decl_text.split_whitespace().collect();
                                for (i, part) in parts.iter().enumerate() {
                                    // Skip modifiers
                                    if matches!(
                                        *part,
                                        "public"
                                            | "private"
                                            | "protected"
                                            | "internal"
                                            | "static"
                                            | "readonly"
                                    ) {
                                        continue;
                                    }
                                    // Check if this part ends with []
                                    if part.ends_with("[]") {
                                        let base_type = part.trim_end_matches("[]");
                                        let element_type = self.map_type(base_type.to_string());
                                        typ = Some(Type::Container(
                                            ContainerKind::Array,
                                            vec![element_type],
                                        ));
                                        break;
                                    } else if i < parts.len() - 1 {
                                        // Check if next part is [] (e.g., "TestPlayer []" with space)
                                        if let Some(next_part) = parts.get(i + 1) {
                                            if next_part.trim() == "[]"
                                                || next_part.starts_with("[")
                                            {
                                                let base_type = part.trim();
                                                let element_type =
                                                    self.map_type(base_type.to_string());
                                                typ = Some(Type::Container(
                                                    ContainerKind::Array,
                                                    vec![element_type],
                                                ));
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Not an array, try to find the type name
                                let parts: Vec<&str> = decl_text.split_whitespace().collect();
                                for part in &parts {
                                    if !matches!(
                                        *part,
                                        "public"
                                            | "private"
                                            | "protected"
                                            | "internal"
                                            | "static"
                                            | "readonly"
                                    ) && !part.contains("=")
                                        && !part.contains("(")
                                        && !part.contains(")")
                                    {
                                        // It's likely the type name (not the variable name or initializer)
                                        typ = Some(self.map_type(part.to_string()));
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    // Get variable declarator
                    if let Some(declarator) = self.get_child_by_kind(child, "variable_declarator") {
                        if let Some(id) = self.get_child_by_kind(declarator, "identifier") {
                            name = self.get_node_text(id);
                        }

                        // Check for initialization
                        if let Some(init) =
                            self.get_child_by_kind(declarator, "equals_value_clause")
                        {
                            // The equals_value_clause has structure: = expression
                            // Find the expression child (skip the = token)
                            let mut init_cursor = init.walk();
                            for init_child in init.children(&mut init_cursor) {
                                if init_child.kind() != "=" {
                                    if let Ok(expr) = self.parse_expression(init_child) {
                                        value = Some(TypedExpr {
                                            expr,
                                            inferred_type: None,
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // If no explicit type was found but we have an initializer, infer from the initializer
        // IMPORTANT: Only infer if explicit type is missing - explicit types take precedence
        // CRITICAL: Never override an explicit type with an inferred type from the initializer
        if typ.is_none() && value.is_some() {
            typ = self.infer_type_from_expr(&value.as_ref().unwrap().expr);
        }

        // Store in type environment for later variable reference inference
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
            attributes,
        })
    }

    fn infer_type_from_expr(&self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Literal(Literal::Number(n)) => {
                if n.contains("f") || n.contains("F") {
                    Some(Type::Number(NumberKind::Float(32)))
                } else if n.contains(".") {
                    Some(Type::Number(NumberKind::Float(64)))
                } else if n.contains("m") || n.contains("M") {
                    Some(Type::Number(NumberKind::Decimal))
                } else {
                    Some(Type::Number(NumberKind::Signed(32)))
                }
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
                // Handle static method calls like BigInteger.Parse()
                if let Expr::MemberAccess(base, method) = &**inner {
                    // Check if it's a known type's static method
                    if let Expr::Ident(type_name) = &**base {
                        match (type_name.as_str(), method.as_str()) {
                            ("BigInteger", "Parse") => Some(Type::Number(NumberKind::BigInt)),
                            ("Decimal", "Parse") | ("decimal", "Parse") => {
                                Some(Type::Number(NumberKind::Decimal))
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
                // Check if it's a known struct
                if self.parsed_structs.iter().any(|s| s.name == *type_name) {
                    Some(Type::Custom(type_name.clone()))
                } else {
                    // Try to map it as a type
                    Some(self.map_type(type_name.clone()))
                }
            }
            Expr::BinaryOp(left, _op, right) => {
                // Infer types from both operands and promote
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
            Expr::Cast(_, target_type) => Some(target_type.clone()),
            _ => None,
        }
    }

    fn promote_types_simple(&self, left: &Type, right: &Type) -> Option<Type> {
        use crate::scripting::ast::NumberKind;
        use crate::scripting::ast::Type::*;

        // Fast path for identical types
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
        self.debug_node("METHOD_DECL_START", node); // Debug method declaration start
        let mut is_public = false;
        let mut attributes = Vec::new();
        let mut return_type = Type::Void;
        let mut name = String::new();
        let mut params = Vec::new();
        let mut body = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.debug_node("METHOD_CHILD", child); // Debug method children
            match child.kind() {
                "attribute_list" => {
                    attributes = self.parse_attributes(child);
                }
                "modifier" => {
                    if self.get_node_text(child) == "public" {
                        is_public = true;
                    }
                }
                "type" | "predefined_type" | "void_keyword" => {
                    return_type = self.parse_type(child);
                }
                "identifier" => {
                    name = self.get_node_text(child);
                }
                "parameter_list" => {
                    params = self.parse_parameter_list(child)?;
                }
                "block" => {
                    body = self.parse_block(child)?;
                }
                _ => {}
            }
        }

        let is_trait_method = name.to_lowercase() == "init"
            || name.to_lowercase() == "update"
            || name.to_lowercase() == "fixed_update"
            || name.to_lowercase() == "draw";
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
            attributes,
            is_on_signal: false,
            signal_name: None,
        })
    }

    // ... (parse_nested_class is unchanged, but you could add debug_node calls there too)

    fn parse_nested_class(&mut self, node: tree_sitter::Node) -> Result<StructDef, String> {
        self.debug_node("NESTED_CLASS_START", node);
        let mut name = String::new();
        let mut base = None;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" => {
                    name = self.get_node_text(child);
                }
                "base_list" => {
                    // Look for identifier within base_list (similar to parse_class_as_script)
                    let mut base_cursor = child.walk();
                    for base_child in child.children(&mut base_cursor) {
                        if base_child.kind() == "identifier" {
                            base = Some(self.get_node_text(base_child));
                            break;
                        } else if base_child.kind() == "type" {
                            // Sometimes it's wrapped in a type node
                            if let Some(id_node) = self.get_child_by_kind(base_child, "identifier")
                            {
                                base = Some(self.get_node_text(id_node));
                                break;
                            } else {
                                base = Some(self.get_node_text(base_child));
                                break;
                            }
                        }
                    }
                }
                "declaration_list" => {
                    let mut body_cursor = child.walk();
                    for member in child.children(&mut body_cursor) {
                        match member.kind() {
                            "field_declaration" => {
                                if let Ok(var) = self.parse_field_declaration(member) {
                                    fields.push(StructField {
                                        name: var.name,
                                        typ: var.typ.unwrap_or(Type::Object),
                                        attributes: var.attributes,
                                    });
                                }
                            }
                            "method_declaration" => {
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

    // ... (parse_parameter_list is unchanged)

    fn parse_parameter_list(&self, node: tree_sitter::Node) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "parameter" {
                if let Ok(param) = self.parse_parameter(child) {
                    params.push(param);
                }
            }
        }

        Ok(params)
    }

    // ... (parse_parameter is unchanged)

    fn parse_parameter(&self, node: tree_sitter::Node) -> Result<Param, String> {
        let mut typ = Type::Object;
        let mut name = String::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "type" | "predefined_type" => {
                    typ = self.parse_type(child);
                }
                "identifier" => {
                    name = self.get_node_text(child);
                }
                _ => {}
            }
        }

        Ok(Param { name, typ })
    }

    fn parse_block(&mut self, node: tree_sitter::Node) -> Result<Vec<Stmt>, String> {
        self.debug_node("BLOCK_START", node); // Debug block start
        let mut statements = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            // Skip the opening and closing braces
            if child.kind() == "{" || child.kind() == "}" {
                continue;
            }

            // Try to parse as statement
            match self.parse_statement(child) {
                Ok(Stmt::Pass) => {
                    // Skip pass statements
                }
                Ok(stmt) => {
                    statements.push(stmt);
                }
                Err(e) => {
                    // Log but don't fail - some nodes might not be statements
                    self.debug_node(&format!("STMT_PARSE_ERR: {}", e), child);
                }
            }
        }

        self.debug_node("BLOCK_END", node);
        Ok(statements)
    }

    fn parse_statement(&mut self, node: tree_sitter::Node) -> Result<Stmt, String> {
        self.debug_node("STMT_START", node); // Debug statement being parsed
        let result = match node.kind() {
            "local_declaration_statement" => self.parse_local_declaration(node),
            "expression_statement" => {
                if let Some(expr_node) = node.child(0) {
                    self.parse_expression_statement(expr_node)
                } else {
                    Err("Empty expression statement".into())
                }
            }
            "if_statement" | "while_statement" | "for_statement" | "return_statement" => {
                // For now, treat control flow as pass statements
                Ok(Stmt::Pass)
            }
            "{" | "}" | ";" => {
                // Skip braces and semicolons
                Ok(Stmt::Pass)
            }
            _ => {
                // Try to parse as expression
                if let Ok(expr) = self.parse_expression(node) {
                    Ok(Stmt::Expr(TypedExpr {
                        expr,
                        inferred_type: None,
                    }))
                } else {
                    // Don't error on unknown nodes, just skip them
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

    // ... (parse_local_declaration is unchanged)

    fn parse_local_declaration(&mut self, node: tree_sitter::Node) -> Result<Stmt, String> {
        self.debug_node("LOCAL_DECL_START", node);
        let mut typ = None;
        let mut name = String::new();
        let mut value = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                let mut decl_cursor = child.walk();
                for decl_child in child.children(&mut decl_cursor) {
                    match decl_child.kind() {
                        "type" | "predefined_type" | "implicit_type" | "identifier"
                        | "generic_name" => {
                            let type_text = self.get_node_text(decl_child);
                            if type_text != "var" {
                                typ = Some(self.parse_type(decl_child));
                            }
                        }
                        "variable_declarator" => {
                            if let Some(id) = self.get_child_by_kind(decl_child, "identifier") {
                                name = self.get_node_text(id);
                            }

                            if let Some(init) =
                                self.get_child_by_kind(decl_child, "equals_value_clause")
                            {
                                // The equals_value_clause has structure: = expression
                                // Find the expression child (skip the = token)
                                let mut init_cursor = init.walk();
                                for init_child in init.children(&mut init_cursor) {
                                    if init_child.kind() != "=" {
                                        if let Ok(expr) = self.parse_expression(init_child) {
                                            value = Some(TypedExpr {
                                                expr,
                                                inferred_type: None,
                                            });
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // If no explicit type and we have an initializer, infer from it
        if typ.is_none() && value.is_some() {
            typ = self.infer_type_from_expr(&value.as_ref().unwrap().expr);
        }

        // Store in type environment for later variable reference inference
        if let Some(ref t) = typ {
            self.type_env.insert(name.clone(), t.clone());
        }

        self.debug_node("LOCAL_DECL_END", node);

        Ok(Stmt::VariableDecl(Variable {
            name,
            typ,
            value,
            is_exposed: false,
            is_public: false,
            attributes: Vec::new(),
        }))
    }

    fn parse_expression_statement(&self, node: tree_sitter::Node) -> Result<Stmt, String> {
        self.debug_node("EXPR_STMT_START", node);
        let result = match node.kind() {
            "assignment_expression" => self.parse_assignment(node),
            _ => {
                let expr = self.parse_expression(node)?;
                Ok(Stmt::Expr(TypedExpr {
                    expr,
                    inferred_type: None,
                }))
            }
        };
        self.debug_node("EXPR_STMT_END", node);
        result
    }

    // ... (parse_assignment is unchanged)

    fn parse_assignment(&self, node: tree_sitter::Node) -> Result<Stmt, String> {
        let mut lhs = None;
        let mut op = None;
        let mut rhs = None;

        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        if children.len() >= 3 {
            lhs = Some(self.parse_expression(children[0])?);

            // Parse operator
            let op_text = self.get_node_text(children[1]);
            op = match op_text.as_str() {
                "=" => Some(None),
                "+=" => Some(Some(Op::Add)),
                "-=" => Some(Some(Op::Sub)),
                "*=" => Some(Some(Op::Mul)),
                "/=" => Some(Some(Op::Div)),
                _ => None,
            };

            rhs = Some(self.parse_expression(children[2])?);
        }

        if let (Some(lhs_expr), Some(op_val), Some(rhs_expr)) = (lhs, op, rhs) {
            self.make_assign_stmt(lhs_expr, op_val, rhs_expr)
        } else {
            Err("Invalid assignment".into())
        }
    }

    // ... (make_assign_stmt is unchanged)

    fn make_assign_stmt(&self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
        let typed_rhs = TypedExpr {
            expr: rhs,
            inferred_type: None,
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
        self.debug_node("EXPR_START", node); // Debug expression being parsed
        let result = match node.kind() {
            "identifier" => Ok(Expr::Ident(self.get_node_text(node))),

            "this_expression" => Ok(Expr::SelfAccess),

            "base_expression" => Ok(Expr::BaseAccess),

            "integer_literal" | "real_literal" => {
                let mut text = self.get_node_text(node);
                // Strip C# float suffixes (f, F, d, D, m, M) from the literal text
                // The type will be inferred separately based on the suffix
                if text.ends_with('f') || text.ends_with('F') {
                    text.pop(); // Remove f/F suffix - type will be inferred as f32
                } else if text.ends_with('d') || text.ends_with('D') {
                    text.pop(); // Remove d/D suffix - type will be inferred as f64
                } else if text.ends_with('m') || text.ends_with('M') {
                    text.pop(); // Remove m/M suffix - type will be inferred as decimal
                }
                Ok(Expr::Literal(Literal::Number(text)))
            }

            "string_literal" => {
                let text = self.get_node_text(node);
                let unquoted = text.trim_matches('"');
                Ok(Expr::Literal(Literal::String(unquoted.to_string())))
            }

            "true_literal" => Ok(Expr::Literal(Literal::Bool(true))),

            "false_literal" => Ok(Expr::Literal(Literal::Bool(false))),

            "parenthesized_expression" => {
                if let Some(inner) = node.child(1) {
                    self.parse_expression(inner)
                } else {
                    Err("Empty parenthesized expression".into())
                }
            }

            "binary_expression" => self.parse_binary_expression(node),

            "invocation_expression" => self.parse_invocation(node),

            "member_access_expression" => self.parse_member_access(node),

            "element_access_expression" => self.parse_element_access(node),

            "object_creation_expression" => self.parse_object_creation(node),

            "array_creation_expression" => self.parse_array_creation(node),

            "anonymous_object_creation_expression" => self.parse_anonymous_object(node),

            _ => Err(format!("Unsupported expression kind: {}", node.kind())),
        };

        if result.is_ok() {
            self.debug_node("EXPR_END", node);
        } else {
            self.debug_node("EXPR_FAILED", node);
        }

        result
    }

    // ... (rest of the functions are unchanged, but they could also benefit from debug_node calls)

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
                "argument_list" => {
                    args = self.parse_argument_list(child)?;
                }
                "member_access_expression" | "identifier" | "simple_name" => {
                    if func_expr.is_none() {
                        func_expr = Some(self.parse_expression(child)?);
                    }
                }
                _ => {}
            }
        }

        if let Some(expr) = func_expr {
            // Check if this is a type conversion method like BigInteger.Parse() or Decimal.Parse()
            if let Expr::MemberAccess(obj, method) = &expr {
                if method == "Parse" && args.len() == 1 {
                    if let Expr::Ident(type_name) = &**obj {
                        match type_name.as_str() {
                            "BigInteger" => {
                                // Convert BigInteger.Parse(string) to a Cast expression
                                // Codegen already knows how to handle String -> BigInt casts
                                return Ok(Expr::Cast(
                                    Box::new(args[0].clone()),
                                    Type::Number(NumberKind::BigInt),
                                ));
                            }
                            "Decimal" | "decimal" => {
                                // Convert Decimal.Parse(string) to a Cast expression
                                return Ok(Expr::Cast(
                                    Box::new(args[0].clone()),
                                    Type::Number(NumberKind::Decimal),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Check if this is an API call
            if let Expr::MemberAccess(obj, method) = &expr {
                if let Expr::Ident(module) = &**obj {
                    if let Some(api_sem) = CSharpAPI::resolve(module, method) {
                        return Ok(Expr::ApiCall(api_sem, args));
                    }
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
            if child.kind() == "argument" {
                if let Some(expr_node) = child.child(0) {
                    args.push(self.parse_expression(expr_node)?);
                }
            }
        }

        Ok(args)
    }

    fn parse_member_access(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut cursor = node.walk();
        let children: Vec<tree_sitter::Node> = node.children(&mut cursor).collect();

        if children.len() >= 2 {
            // First child is the object, last is the member name
            let obj = self.parse_expression(children[0])?;

            // The member name might be in a "simple_name" or directly as "identifier"
            let last_child = &children[children.len() - 1];
            let member = if last_child.kind() == "simple_name" {
                if let Some(id) = self.get_child_by_kind(*last_child, "identifier") {
                    self.get_node_text(id)
                } else {
                    self.get_node_text(*last_child)
                }
            } else {
                self.get_node_text(*last_child)
            };

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
                "bracketed_argument_list" => {
                    if let Some(arg_node) = child.child(1) {
                        // Skip '['
                        if let Some(expr_node) = arg_node.child(0) {
                            index = Some(self.parse_expression(expr_node)?);
                        }
                    }
                }
                _ => {
                    obj = Some(self.parse_expression(child)?);
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
        let mut initializers = Vec::new();
        let mut is_array_type = false;

        // First, check if any child is an array_type node or has array_rank_specifier
        let mut cursor = node.walk();
        let has_array_type_node = node
            .children(&mut cursor)
            .any(|c| c.kind() == "array_type" || c.kind() == "array_rank_specifier");
        if has_array_type_node {
            is_array_type = true;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "array_type" => {
                    // Parse the array_type to get the base type
                    let base_type = self.parse_type(child);
                    // Extract element type from the array type for display
                    if let Type::Container(ContainerKind::Array, params) = base_type {
                        if let Some(element_type) = params.get(0) {
                            type_name = element_type.to_rust_type(); // Use for display, but we know it's an array
                        }
                    }
                    is_array_type = true;
                }
                "array_rank_specifier" => {
                    is_array_type = true;
                }
                "type" | "predefined_type" | "generic_name" | "identifier" => {
                    let type_text = self.get_node_text(child);
                    type_name = type_text.clone();
                }
                "argument_list" => {
                    args = self.parse_argument_list(child)?;
                }
                "initializer_expression" => {
                    // Check if this is an array initializer { 1, 2, 3 } or object initializer { field = value }
                    let mut init_cursor = child.walk();
                    let mut has_assignments = false;

                    for init_child in child.children(&mut init_cursor) {
                        if init_child.kind() == "assignment_expression" {
                            has_assignments = true;
                            let mut assign_cursor = init_child.walk();
                            let assign_children: Vec<tree_sitter::Node> =
                                init_child.children(&mut assign_cursor).collect();

                            if assign_children.len() >= 3 {
                                let key = self.get_node_text(assign_children[0]);
                                let value = self.parse_expression(assign_children[2])?;
                                named_args.push((key, value));
                            }
                        } else if init_child.kind() != ","
                            && init_child.kind() != "{"
                            && init_child.kind() != "}"
                        {
                            // Try to parse as expression (for array elements)
                            if let Ok(expr) = self.parse_expression(init_child) {
                                initializers.push(expr);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Handle array types first - check is_array_type flag set from array_type/array_rank_specifier nodes
        if is_array_type {
            if !initializers.is_empty() {
                let len = initializers.len();
                Ok(Expr::ContainerLiteral(
                    ContainerKind::FixedArray(len),
                    ContainerLiteralData::FixedArray(len, initializers),
                ))
            } else {
                // Empty array
                Ok(Expr::ContainerLiteral(
                    ContainerKind::Array,
                    ContainerLiteralData::Array(vec![]),
                ))
            }
        } else if type_name.ends_with("[]") {
            // Fallback: check type name for [] suffix
            if !initializers.is_empty() {
                let len = initializers.len();
                Ok(Expr::ContainerLiteral(
                    ContainerKind::FixedArray(len),
                    ContainerLiteralData::FixedArray(len, initializers),
                ))
            } else {
                Ok(Expr::ContainerLiteral(
                    ContainerKind::Array,
                    ContainerLiteralData::Array(vec![]),
                ))
            }
        } else if type_name.starts_with("Dictionary") {
            Ok(Expr::ContainerLiteral(
                ContainerKind::Map,
                ContainerLiteralData::Map(vec![]),
            ))
        } else if type_name.starts_with("List") {
            Ok(Expr::ContainerLiteral(
                ContainerKind::Array,
                ContainerLiteralData::Array(vec![]),
            ))
        } else {
            // Use named args if available, otherwise use positional args converted to named
            if !named_args.is_empty() {
                Ok(Expr::StructNew(type_name, named_args))
            } else if !args.is_empty() {
                // Convert positional args to named args by field order (similar to Pup parser)
                // For now, just use empty named args - this will be handled during codegen
                Ok(Expr::StructNew(type_name, vec![]))
            } else {
                Ok(Expr::StructNew(type_name, vec![]))
            }
        }
    }

    fn parse_array_creation(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut size = None;
        let mut initializers = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "array_rank_specifier" => {
                    if let Some(size_node) = child.child(1) {
                        if let Ok(Expr::Literal(Literal::Number(n))) =
                            self.parse_expression(size_node)
                        {
                            size = n.parse::<usize>().ok();
                        }
                    }
                }
                "initializer_expression" => {
                    let mut init_cursor = child.walk();
                    for init_child in child.children(&mut init_cursor) {
                        if let Ok(expr) = self.parse_expression(init_child) {
                            initializers.push(expr);
                        }
                    }
                }
                _ => {}
            }
        }

        if !initializers.is_empty() {
            let len = initializers.len();
            Ok(Expr::ContainerLiteral(
                ContainerKind::FixedArray(len),
                ContainerLiteralData::FixedArray(len, initializers),
            ))
        } else if let Some(sz) = size {
            Ok(Expr::ContainerLiteral(
                ContainerKind::FixedArray(sz),
                ContainerLiteralData::FixedArray(sz, vec![]),
            ))
        } else {
            Ok(Expr::ContainerLiteral(
                ContainerKind::Array,
                ContainerLiteralData::Array(vec![]),
            ))
        }
    }

    fn parse_anonymous_object(&self, node: tree_sitter::Node) -> Result<Expr, String> {
        let mut pairs = Vec::new();

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "initializer_expression" {
                let mut init_cursor = child.walk();
                for init_child in child.children(&mut init_cursor) {
                    if init_child.kind() == "assignment_expression" {
                        let mut assign_cursor = init_child.walk();
                        let assign_children: Vec<tree_sitter::Node> =
                            init_child.children(&mut assign_cursor).collect();

                        if assign_children.len() >= 3 {
                            let key = self.get_node_text(assign_children[0]);
                            let value = self.parse_expression(assign_children[2])?;
                            pairs.push((Some(key), value));
                        }
                    }
                }
            }
        }

        Ok(Expr::ObjectLiteral(pairs))
    }

    fn parse_type(&self, node: tree_sitter::Node) -> Type {
        // Handle array_type nodes directly
        if node.kind() == "array_type" {
            // array_type has structure: base_type + array_rank_specifier
            // Find the base type (could be predefined_type, type, or identifier)
            let mut base_type_node = None;
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "predefined_type" | "type" | "identifier" | "generic_name" => {
                        base_type_node = Some(child);
                        break;
                    }
                    _ => {}
                }
            }

            if let Some(base_node) = base_type_node {
                let base_type = self.parse_type(base_node); // Recursively parse the base type
                // Wrap in Array container
                Type::Container(ContainerKind::Array, vec![base_type])
            } else {
                // Fallback: try to extract from text
                let text = self.get_node_text(node);
                let base_type_str = text.replace("[]", "").trim().to_string();
                let element_type = self.map_type(base_type_str);
                Type::Container(ContainerKind::Array, vec![element_type])
            }
        } else {
            // Check if this is an array type (has array_rank_specifier as a child)
            let mut cursor = node.walk();
            let has_array_spec = node.children(&mut cursor).any(|child| {
                child.kind() == "array_rank_specifier" || child.kind() == "[" || child.kind() == "]"
            });

            if has_array_spec {
                // For array types, get the base type and wrap it in Container
                let base_type_text = self.get_node_text(node);
                // Remove [] from the text to get base type
                let base_type_str = base_type_text.replace("[]", "").trim().to_string();
                let element_type = self.map_type(base_type_str);
                Type::Container(ContainerKind::Array, vec![element_type])
            } else {
                // Get the type text - it might be directly in the node or in an identifier child
                let type_text = if let Some(id_node) = self.get_child_by_kind(node, "identifier") {
                    self.get_node_text(id_node)
                } else {
                    self.get_node_text(node)
                };
                self.map_type(type_text)
            }
        }
    }

    fn map_type(&self, t: String) -> Type {
        // Handle qualified names like "System.Numerics.BigInteger" or just "BigInteger"
        let type_name = if t.contains('.') {
            t.split('.').last().unwrap_or(&t).to_string()
        } else {
            t.clone()
        };

        match type_name.as_str() {
            "void" => Type::Void,
            "float" => Type::Number(NumberKind::Float(32)),
            "double" => Type::Number(NumberKind::Float(64)),
            "decimal" => Type::Number(NumberKind::Decimal),
            "sbyte" => Type::Number(NumberKind::Signed(8)),
            "short" => Type::Number(NumberKind::Signed(16)),
            "int" => Type::Number(NumberKind::Signed(32)),
            "long" => Type::Number(NumberKind::Signed(64)),
            "byte" => Type::Number(NumberKind::Unsigned(8)),
            "ushort" => Type::Number(NumberKind::Unsigned(16)),
            "uint" => Type::Number(NumberKind::Unsigned(32)),
            "ulong" => Type::Number(NumberKind::Unsigned(64)),
            "bool" => Type::Bool,
            "char" => Type::Number(NumberKind::Unsigned(16)),
            "string" => Type::String,
            "object" | "var" => Type::Object,
            "BigInteger" => Type::Number(NumberKind::BigInt),
            ty if ty.starts_with("Dictionary") => {
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object])
            }
            ty if ty.starts_with("List") => {
                Type::Container(ContainerKind::Array, vec![Type::Object])
            }
            ty if ty.ends_with("[]") => {
                let base_type_str = ty.trim_end_matches("[]").to_string();
                let element_type = self.map_type(base_type_str);
                Type::Container(ContainerKind::Array, vec![element_type])
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

    fn has_expose_attribute(&self, attr_list: tree_sitter::Node) -> bool {
        let mut cursor = attr_list.walk();
        for child in attr_list.children(&mut cursor) {
            if child.kind() == "attribute" {
                let text = self.get_node_text(child);
                if text.contains("Expose") {
                    return true;
                }
            }
        }
        false
    }

    fn parse_attributes(&self, attr_list: tree_sitter::Node) -> Vec<String> {
        let mut attrs = Vec::new();
        let mut cursor = attr_list.walk();
        for child in attr_list.children(&mut cursor) {
            if child.kind() == "attribute" {
                // Extract attribute name - could be "AttributeName" or "AttributeName()"
                let text = self.get_node_text(child);
                // Remove parentheses and arguments if present
                let attr_name = text.split('(').next().unwrap_or(&text).trim().to_string();
                if !attr_name.is_empty() {
                    attrs.push(attr_name);
                }
            }
        }
        attrs
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
}
