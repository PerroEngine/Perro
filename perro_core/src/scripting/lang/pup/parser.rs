use std::collections::HashMap;

use crate::api_modules::*;
use crate::call_modules::CallModule;
use crate::ast::*;
use crate::lang::pup::api::{PupAPI, normalize_type_name};
use crate::lang::pup::resource_api::PupResourceAPI;
use crate::lang::pup::node_api::{PupNodeApiRegistry, PUP_NODE_API};
use crate::lang::pup::enums::resolve_enum_access;
use crate::lang::pup::lexer::{PupLexer, PupToken};

/// Convert PascalCase to snake_case (e.g., "Sprite2D" -> "sprite2d", "NodeType" -> "node_type")
fn pascal_to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch.is_uppercase() && !result.is_empty() {
            // Add underscore before uppercase (except at start)
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    
    result
}

pub struct PupParser {
    lexer: PupLexer,
    current_token: PupToken,
    /// Variable name → inferred type (for local scope/type inference during parsing)
    type_env: HashMap<String, Type>,
    pub parsed_structs: Vec<StructDef>,
    /// Pending attributes that were consumed at top level (for @AttributeName before var/fn)
    pending_attributes: Vec<String>,
    /// Source file path for source location tracking
    source_file: Option<String>,
    /// Start position of the current token (for source location tracking)
    current_token_line: u32,
    current_token_column: u32,
    /// If true, the parser will try to recover from incomplete syntax that is common
    /// while typing (e.g. `self.`) instead of hard-failing. Intended for LSP usage.
    error_tolerant: bool,
}

impl PupParser {
    pub fn new(input: &str) -> Self {
        let mut lex = PupLexer::new(input);
        // Capture position before getting first token
        let line = lex.current_line();
        let column = lex.current_column();
        let cur = lex.next_token();
        Self {
            lexer: lex,
            current_token: cur,
            type_env: HashMap::new(),
            parsed_structs: Vec::new(),
            pending_attributes: Vec::new(),
            source_file: None,
            current_token_line: line,
            current_token_column: column,
            error_tolerant: false,
        }
    }
    
    pub fn set_source_file(&mut self, file: String) {
        self.source_file = Some(file);
    }

    /// Enable/disable error-tolerant parsing (LSP-friendly).
    pub fn set_error_tolerant(&mut self, tolerant: bool) {
        self.error_tolerant = tolerant;
    }
    
    fn current_source_span(&self) -> Option<crate::scripting::source_span::SourceSpan> {
        self.source_file.as_ref().map(|file| {
            crate::scripting::source_span::SourceSpan {
                file: file.clone(),
                line: self.lexer.current_line(),
                column: self.lexer.current_column(),
                length: 1,
                language: "pup".to_string(),
            }
        })
    }
    
    fn typed_expr(&self, expr: Expr) -> TypedExpr {
        TypedExpr {
            expr,
            inferred_type: None,
            span: self.current_source_span(),
        }
    }

    fn next_token(&mut self) {
        // Capture the start position of the current token before advancing
        self.current_token_line = self.lexer.current_line();
        self.current_token_column = self.lexer.current_column();
        self.current_token = self.lexer.next_token();
    }

    fn expect(&mut self, tok: PupToken) -> Result<(), String> {
        if self.current_token == tok {
            self.next_token();
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", tok, self.current_token))
        }
    }

    // ============================================================
    // Script-Level Parsing
    // ============================================================

    pub fn parse_script(&mut self) -> Result<Script, String> {
        // Parse @script Name extends NodeType syntax
        let script_name = if self.current_token == PupToken::At {
            self.next_token();
            if self.current_token == PupToken::Script {
                self.next_token();
                if let PupToken::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.next_token();
                    Some(name)
                } else {
                    return Err("Expected script name after @script".into());
                }
            } else {
                return Err("Expected 'script' after '@'".into());
            }
        } else {
            None
        };

        self.expect(PupToken::Extends)?;
        let node_type = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after extends".into());
        };
        self.next_token();

        let mut script_vars = Vec::new(); // This is the unified, ordered list of all script-level variables
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut on_signal_functions = Vec::new(); // Track functions defined with "on SIGNALNAME()" syntax

        while self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::At => {
                    // Check if it's @expose (special directive) or @AttributeName (generic attribute)
                    self.next_token();
                    if self.current_token == PupToken::Expose {
                        // @expose is a special directive that also sets is_exposed
                        self.next_token();
                        let mut var = self.parse_variable_decl()?; // Parse `var name: type = value` part
                        var.is_exposed = true; // Mark this variable as exposed
                        var.is_public = true; // All top-level Pup variables are public
                        // Also add "Expose" as an attribute
                        if !var.attributes.contains(&"Expose".to_string()) {
                            var.attributes.push("Expose".to_string());
                        }
                        script_vars.push(var); // Add to the unified list
                    } else if let PupToken::Ident(attr_name) = &self.current_token {
                        // @AttributeName - store it as pending, will be picked up by parse_attributes()
                        self.pending_attributes.push(attr_name.clone());
                        self.next_token();
                        // Continue to parse the actual declaration (var/fn)
                        continue;
                    } else {
                        return Err(format!(
                            "Expected identifier after '@', got {:?}",
                            self.current_token
                        ));
                    }
                }
                PupToken::On => {
                    // Handle "on init() {}" (lifecycle) or "on SIGNALNAME() {}" (signal) syntax
                    self.next_token();
                    let name = if let PupToken::Ident(name) = &self.current_token {
                        name.clone()
                    } else {
                        return Err("Expected identifier after 'on'".into());
                    };
                    self.next_token();
                    
                    // Check if this is a lifecycle method (init, update, fixed_update)
                    let is_lifecycle = name == "init" || name == "update" || name == "fixed_update";
                    
                    if is_lifecycle {
                        // Parse as lifecycle method - not callable, but still a trait method
                        let mut func = self.parse_function_with_name(name.clone())?;
                        func.is_trait_method = true;
                        func.is_lifecycle_method = true;
                        functions.push(func);
                    } else {
                        // Parse as signal handler
                        let mut func = self.parse_function_with_name(name.clone())?;
                        func.is_on_signal = true;
                        func.signal_name = Some(name.clone());
                        on_signal_functions.push(name);
                        functions.push(func);
                    }
                }
                PupToken::Struct => {
                    let def = self.parse_struct_def()?;
                    self.parsed_structs.push(def.clone());
                    structs.push(def);
                }
                PupToken::Var | PupToken::Const => {
                    let mut var = self.parse_variable_decl()?; // Parse `var name: type = value` part
                    var.is_exposed = false; // Mark this variable as NOT exposed
                    var.is_public = true; // All top-level Pup variables are public
                    script_vars.push(var); // Add to the unified list
                }
                PupToken::Fn => functions.push(self.parse_function()?),
                other => {
                    return Err(format!("Unexpected top-level token {:?}", other));
                }
            }
        }

        // Auto-generate Signal.connect calls in init function for on-signal functions
        if !on_signal_functions.is_empty() {
            // Find or create init function
            let init_func = functions.iter_mut().find(|f| f.name == "init");
            if let Some(init_func) = init_func {
                // Add Signal.connect calls at the beginning of init
                for signal_name in &on_signal_functions {
                    let connect_stmt = self.create_signal_connect_stmt(signal_name.clone());
                    init_func.body.insert(0, connect_stmt);
                }
            } else {
                // Create init function if it doesn't exist
                let mut init_body = Vec::new();
                for signal_name in &on_signal_functions {
                    let connect_stmt = self.create_signal_connect_stmt(signal_name.clone());
                    init_body.push(connect_stmt);
                }
                functions.insert(0, Function {
                    name: "init".to_string(),
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: init_body,
                    is_trait_method: true,
                    uses_self: false,
                    cloned_child_nodes: Vec::new(),
                    return_type: Type::Void,
                    span: None,
                    attributes: Vec::new(),
                    is_on_signal: false,
                    signal_name: None,
                    is_lifecycle_method: false, // Auto-generated init is not from "on init()" syntax
                });
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
            script_name,
            node_type,
            variables: script_vars, // Pass the single, unified, and ordered list to the Script AST
            language: Some("pup".to_string()),
            source_file: None, // Will be set by transpiler
            functions,
            structs,
            verbose: true,
            attributes,
            module_names: std::collections::HashSet::new(), // Will be set by transpiler
            module_name_to_identifier: std::collections::HashMap::new(), // Will be set by transpiler
            module_functions: std::collections::HashMap::new(), // Will be set by transpiler
            module_variables: std::collections::HashMap::new(), // Will be set by transpiler
            module_scope_variables: None,
            is_global: false,
            global_names: std::collections::HashSet::new(), // Will be set by transpiler
            global_name_to_node_id: std::collections::HashMap::new(), // Will be set by transpiler
        })
    }

    /// Parse @global Name - like a script that always extends Node internally (no "extends" in source).
    /// Globals get deterministic NodeIDs: Root=1, first global=2, second=3, etc.
    pub fn parse_global(&mut self) -> Result<Script, String> {
        let global_name = if self.current_token == PupToken::At {
            self.next_token();
            if self.current_token == PupToken::Global {
                self.next_token();
                if let PupToken::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.next_token();
                    name
                } else {
                    return Err("Expected global name after @global".into());
                }
            } else {
                return Err("Expected 'global' after '@'".into());
            }
        } else {
            return Err("Expected @global declaration at start of file".into());
        };

        // No "extends" - always Node internally
        let mut script_vars = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut on_signal_functions = Vec::new();

        while self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::At => {
                    self.next_token();
                    if self.current_token == PupToken::Expose {
                        self.next_token();
                        let mut var = self.parse_variable_decl()?;
                        var.is_exposed = true;
                        var.is_public = true;
                        if !var.attributes.contains(&"Expose".to_string()) {
                            var.attributes.push("Expose".to_string());
                        }
                        script_vars.push(var);
                    } else if let PupToken::Ident(attr_name) = &self.current_token {
                        self.pending_attributes.push(attr_name.clone());
                        self.next_token();
                        continue;
                    } else {
                        return Err(format!(
                            "Expected identifier after '@', got {:?}",
                            self.current_token
                        ));
                    }
                }
                PupToken::On => {
                    self.next_token();
                    let name = if let PupToken::Ident(name) = &self.current_token {
                        name.clone()
                    } else {
                        return Err("Expected identifier after 'on'".into());
                    };
                    self.next_token();
                    let is_lifecycle = name == "init" || name == "update" || name == "fixed_update";
                    if is_lifecycle {
                        let mut func = self.parse_function_with_name(name.clone())?;
                        func.is_trait_method = true;
                        func.is_lifecycle_method = true;
                        functions.push(func);
                    } else {
                        let mut func = self.parse_function_with_name(name.clone())?;
                        func.is_on_signal = true;
                        func.signal_name = Some(name.clone());
                        on_signal_functions.push(name);
                        functions.push(func);
                    }
                }
                PupToken::Struct => {
                    let def = self.parse_struct_def()?;
                    self.parsed_structs.push(def.clone());
                    structs.push(def);
                }
                PupToken::Var | PupToken::Const => {
                    let mut var = self.parse_variable_decl()?;
                    var.is_exposed = false;
                    var.is_public = true;
                    script_vars.push(var);
                }
                PupToken::Fn => functions.push(self.parse_function()?),
                other => {
                    return Err(format!("Unexpected top-level token {:?}", other));
                }
            }
        }

        if !on_signal_functions.is_empty() {
            let init_func = functions.iter_mut().find(|f| f.name == "init");
            if let Some(init_func) = init_func {
                for signal_name in &on_signal_functions {
                    let connect_stmt = self.create_signal_connect_stmt(signal_name.clone());
                    init_func.body.insert(0, connect_stmt);
                }
            } else {
                let mut init_body = Vec::new();
                for signal_name in &on_signal_functions {
                    let connect_stmt = self.create_signal_connect_stmt(signal_name.clone());
                    init_body.push(connect_stmt);
                }
                functions.insert(0, Function {
                    name: "init".to_string(),
                    params: Vec::new(),
                    locals: Vec::new(),
                    body: init_body,
                    is_trait_method: true,
                    uses_self: false,
                    cloned_child_nodes: Vec::new(),
                    return_type: Type::Void,
                    span: None,
                    attributes: Vec::new(),
                    is_on_signal: false,
                    signal_name: None,
                    is_lifecycle_method: false,
                });
            }
        }

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
        for struct_def in &structs {
            for field in &struct_def.fields {
                if !field.attributes.is_empty() {
                    let qualified_name = format!("{}.{}", struct_def.name, field.name);
                    attributes.insert(qualified_name, field.attributes.clone());
                }
            }
        }

        Ok(Script {
            script_name: Some(global_name.clone()),
            node_type: "Node".to_string(), // Globals always use Node as nodetype
            variables: script_vars,
            language: Some("pup".to_string()),
            source_file: None,
            functions,
            structs,
            verbose: true,
            attributes,
            module_names: std::collections::HashSet::new(),
            module_name_to_identifier: std::collections::HashMap::new(),
            module_functions: std::collections::HashMap::new(),
            module_variables: std::collections::HashMap::new(),
            module_scope_variables: None,
            is_global: true,
            global_names: std::collections::HashSet::new(),
            global_name_to_node_id: std::collections::HashMap::new(),
        })
    }

    pub fn parse_module(&mut self) -> Result<Module, String> {
        // Parse @module Name syntax
        let module_name = if self.current_token == PupToken::At {
            self.next_token();
            if self.current_token == PupToken::Module {
                self.next_token();
                if let PupToken::Ident(name) = &self.current_token {
                    let name = name.clone();
                    self.next_token();
                    name
                } else {
                    return Err("Expected module name after @module".into());
                }
            } else {
                return Err("Expected 'module' after '@'".into());
            }
        } else {
            return Err("Expected @module declaration at start of file".into());
        };

        let mut module_vars = Vec::new(); // Constants and variables
        let mut functions = Vec::new();
        let mut structs = Vec::new();
        let mut attributes = HashMap::new();

        while self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::At => {
                    // Modules don't support @expose or other attributes
                    return Err("Modules do not support @ attributes. Only functions and constants are allowed.".into());
                }
                PupToken::On => {
                    // Modules don't support lifecycle methods
                    return Err("Modules do not support 'on' lifecycle methods. Only free functions are allowed.".into());
                }
                PupToken::Struct => {
                    let def = self.parse_struct_def()?;
                    self.parsed_structs.push(def.clone());
                    structs.push(def);
                }
                PupToken::Const => {
                    let mut var = self.parse_variable_decl()?;
                    var.is_exposed = false; // Modules don't expose
                    var.is_public = true; // Module top-level is public constants only
                    module_vars.push(var);
                }
                PupToken::Var => {
                    return Err("Modules only allow top-level constants (const), not variables (var).".into());
                }
                PupToken::Fn => {
                    let mut func = self.parse_function()?;
                    func.is_trait_method = false; // Modules don't have trait methods
                    func.is_lifecycle_method = false;
                    func.uses_self = false; // Module functions don't use self
                    functions.push(func);
                }
                other => {
                    return Err(format!("Unexpected top-level token in module: {:?}. Modules only support functions, variables (constants), and structs.", other));
                }
            }
        }

        // Build attributes HashMap
        for var in &module_vars {
            if !var.attributes.is_empty() {
                attributes.insert(var.name.clone(), var.attributes.clone());
            }
        }
        for func in &functions {
            if !func.attributes.is_empty() {
                attributes.insert(func.name.clone(), func.attributes.clone());
            }
        }
        for struct_def in &structs {
            for field in &struct_def.fields {
                if !field.attributes.is_empty() {
                    let qualified_name = format!("{}.{}", struct_def.name, field.name);
                    attributes.insert(qualified_name, field.attributes.clone());
                }
            }
        }

        Ok(Module {
            module_name,
            variables: module_vars,
            functions,
            structs,
            verbose: true,
            attributes,
            source_file: None, // Will be set by transpiler
            language: Some("pup".to_string()),
        })
    }

    // ================= STRUCTS, VARS, FUNCS ====================

    fn parse_struct_def(&mut self) -> Result<StructDef, String> {
        self.expect(PupToken::Struct)?;
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected struct name".into());
        };
        self.next_token();

        let mut base = None;
        if self.current_token == PupToken::Extends {
            self.next_token();
            if let PupToken::Ident(base_n) = &self.current_token {
                base = Some(base_n.clone());
                self.next_token();
            } else {
                return Err("Expected base struct name".into());
            }
        }

        self.expect(PupToken::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while self.current_token != PupToken::RBrace && self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::Fn => methods.push(self.parse_function()?),
                PupToken::Ident(_) | PupToken::Var => {
                    // Struct fields implicitly public, not exposed in this context.
                    // This is for struct internal fields, not script-level vars.
                    if self.current_token == PupToken::Var {
                        self.next_token();
                    }
                    fields.push(self.parse_field()?);
                    if self.current_token == PupToken::Comma {
                        self.next_token();
                    }
                }
                _ => break,
            }
        }

        self.expect(PupToken::RBrace)?;
        Ok(StructDef {
            name,
            fields,
            methods,
            base,
        })
    }

    fn parse_field(&mut self) -> Result<StructField, String> {
        // Parse attributes before field declaration
        let attributes = self.parse_attributes()?;

        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected field name".into());
        };
        self.next_token();
        self.expect(PupToken::Colon)?;
        Ok(StructField {
            name,
            typ: self.parse_type()?,
            attributes,
        })
    }

    fn parse_attributes(&mut self) -> Result<Vec<String>, String> {
        let mut attrs = Vec::new();

        // First, add any pending attributes that were consumed at top level
        attrs.extend(self.pending_attributes.drain(..));

        // Parse @AttributeName syntax (e.g., @Expose, @MyAttribute)
        while self.current_token == PupToken::At {
            self.next_token();
            if let PupToken::Ident(attr_name) = &self.current_token {
                attrs.push(attr_name.clone());
                self.next_token();
            } else {
                return Err("Expected attribute name after '@'".into());
            }
        }

        // Also support [attr1, attr2] syntax for backwards compatibility
        if self.current_token == PupToken::LBracket {
            self.next_token();
            // Handle empty attribute list []
            if self.current_token == PupToken::RBracket {
                self.next_token();
                return Ok(attrs);
            }
            loop {
                if let PupToken::Ident(attr_name) = &self.current_token {
                    attrs.push(attr_name.clone());
                    self.next_token();
                } else {
                    return Err("Expected attribute name".into());
                }
                if self.current_token == PupToken::Comma {
                    self.next_token();
                } else if self.current_token == PupToken::RBracket {
                    self.next_token();
                    break;
                } else {
                    return Err("Expected ',' or ']' in attribute list".into());
                }
            }
        }

        Ok(attrs)
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        // Parse attributes before function declaration
        let attributes = self.parse_attributes()?;

        self.expect(PupToken::Fn)?;
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected function name".into());
        };
        self.next_token();
        self.parse_function_with_name_and_attributes(name, attributes)
    }

    fn parse_function_with_name(&mut self, name: String) -> Result<Function, String> {
        self.parse_function_with_name_and_attributes(name, Vec::new())
    }

    fn parse_function_with_name_and_attributes(&mut self, name: String, attributes: Vec<String>) -> Result<Function, String> {
        self.expect(PupToken::LParen)?;
        let mut params = Vec::new();
        if self.current_token != PupToken::RParen {
            params.push(self.parse_param()?);
            while self.current_token == PupToken::Comma {
                self.next_token();
                params.push(self.parse_param()?);
            }
        }
        self.expect(PupToken::RParen)?;

        // Parse optional return type annotation: -> Type
        let mut return_type = Type::Void;
        if self.current_token == PupToken::Arrow {
            self.next_token(); // consume ->
            return_type = self.parse_type()?;
        }

        // Add function parameters to type environment so they can be recognized as node types
        // when parsing the function body (e.g., collision.get_parent() where collision is a parameter)
        for param in &params {
            self.type_env.insert(param.name.clone(), param.typ.clone());
        }

        let body = self.parse_block()?;
        
        // Remove function parameters from type environment after parsing body
        // (they're scoped to this function only)
        for param in &params {
            self.type_env.remove(&param.name);
        }
        let is_trait = name == "init" || name == "update" || name == "fixed_update";
        let locals = self.collect_locals(&body);

        Ok(Function {
            name,
            params,
            locals,
            body,
            is_trait_method: is_trait,
            uses_self: false,
            cloned_child_nodes: Vec::new(), // Will be populated during analyze_self_usage
            return_type,
            span: None,
            attributes, // Use the parsed attributes
            is_on_signal: false,
            signal_name: None,
            is_lifecycle_method: false, // Will be set to true if parsed with "on init()" syntax
        })
    }

    fn create_signal_connect_stmt(&self, signal_name: String) -> Stmt {
        use crate::scripting::ast::{Expr, Literal};
        // Create: Signal.connect("SIGNALNAME", function_name)
        // The function name is the same as the signal name, and it's on self
        use crate::resource_modules::{SignalResource, ResourceModule};
        Stmt::Expr(self.typed_expr(Expr::ApiCall(
            CallModule::Resource(ResourceModule::Signal(SignalResource::Connect)),
            vec![
                Expr::Literal(Literal::String(signal_name.clone())),
                Expr::Literal(Literal::String(signal_name)),
            ],
        )))
    }

    fn collect_locals(&self, body: &[Stmt]) -> Vec<Variable> {
        body.iter()
            .filter_map(|stmt| match stmt {
                Stmt::VariableDecl(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    fn parse_param(&mut self) -> Result<Param, String> {
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected param name".into());
        };
        self.next_token();
        self.expect(PupToken::Colon)?;
        Ok(Param {
            name,
            typ: self.parse_type()?,
            span: None,
        })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        if let PupToken::Ident(base) = &self.current_token {
            let base_name = base.clone();
            self.next_token();

            // Typed Map: Map<[K: V]>
            if self.current_token == PupToken::LessThan {
                self.next_token();
                self.expect(PupToken::LBracket)?;
                let key_type = self.parse_type()?;
                self.expect(PupToken::Colon)?;
                let val_type = self.parse_type()?;
                self.expect(PupToken::RBracket)?;
                self.expect(PupToken::GreaterThan)?;
                return Ok(Type::Container(
                    ContainerKind::Map,
                    vec![key_type, val_type],
                ));
            }

            // Handle both Array[T] and float[] syntaxes
            if self.current_token == PupToken::LBracket {
                self.next_token();

                // Check if empty brackets (shorthand: float[])
                if self.current_token == PupToken::RBracket {
                    self.next_token();
                    return Ok(Type::Container(
                        ContainerKind::Array,
                        vec![self.map_type(base_name)],
                    ));
                }

                // Otherwise parse inner type (explicit: Array[int])
                let inner = self.parse_type()?;
                self.expect(PupToken::RBracket)?;
                return Ok(Type::Container(ContainerKind::Array, vec![inner]));
            }

            Ok(self.map_type(base_name))
        } else {
            Err("Expected type".into())
        }
    }

    // ======================= BLOCKS/STMTS =======================

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(PupToken::LBrace)?;
        let mut body = Vec::new();
        while self.current_token != PupToken::RBrace && self.current_token != PupToken::Eof {
            body.push(self.parse_statement()?);
        }
        self.expect(PupToken::RBrace)?;
        Ok(body)
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        if self.current_token == PupToken::Var {
            return Ok(Stmt::VariableDecl(self.parse_variable_decl()?));
        }
        if self.current_token == PupToken::Pass {
            self.next_token();
            return Ok(Stmt::Pass);
        }
        if self.current_token == PupToken::Return {
            return self.parse_return_statement();
        }
        if self.current_token == PupToken::If {
            return self.parse_if_statement();
        }
        if self.current_token == PupToken::For {
            return self.parse_for_loop();
        }

        // Note: Increment/decrement (i++, i--) are handled in parse_statement
        // by checking after parsing an identifier expression, not by peeking ahead
        // This avoids token restoration issues

        // Capture position of the current token - this is where the expression starts
        let expr_start_line = self.current_token_line;
        let expr_start_column = self.current_token_column;
        let lhs = self.parse_expression(0)?;

        // Handle increment/decrement operators (i++, i--) after parsing expression
        if let Expr::Ident(name) = &lhs {
            if self.current_token == PupToken::PlusPlus {
                self.next_token();
                return Ok(Stmt::AssignOp(
                    name.clone(),
                    Op::Add,
                    self.typed_expr(Expr::Literal(Literal::Number("1".to_string()))),
                ));
            }
            if self.current_token == PupToken::MinusMinus {
                self.next_token();
                return Ok(Stmt::AssignOp(
                    name.clone(),
                    Op::Sub,
                    self.typed_expr(Expr::Literal(Literal::Number("1".to_string()))),
                ));
            }
        }
        if let Some(op) = self.take_assign_op() {
            let rhs = self.parse_expression(0)?;
            return self.make_assign_stmt(lhs, op, rhs);
        }

        // Create TypedExpr with the position we captured at the start of the expression
        Ok(Stmt::Expr(TypedExpr {
            expr: lhs,
            inferred_type: None,
            span: self.source_file.as_ref().map(|file| {
                crate::scripting::source_span::SourceSpan {
                    file: file.clone(),
                    line: expr_start_line,
                    column: expr_start_column,
                    length: 1,
                    language: "pup".to_string(),
                }
            }),
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, String> {
        self.expect(PupToken::If)?;

        // Parentheses are optional for if statements
        let condition = if self.current_token == PupToken::LParen {
            self.next_token();
            let cond = self.parse_expression(0)?;
            self.expect(PupToken::RParen)?;
            cond
        } else {
            self.parse_expression(0)?
        };

        let then_body = self.parse_block()?;

        let else_body = if self.current_token == PupToken::Else {
            self.next_token();
            Some(self.parse_block()?)
        } else {
            None
        };

        Ok(Stmt::If {
            condition: TypedExpr {
                expr: condition,
                inferred_type: None,
                span: None,
            },
            then_body,
            else_body,
        })
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, String> {
        self.expect(PupToken::Return)?;
        
        // Check if there's an expression after return
        if self.current_token == PupToken::Semicolon || self.current_token == PupToken::RBrace || self.current_token == PupToken::Eof {
            // return; (no expression)
            Ok(Stmt::Return(None))
        } else {
            // return expr;
            let expr = self.parse_expression(0)?;
            Ok(Stmt::Return(Some(TypedExpr {
                expr,
                inferred_type: None,
                span: None,
            })))
        }
    }

    fn parse_for_loop(&mut self) -> Result<Stmt, String> {
        self.expect(PupToken::For)?;
        self.expect(PupToken::LParen)?;

        // Check if it's a traditional for loop (starts with 'var') or range-based (has 'in')
        // We'll parse the first part and check if next token is 'in'

        if matches!(self.current_token, PupToken::Var) {
            // Traditional for loop: for (var i = 0, i < 10, i++) { body }
            // Traditional for loop: for (init, condition, increment) { body }
            // Uses commas instead of semicolons since pup doesn't use semicolons
            let init = if self.current_token == PupToken::Comma {
                self.next_token();
                None
            } else {
                let init_stmt = self.parse_statement()?;
                self.expect(PupToken::Comma)?;
                Some(Box::new(init_stmt))
            };

            let condition = if self.current_token == PupToken::Comma {
                self.next_token();
                None
            } else {
                let cond = self.parse_expression(0)?;
                self.expect(PupToken::Comma)?;
                Some(TypedExpr {
                    expr: cond,
                    inferred_type: None,
                    span: None,
                })
            };

            let increment = if self.current_token == PupToken::RParen {
                None
            } else {
                let incr_stmt = self.parse_statement()?;
                Some(Box::new(incr_stmt))
            };

            self.expect(PupToken::RParen)?;
            let body = self.parse_block()?;

            Ok(Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            })
        } else {
            // Range-based for loop: for (var in iterable) { body }
            // Parse identifier first
            let var_name = if let PupToken::Ident(name) = &self.current_token {
                let name = name.clone();
                self.next_token();
                name
            } else {
                return Err("Expected identifier in for loop".to_string());
            };

            // Check if next token is 'in' - if not, it might be a traditional loop without 'var'
            if self.current_token != PupToken::In {
                // This might be a traditional loop like: for (i = 0, i < 10, i++)
                // But we've already consumed the identifier, so we need to handle this differently
                // For now, let's assume if there's no 'in', it's an error for range-based
                return Err(format!(
                    "Expected 'in' after identifier in for loop, got {:?}",
                    self.current_token
                ));
            }

            self.expect(PupToken::In)?;
            let iterable = self.parse_expression(0)?;
            self.expect(PupToken::RParen)?;

            let body = self.parse_block()?;

            Ok(Stmt::For {
                var_name,
                iterable: self.typed_expr(iterable),
                body,
            })
        }
    }

    fn make_assign_stmt(&mut self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
        let typed_rhs = self.typed_expr(rhs);

        match lhs {
            Expr::Ident(name) => {
                // Note: Constant reassignment validation will be done in codegen
                // where we have access to the full script/module context
                Ok(match op {
                    None => Stmt::Assign(name, typed_rhs),
                    Some(op) => Stmt::AssignOp(name, op, typed_rhs),
                })
            },
            Expr::MemberAccess(obj, field) => {
                // Handle both single-level and nested MemberAccess (e.g., s.transform.position.y)
                let typed_lhs = self.typed_expr(Expr::MemberAccess(obj, field));
                Ok(match op {
                    None => Stmt::MemberAssign(typed_lhs, typed_rhs),
                    Some(op) => Stmt::MemberAssignOp(typed_lhs, op, typed_rhs),
                })
            }
            Expr::ApiCall(CallModule::NodeMethod(crate::structs::engine_registry::NodeMethodRef::GetVar), args) => {
                if args.len() == 2 {
                    let node = args[0].clone();
                    let field = args[1].clone();
                    // Extract node variable name and field name for ScriptAssign/ScriptAssignOp
                    if let (Expr::Ident(node_var), Expr::Literal(Literal::String(field_name))) = (&args[0], &args[1]) {
                        // Use ScriptAssign/ScriptAssignOp for proper codegen
                        Ok(match op {
                            None => Stmt::ScriptAssign(node_var.clone(), field_name.clone(), typed_rhs),
                            Some(op) => Stmt::ScriptAssignOp(node_var.clone(), field_name.clone(), op, typed_rhs),
                        })
                    } else {
                        // Fallback to SetVar API call for complex expressions
                        use crate::structs::engine_registry::NodeMethodRef;
                        Ok(Stmt::Expr(self.typed_expr(Expr::ApiCall(
                            CallModule::NodeMethod(NodeMethodRef::SetVar),
                            vec![node, field, typed_rhs.expr],
                        ))))
                    }
                } else {
                    Err("Invalid get_var arg count".into())
                }
            }
            Expr::Index(obj, key) => Ok(match op {
                None => Stmt::IndexAssign(obj, key, typed_rhs),
                Some(bop) => Stmt::IndexAssignOp(obj, key, bop, typed_rhs),
            }),
            other => Err(format!("Invalid assignment target: {:?}", other)),
        }
    }

    fn take_assign_op(&mut self) -> Option<Option<Op>> {
        let op = match self.current_token {
            PupToken::Assign => Some(None),
            PupToken::PlusEq => Some(Some(Op::Add)),
            PupToken::MinusEq => Some(Some(Op::Sub)),
            PupToken::MulEq => Some(Some(Op::Mul)),
            PupToken::DivEq => Some(Some(Op::Div)),
            _ => None,
        };
        if op.is_some() {
            self.next_token();
        }
        op
    }

    // =================== VARIABLE DECLARATIONS ==================

    fn parse_variable_decl(&mut self) -> Result<Variable, String> {
        // Parse attributes before variable declaration
        let attributes = self.parse_attributes()?;

        // Check if it's const or var
        let is_const = if self.current_token == PupToken::Const {
            self.next_token();
            true
        } else {
            self.expect(PupToken::Var)?;
            false
        };
        
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected var name".into());
        };
        self.next_token();

        let mut typ: Option<Type> = None;
        let mut value: Option<TypedExpr> = None;

        if self.current_token == PupToken::Colon {
            self.next_token();
            typ = Some(self.parse_type()?);
        }

        if self.current_token == PupToken::Assign {
            self.next_token();
            let expr = self.parse_expression(0)?;

            if typ.is_none() {
                typ = match &expr {
                    Expr::Literal(Literal::Number(_)) => Some(Type::Number(NumberKind::Float(32))),
                    Expr::Literal(Literal::String(_)) | Expr::Literal(Literal::Interpolated(_)) => {
                        Some(Type::String)
                    }
                    Expr::Literal(Literal::Bool(_)) => Some(Type::Bool),
                    Expr::Literal(Literal::Null) => None, // null can be assigned to any Option<T>, type will be inferred from context

                    Expr::ContainerLiteral(kind, _) => match kind {
                        ContainerKind::Map => Some(Type::Container(
                            ContainerKind::Map,
                            vec![Type::String, Type::Object],
                        )),
                        ContainerKind::Array => {
                            Some(Type::Container(ContainerKind::Array, vec![Type::Object]))
                        }
                        _ => None, // or panic!("Unexpected kind for ContainerLiteral in infer_expr_type")
                    },
                    Expr::ObjectLiteral(_) => Some(Type::Object),
                    Expr::Cast(_, target) => Some(target.clone()),
                    Expr::Ident(var_name) => self.type_env.get(var_name).cloned(),
                    Expr::StructNew(type_name, _) => {
                        // Check if it's a node type
                        if crate::scripting::codegen::is_node_type(type_name) {
                            // Convert node type name to Type::Node
                            use crate::structs::engine_registry::ENGINE_REGISTRY;
                            if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
                                format!("{:?}", nt) == *type_name
                            }) {
                                Some(Type::Node(node_type.clone()))
                            } else {
                                None
                            }
                        } else if crate::structs::engine_structs::EngineStruct::is_engine_struct(type_name) {
                            // Engine struct
                            if let Some(engine_struct) = crate::structs::engine_structs::EngineStruct::from_string(type_name) {
                                Some(Type::EngineStruct(engine_struct))
                            } else {
                                None
                            }
                        } else {
                            // Custom struct
                            Some(Type::Custom(type_name.clone()))
                        }
                    }
                    Expr::Call(inner, _) => {
                        if let Expr::MemberAccess(base, method) = &**inner {
                            if method == "new" {
                                if let Expr::Ident(id) = &**base {
                                    Some(Type::Custom(id.clone()))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
            }

            value = Some(TypedExpr {
                expr,
                inferred_type: typ.clone(),
                span: None,
            });
        }

        if let Some(t) = &typ {
            self.type_env.insert(name.clone(), t.clone());
        }

        // Initialize is_exposed and is_public with defaults; these will be overwritten
        // by the parse_script logic if it's an @expose var.
        Ok(Variable {
            name,
            typ,
            value,
            is_exposed: false,
            is_public: false,
            is_const,
            span: None,
            attributes,
        })
    }

    // ====================== EXPRESSIONS =========================

    fn parse_expression(&mut self, prec: u8) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while prec < self.get_precedence() {
            left = self.parse_infix(left)?;
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match &self.current_token {
            PupToken::LessThan => {
                self.next_token();

                // Handle empty map case
                if self.current_token == PupToken::GreaterThan {
                    self.next_token();
                    return Ok(Expr::ContainerLiteral(
                        ContainerKind::Map,
                        ContainerLiteralData::Map(vec![]),
                    ));
                }

                let mut pairs = Vec::new();

                // Loop through [key: val] blocks separated by commas
                loop {
                    self.expect(PupToken::LBracket)?;

                    // Parse key
                    let key_expr = self.parse_expression(0)?;
                    self.expect(PupToken::Colon)?;
                    let val = self.parse_expression(0)?;
                    self.expect(PupToken::RBracket)?;

                    pairs.push((key_expr, val));

                    if self.current_token == PupToken::Comma {
                        self.next_token();
                        continue;
                    }

                    break;
                }

                self.expect(PupToken::GreaterThan)?;
                Ok(Expr::ContainerLiteral(
                    ContainerKind::Map,
                    ContainerLiteralData::Map(pairs),
                ))
            }
            PupToken::LBracket => {
                self.next_token();
                let mut elements = Vec::new();
                while self.current_token != PupToken::RBracket
                    && self.current_token != PupToken::Eof
                {
                    elements.push(self.parse_expression(0)?);
                    if self.current_token == PupToken::Comma {
                        self.next_token();
                    } else {
                        break;
                    }
                }
                self.expect(PupToken::RBracket)?;
                Ok(Expr::ContainerLiteral(
                    ContainerKind::Array,
                    ContainerLiteralData::Array(elements),
                ))
            }
            PupToken::LBrace => {
                self.next_token();
                let mut pairs = Vec::new();
                while self.current_token != PupToken::RBrace && self.current_token != PupToken::Eof
                {
                    let k = match &self.current_token {
                        PupToken::Ident(k) | PupToken::String(k) => k.clone(),
                        other => return Err(format!("Expected key in object, got {:?}", other)),
                    };
                    self.next_token();
                    self.expect(PupToken::Colon)?;
                    let v = self.parse_expression(0)?;
                    pairs.push((Some(k), v));
                    if self.current_token == PupToken::Comma {
                        self.next_token();
                    } else {
                        break;
                    }
                }
                self.expect(PupToken::RBrace)?;
                Ok(Expr::ObjectLiteral(pairs))
            }
            PupToken::New => {
                self.next_token();

                // --- Parse the type name after `new` ---
                let type_name = match &self.current_token {
                    PupToken::Ident(n) => n.clone(),
                    _ => return Err("Expected identifier after 'new'".into()),
                };
                self.next_token();

                // --- Check what comes next: '(' positional or '{' named ---
                match &self.current_token {
                    PupToken::LParen => {
                        // -----------------------------
                        // new Struct(...positional...)
                        // -----------------------------
                        self.next_token();
                        let mut args = Vec::new();
                        if self.current_token != PupToken::RParen {
                            args.push(self.parse_expression(0)?);
                            while self.current_token == PupToken::Comma {
                                self.next_token();
                                args.push(self.parse_expression(0)?);
                            }
                        }
                        self.expect(PupToken::RParen)?;

                        // 🧠 If this is a known struct type — convert positional args to matched field pairs
                        let is_custom_struct =
                            self.parsed_structs.iter().any(|s| s.name == type_name);

                        if is_custom_struct {
                            // No args → default construct
                            if args.is_empty() {
                                return Ok(Expr::StructNew(type_name, vec![]));
                            }

                            // Map args by field order
                            fn gather_flat_fields(
                                def: &StructDef,
                                all: &[StructDef],
                                out: &mut Vec<String>,
                            ) {
                                if let Some(ref base) = def.base {
                                    if let Some(base_def) = all.iter().find(|s| &s.name == base) {
                                        gather_flat_fields(base_def, all, out);
                                    }
                                }
                                for f in &def.fields {
                                    out.push(f.name.clone());
                                }
                            }

                            // Build full flattened field list
                            let mut matched_fields = Vec::new();
                            if let Some(def) =
                                self.parsed_structs.iter().find(|s| s.name == type_name)
                            {
                                gather_flat_fields(def, &self.parsed_structs, &mut matched_fields);
                            }

                            let mut pairs = Vec::new();
                            for (i, arg) in args.iter().enumerate() {
                                let field_name = matched_fields
                                    .get(i)
                                    .cloned()
                                    .unwrap_or_else(|| format!("_{}", i));
                                pairs.push((field_name, arg.clone()));
                            }
                            return Ok(Expr::StructNew(type_name, pairs));
                        }

                        // Check if it's a node type - if so, use StructNew with no args
                        // Node types don't take constructor arguments (name is set via .name = "" later)
                        if crate::scripting::codegen::is_node_type(&type_name) {
                            // Ignore any arguments - node constructors take no parameters
                            return Ok(Expr::StructNew(type_name, vec![]));
                        }

                        // Check if it's an engine struct - if so, use StructNew
                        // Engine structs like Vector2, Color, etc. have constructors
                        if crate::structs::engine_structs::EngineStruct::is_engine_struct(&type_name) {
                            // Convert positional args to field pairs (we'll handle this in codegen)
                            // For now, just store them as empty field names - codegen will handle ::new()
                            let pairs: Vec<(String, Expr)> = args
                                .into_iter()
                                .enumerate()
                                .map(|(i, expr)| (format!("_{}", i), expr))
                                .collect();
                            return Ok(Expr::StructNew(type_name, pairs));
                        }

                        // Otherwise treat `new Something()` as an API call or method
                        if let Some(api) = PupAPI::resolve(&type_name, "new") {
                            return Ok(Expr::ApiCall(CallModule::Module(api), args));
                        }

                        Ok(Expr::Call(
                            Box::new(Expr::MemberAccess(
                                Box::new(Expr::Ident(type_name.clone())),
                                "new".into(),
                            )),
                            args,
                        ))
                    }

                    PupToken::LBrace => {
                        // -----------------------------
                        // new Struct { field: expr, ... }
                        // -----------------------------
                        self.next_token();
                        let mut pairs = Vec::new();

                        while self.current_token != PupToken::RBrace
                            && self.current_token != PupToken::Eof
                        {
                            // field name
                            let field_name = match &self.current_token {
                                PupToken::Ident(n) | PupToken::String(n) => n.clone(),
                                other => {
                                    return Err(format!(
                                        "Expected field name in struct init, got {:?}",
                                        other
                                    ));
                                }
                            };
                            self.next_token();
                            self.expect(PupToken::Colon)?;
                            let expr = self.parse_expression(0)?;
                            pairs.push((field_name, expr));

                            if self.current_token == PupToken::Comma {
                                self.next_token();
                            } else {
                                break;
                            }
                        }

                        self.expect(PupToken::RBrace)?;

                        // Emit it as a StructNew (so codegen path stays the same)
                        Ok(Expr::StructNew(type_name, pairs))
                    }

                    other => Err(format!(
                        "Expected '(' or '{{' after 'new <Type>', got {:?}",
                        other
                    )),
                }
            }
            PupToken::SelfAccess => {
                self.next_token();
                Ok(Expr::SelfAccess)
            }
            PupToken::Super => {
                self.next_token();
                Ok(Expr::BaseAccess)
            }
            PupToken::True => {
                self.next_token();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            PupToken::False => {
                self.next_token();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            PupToken::Null => {
                self.next_token();
                Ok(Expr::Literal(Literal::Null))
            }
            PupToken::Ident(n) => {
                let name = n.clone();
                self.next_token();
                Ok(Expr::Ident(name))
            }
            PupToken::Number(n) => {
                let val = n.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::Number(val)))
            }
            PupToken::String(s) => {
                let val = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::String(val)))
            }
            PupToken::InterpolatedString(s) => {
                let val = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::Interpolated(val)))
            }
            PupToken::LParen => {
                self.next_token();
                let expr = self.parse_expression(0)?;
                self.expect(PupToken::RParen)?;
                Ok(expr)
            }
            _ => Err(format!(
                "Unexpected token {:?} in primary",
                self.current_token
            )),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, String> {
        match &self.current_token {
            PupToken::LBracket => {
                self.next_token();
                let index = self.parse_expression(0)?;
                self.expect(PupToken::RBracket)?;
                Ok(Expr::Index(Box::new(left), Box::new(index)))
            }
            PupToken::As => {
                self.next_token();
                let tstr = match &self.current_token {
                    PupToken::Ident(i) => i.clone(),
                    _ => return Err("Expected type".into()),
                };
                self.next_token();
                Ok(Expr::Cast(Box::new(left), self.map_type(tstr)))
            }
            PupToken::LParen => {
                self.next_token();
                let mut args = Vec::new();
                if self.current_token != PupToken::RParen {
                    args.push(self.parse_expression(0)?);
                    while self.current_token == PupToken::Comma {
                        self.next_token();
                        args.push(self.parse_expression(0)?);
                    }
                }
                self.expect(PupToken::RParen)?;

                // API sugar handling...
                // Handle case where left is already an ApiCall (shouldn't happen, but handle gracefully)
                if let Expr::ApiCall(_, _) = &left {
                    // This means the Dot handler converted it too early - treat as function call
                    return Ok(Expr::Call(Box::new(left), args));
                }
                
                if let Expr::MemberAccess(obj, method) = &left {
                    // Handle nested member access like Input.Keyboard.is_key_pressed
                    // Check if obj is itself a MemberAccess (e.g., Input.Keyboard)
                    if let Expr::MemberAccess(inner_obj, _inner_field) = &**obj {
                        // Check if the inner object is an API module (like Input)
                        if let Expr::Ident(mod_name) = &**inner_obj {
                            // Check if this is an API module - try to resolve it
                            if let Some(api) = PupAPI::resolve(mod_name, method) {
                                // For Input.Keyboard.method or Input.Mouse.method, 
                                // resolve it as Input.method (the API binding handles Keyboard/Mouse prefix)
                                return Ok(Expr::ApiCall(CallModule::Module(api), args));
                            }
                        }
                    }
                    
                    // Check if obj is a node instance (self or node variable)
                    let is_node_instance = match &**obj {
                        Expr::SelfAccess => true,
                        Expr::Ident(var_name) => {
                            // Check if variable is a node type
                            if let Some(var_type) = self.type_env.get(var_name) {
                                matches!(var_type, Type::Node(_) | Type::DynNode)
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    
                    // Resolve by receiver type first: if variable has a resource type (Texture, Signal, etc.),
                    // try that resource API so tex.remove() -> Texture.remove(tex), not remove_node.
                    if let Expr::Ident(var_name) = &**obj {
                        if let Some(var_type) = self.type_env.get(var_name) {
                            let norm_type_name = normalize_type_name(var_type);
                            if !norm_type_name.is_empty() {
                                if let Some(api) = PupAPI::resolve(&norm_type_name, method) {
                                    let mut call_args = vec![*obj.clone()];
                                    call_args.extend(args);
                                    return Ok(Expr::ApiCall(CallModule::Module(api), call_args));
                                }
                                if let Some(resource) = PupResourceAPI::resolve(&norm_type_name, method) {
                                    let mut call_args = vec![*obj.clone()];
                                    call_args.extend(args);
                                    return Ok(Expr::ApiCall(CallModule::Resource(resource), call_args));
                                }
                            }
                        }
                    }
                    
                    // GetVar/SetVar are special node methods — always resolve to ApiCall(GetVar/SetVar)
                    // when the receiver is a node (including DynNode), so codegen uses get_script_var/set_script_var
                    // instead of read_node(..., |n| n.get_var)(...).
                    if is_node_instance && (method == "get_var" || method == "set_var") {
                        if method == "get_var" && args.len() == 1 {
                            if let Some(method_def) = PUP_NODE_API.get_methods(&crate::node_registry::NodeType::Node)
                                .iter()
                                .find(|m| m.script_name == "get_var")
                            {
                                let mut args_full = vec![*obj.clone()];
                                args_full.extend(args.iter().cloned());
                                return Ok(Expr::ApiCall(
                                    CallModule::NodeMethod(method_def.rust_method),
                                    args_full,
                                ));
                            }
                        }
                        if method == "set_var" && args.len() == 2 {
                            if let Some(method_def) = PUP_NODE_API.get_methods(&crate::node_registry::NodeType::Node)
                                .iter()
                                .find(|m| m.script_name == "set_var")
                            {
                                let mut args_full = vec![*obj.clone()];
                                args_full.extend(args.iter().cloned());
                                return Ok(Expr::ApiCall(
                                    CallModule::NodeMethod(method_def.rust_method),
                                    args_full,
                                ));
                            }
                        }
                    }

                    // Check node API registry only when receiver is actually a node (self or variable typed Node/DynNode).
                    // Do not fall back to node methods for other Idents (e.g. Texture.remove(tex) must be Texture API).
                    let node_type_to_check = match &**obj {
                        Expr::SelfAccess => Some(crate::node_registry::NodeType::Node),
                        Expr::Ident(var_name) => {
                            if let Some(Type::Node(nt)) = self.type_env.get(var_name) {
                                Some(*nt)
                            } else {
                                None // Only use node API when type is actually Node/DynNode
                            }
                        }
                        _ => None,
                    };
                    
                    if is_node_instance {
                        if let Some(nt) = node_type_to_check {
                            // Check if method exists in node API registry (including inherited methods)
                            if let Some(method_def) = PUP_NODE_API.get_methods(&nt)
                                .iter()
                                .find(|m| m.script_name == method)
                            {
                                // It's a node method - use NodeMethodRef from the method definition
                                use crate::structs::engine_registry::NodeMethodRef;
                                
                                // Handle self.get_node("name") - special case
                                if matches!(obj.as_ref(), Expr::SelfAccess) && method == "get_node" {
                                    let mut args_full = vec![Expr::SelfAccess];
                                    args_full.extend(args);
                                    return Ok(Expr::ApiCall(
                                        CallModule::NodeMethod(method_def.rust_method),
                                        args_full
                                    ));
                                }
                                // For other node methods, add obj as first arg
                                let mut args_full = vec![*obj.clone()];
                                args_full.extend(args);
                                return Ok(Expr::ApiCall(
                                    CallModule::NodeMethod(method_def.rust_method),
                                    args_full
                                ));
                            }
                        }
                    }
                    
                    // Check PupAPI::resolve for module APIs (Time, JSON, etc.)
                    if let Expr::Ident(mod_name) = &**obj {
                        if let Some(api) = PupAPI::resolve(mod_name, method) {
                            return Ok(Expr::ApiCall(CallModule::Module(api), args));
                        }
                        // Check PupResourceAPI::resolve for resource APIs (Signal, Texture, etc.)
                        if let Some(resource) = PupResourceAPI::resolve(mod_name, method) {
                            return Ok(Expr::ApiCall(CallModule::Resource(resource), args));
                        }
                    }
                    
                    // Check if obj is a variable that might be a resource type
                    if let Expr::Ident(var_name) = &**obj {
                        if let Some(var_type) = self.type_env.get(var_name) {
                            let norm_type_name = normalize_type_name(var_type);
                            if !norm_type_name.is_empty() {
                                // Try module APIs first
                                if let Some(api) = PupAPI::resolve(&norm_type_name, method) {
                                    let mut call_args = vec![*obj.clone()];
                                    call_args.extend(args);
                                    return Ok(Expr::ApiCall(CallModule::Module(api), call_args));
                                }
                                // Try resource APIs
                                if let Some(resource) = PupResourceAPI::resolve(&norm_type_name, method) {
                                    let mut call_args = vec![*obj.clone()];
                                    call_args.extend(args);
                                    return Ok(Expr::ApiCall(CallModule::Resource(resource), call_args));
                                }
                            }
                        }
                    }
                }
                Ok(Expr::Call(Box::new(left), args))
            }
            PupToken::Dot => {
                self.next_token();

                let (field_name, should_advance) = match &self.current_token {
                    PupToken::Ident(n) => (n.clone(), true),
                    PupToken::New => ("new".to_string(), true), // ✅ allow `.new` keyword
                    PupToken::Struct => ("struct".to_string(), true), // (optional future‑proof)
                    // While typing, users frequently have `something.` with no field yet.
                    // In error-tolerant mode, recover by producing an empty MemberAccess node
                    // and DO NOT consume the current token (so higher-level parsers can continue).
                    _ if self.error_tolerant => (String::new(), false),
                    _ => {
                        return Err(format!(
                            "Expected field after '.', got {:?}",
                            self.current_token
                        ));
                    }
                };

                if should_advance {
                    self.next_token();
                }
                
                // Check if this is enum access (e.g., NODE_TYPE.Sprite2D)
                // Enum names must be SCREAMING_SNAKE_CASE (all caps with underscores)
                if let Expr::Ident(enum_type_name) = &left {
                    if let Some(enum_variant) = resolve_enum_access(enum_type_name, &field_name) {
                        return Ok(Expr::EnumAccess(enum_variant));
                    }
                }
                
                // Check if this is API module field access (e.g., Shape2D.rectangle -> Shape2D.rectangle())
                // NOTE: We should NOT eagerly convert to ApiCall here, because if there's a following '(',
                // the LParen handler needs to see the MemberAccess to properly convert it with arguments.
                // Only convert if we're sure there's no following call (but we can't peek ahead easily).
                // Instead, let the LParen handler do the conversion - it will check for API modules.
                // This fixes the issue where Console.info(125) was being parsed as api.print_info("")(125f32)
                // 
                // If we really need field-like access without parentheses, we could add a check here,
                // but for now, let's keep it as MemberAccess and let the call handler convert it.
                // 
                // if let Expr::Ident(mod_name) = &left {
                //     let method_name = pascal_to_snake_case(&field_name);
                //     if let Some(api) = PupAPI::resolve(mod_name, &method_name) {
                //         // Only convert if next token is NOT LParen
                //         // But we can't easily peek ahead in this parser structure
                //         // So we'll let the LParen handler do it
                //     }
                // }
                
                Ok(Expr::MemberAccess(Box::new(left), field_name))
            }
            PupToken::DoubleColon => {
                self.next_token();
                
                // Check for dynamic access syntax: ::[expr] or ::(expr)
                let var_name_expr = if self.current_token == PupToken::LBracket {
                    // Dynamic access with brackets: VARNAME::[expr]
                    self.next_token();
                    let expr = self.parse_expression(2)?;
                    self.expect(PupToken::RBracket)?;
                    expr
                } else if self.current_token == PupToken::LParen {
                    // Dynamic access with parentheses: VARNAME::(expr)
                    self.next_token();
                    let expr = self.parse_expression(2)?;
                    self.expect(PupToken::RParen)?;
                    expr
                } else if let PupToken::Ident(n) = &self.current_token {
                    // Static access: VARNAME::var_name
                    let name = n.clone();
                    self.next_token();
                    Expr::Literal(Literal::String(name))
                } else {
                    return Err("Expected identifier, '[', or '(' after '::'".into());
                };
                
                // Check if this is a method call (followed by '(') or variable access
                if self.current_token == PupToken::LParen {
                    // Method call: VARNAME::method_name(params) or VARNAME::[expr](params)
                    // Support both static and dynamic function names
                    self.next_token(); // consume '('
                    let mut args = Vec::new();
                    if self.current_token != PupToken::RParen {
                        args.push(self.parse_expression(2)?);
                        while self.current_token == PupToken::Comma {
                            self.next_token();
                            args.push(self.parse_expression(2)?);
                        }
                    }
                    self.expect(PupToken::RParen)?;
                    // Build args: [node_expr, method_name_expr, ...params]
                    // method_name_expr can be either a literal string (static) or an expression (dynamic)
                    let mut call_args = vec![left, var_name_expr];
                    call_args.extend(args);
                    use crate::structs::engine_registry::NodeMethodRef;
                    Ok(Expr::ApiCall(
                        CallModule::NodeMethod(NodeMethodRef::CallFunction),
                        call_args,
                    ))
                } else if self.current_token == PupToken::Assign {
                    // Variable assignment: VARNAME::var_name = value or VARNAME::[expr] = value
                    self.next_token();
                    let val = self.parse_expression(2)?;
                    use crate::structs::engine_registry::NodeMethodRef;
                    Ok(Expr::ApiCall(
                        CallModule::NodeMethod(NodeMethodRef::SetVar),
                        vec![left, var_name_expr, val],
                    ))
                } else {
                    // Variable access: VARNAME::var_name or VARNAME::[expr]
                    use crate::structs::engine_registry::NodeMethodRef;
                    Ok(Expr::ApiCall(
                        CallModule::NodeMethod(NodeMethodRef::GetVar),
                        vec![left, var_name_expr],
                    ))
                }
            }
            PupToken::Star => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Mul,
                    Box::new(self.parse_expression(2)?),
                ))
            }
            PupToken::Slash => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Div,
                    Box::new(self.parse_expression(2)?),
                ))
            }
            PupToken::DotDot => {
                self.next_token();
                Ok(Expr::Range(
                    Box::new(left),
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Plus => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Add,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Minus => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Sub,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::LessThan => {
                // In infix context, < is a comparison operator (Lt)
                // Map literals are handled in parse_primary
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Lt,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::GreaterThan => {
                // In infix context, > is a comparison operator (Gt)
                // Map literals are handled in parse_primary
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Gt,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Le => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Le,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Ge => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Ge,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Eq => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Eq,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            PupToken::Ne => {
                self.next_token();
                Ok(Expr::BinaryOp(
                    Box::new(left),
                    Op::Ne,
                    Box::new(self.parse_expression(1)?),
                ))
            }
            _ => Ok(left),
        }
    }

    fn get_precedence(&self) -> u8 {
        match &self.current_token {
            PupToken::LBracket => 7,
            PupToken::LParen => 6,
            PupToken::Dot | PupToken::DoubleColon => 5,
            PupToken::As => 4,
            PupToken::Star | PupToken::Slash => 3,
            PupToken::Plus | PupToken::Minus => 2,
            PupToken::LessThan
            | PupToken::GreaterThan
            | PupToken::Le
            | PupToken::Ge
            | PupToken::Eq
            | PupToken::Ne => 1, // Comparison operators
            PupToken::DotDot => 1, // Range operator has low precedence
            _ => 0,
        }
    }

    fn map_type(&self, t: String) -> Type {
        match t.as_str() {
            "float" | "float_32" => Type::Number(NumberKind::Float(32)),
            "double" | "float_64" => Type::Number(NumberKind::Float(64)),
            "int_8" => Type::Number(NumberKind::Signed(8)),
            "int_16" => Type::Number(NumberKind::Signed(16)),
            "int" | "int_32" => Type::Number(NumberKind::Signed(32)),
            "int_64" => Type::Number(NumberKind::Signed(64)),
            "int_128" => Type::Number(NumberKind::Signed(128)),
            "uint_8" => Type::Number(NumberKind::Unsigned(8)),
            "uint_16" => Type::Number(NumberKind::Unsigned(16)),
            "uint" | "uint_32" => Type::Number(NumberKind::Unsigned(32)),
            "uint_64" => Type::Number(NumberKind::Unsigned(64)),
            "uint_128" => Type::Number(NumberKind::Unsigned(128)),
            "decimal" => Type::Number(NumberKind::Decimal),
            "big_int" | "big" | "bigint" => Type::Number(NumberKind::BigInt),
            "bool" => Type::Bool,
            "string" => Type::String,
            "signal" => Type::Signal,
            "Map" | "map" => Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
            "Array" | "array" => Type::Container(ContainerKind::Array, vec![Type::Object]),
            "Object" | "object" => Type::Object,
            _ => {
                // Check if it's an engine struct first
                use crate::structs::engine_structs::EngineStruct;
                if let Some(engine_struct) = EngineStruct::from_string(&t) {
                    Type::EngineStruct(engine_struct)
                } else {
                    // Check engine registry for node types
                    use crate::structs::engine_registry::ENGINE_REGISTRY;
                    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
                        format!("{:?}", nt) == t
                    }) {
                        Type::Node(node_type.clone())
                    } else {
                        Type::Custom(t)
                    }
                }
            }
        }
    }
}
