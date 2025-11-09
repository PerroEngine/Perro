use std::collections::HashMap;

use crate::lang::ast::*;
use crate::lang::api_modules::{ApiModule, NodeSugarApi};
use crate::lang::csharp::lexer::{CsLexer, CsToken};
use crate::lang::csharp::api::CSharpAPI;

pub struct CsParser {
    lexer: CsLexer,
    current_token: CsToken,
    /// Variable name → inferred type (for local scope/type inference during parsing)
    type_env: HashMap<String, Type>, // Kept for local variable type inference logic if implemented
    pub parsed_structs: Vec<StructDef>,
}

impl CsParser {
    pub fn new(input: &str) -> Self {
        let mut lex = CsLexer::new(input);
        let cur = lex.next_token();
        Self {
            lexer: lex,
            current_token: cur,
            type_env: HashMap::new(),
            parsed_structs: Vec::new(),
        }
    }

    fn next_token(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect(&mut self, tok: CsToken) -> Result<(), String> {
        if self.current_token == tok {
            self.next_token();
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", tok, self.current_token))
        }
    }

    pub fn parse_script(&mut self) -> Result<Script, String> {
        // Skip 'using', 'namespace'
        while matches!(self.current_token, CsToken::Using | CsToken::Namespace) {
            while self.current_token != CsToken::Semicolon && self.current_token != CsToken::Eof {
                self.next_token();
            }
            self.next_token();
        }
        
        // Consume any initial access modifiers for the class itself
        // (we ignore them for the script's overall node_type, but good to consume)
        while matches!(self.current_token, CsToken::AccessModifier(_)) {
            self.next_token();
        }

        self.expect(CsToken::Class)?;
        let class_name = if let CsToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected class name".into());
        };
        self.next_token();

        let mut node_type = String::new();
        if self.current_token == CsToken::Colon { // Base class
            self.next_token();
            if let CsToken::Ident(base) = &self.current_token {
                node_type = base.clone();
                self.next_token();
            }
        }

        self.expect(CsToken::LBrace)?;

        let mut script_vars = Vec::new(); // Unified list for all script-level variables
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            let mut is_public = false;
            let mut is_exposed = false;

            // Step 1: Parse Attributes (e.g., [Expose])
            if self.current_token == CsToken::LBracket {
                self.next_token(); // consume '['
                if let CsToken::Ident(attr_name) = &self.current_token {
                    if attr_name == "Expose" { // Check for [Expose] attribute
                        is_exposed = true;
                    }
                }
                // Consume the rest of the attribute declaration
                while self.current_token != CsToken::RBracket && self.current_token != CsToken::Eof {
                    self.next_token();
                }
                self.expect(CsToken::RBracket)?; // consume ']'
            }

            // Step 2: Parse Access Modifiers (e.g., public, private)
            if let CsToken::AccessModifier(modifier) = &self.current_token {
                if modifier == "public" {
                    is_public = true;
                }
                self.next_token(); // consume access modifier
            }


            match &self.current_token {
                CsToken::Class => {
                    // Nested classes/structs don't use is_public/is_exposed flags for their definition
                    structs.push(self.parse_class_def()?);
                }
                CsToken::Var | CsToken::Type(_) | CsToken::Void => {
                    // This block handles both explicit type declarations (e.g., `float myField;`)
                    // and `var` keyword (which is for local inference, but here assuming a class field)
                    // and `void` (for methods).

                    let mut return_or_type_name = None;
                    let starting_token = self.current_token.clone(); // Capture type or var/void token

                    if starting_token == CsToken::Var {
                        self.next_token(); // Consume 'var'
                        // For class fields, `var` without explicit type is ambiguous; treat as `object` or assume type inference
                        // For this transpiler, if no explicit type is provided later, it might default to `Object`.
                    } else if let CsToken::Type(t) = starting_token {
                        return_or_type_name = Some(t.clone());
                        self.next_token(); // Consume explicit type
                    } else if starting_token == CsToken::Void {
                        return_or_type_name = Some("void".to_string());
                        self.next_token(); // Consume 'void'
                    } else {
                        return Err(format!("Expected type, 'var', or 'void', got {:?}", starting_token));
                    }
                    
                    let name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected identifier after type or 'var'".into());
                    };
                    self.next_token(); // Consume identifier (variable name or function name)

                    if self.current_token == CsToken::LParen { // It's a method
                        functions.push(self.parse_function(return_or_type_name.unwrap_or_default(), name, is_public)?);
                    } else { // It's a field/variable declaration
                        let mut var_decl = self.parse_variable_decl(return_or_type_name)?;
                        var_decl.name = name; // Ensure correct name is set
                        var_decl.is_public = is_public;
                        var_decl.is_exposed = is_exposed;
                        script_vars.push(var_decl);
                    }
                }
                _ => {
                    self.next_token(); // Consume unexpected token
                }
            }
        }

        self.expect(CsToken::RBrace)?;

        Ok(Script {
            node_type,
            variables: script_vars, // Pass the single, unified, and ordered list
            functions,
            structs,
            verbose: true,
        })
    }

    fn parse_class_def(&mut self) -> Result<StructDef, String> {
        // Access modifiers for nested classes are handled by the outer loop if present.
        self.expect(CsToken::Class)?;
        let name = if let CsToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected class name".into());
        };
        self.next_token();

        let mut base = None;
        if self.current_token == CsToken::Colon {
            self.next_token();
            if let CsToken::Ident(base_name) = &self.current_token {
                base = Some(base_name.clone());
                self.next_token();
            }
        }

        self.expect(CsToken::LBrace)?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            let mut is_public_field = false; // Internal to the struct, usually not 'exposed' in Perro sense
            if let CsToken::AccessModifier(modifier) = &self.current_token {
                if modifier == "public" {
                    is_public_field = true;
                }
                self.next_token();
            }
            // No [Expose] for nested class fields in this simplified parser

            match &self.current_token {
                CsToken::Void | CsToken::Type(_) => {
                    let typ_str = match &self.current_token {
                        CsToken::Void => "void".to_string(),
                        CsToken::Type(t) => t.clone(),
                        _ => unreachable!(),
                    };
                    self.next_token();
                    let field_name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected field/method name after type".into());
                    };
                    self.next_token();
                    if self.current_token == CsToken::LParen { // It's a method
                        methods.push(self.parse_function(typ_str, field_name, is_public_field)?);
                    } else { // It's a field
                        // StructField doesn't track is_public/is_exposed, assuming inner fields are all public/private based on context.
                        // If it needs to, StructField would also need those flags.
                        fields.push(StructField { name: field_name, typ: self.map_type(typ_str) });
                        if self.current_token == CsToken::Semicolon {
                            self.next_token();
                        }
                    }
                }
                _ => { self.next_token(); }
            }
        }

        self.expect(CsToken::RBrace)?;
        Ok(StructDef { name, fields, methods, base })
    }

    fn parse_variable_decl(&mut self, explicit_type: Option<String>) -> Result<Variable, String> {
        // If explicit_type is None here, it means `var` was the keyword (`CsToken::Var` was already consumed by caller).
        // If explicit_type is Some, `CsToken::Type` was already consumed by caller.
        
        // At this point, we've already parsed attributes and access modifiers in the caller.
        // This function just parses the `name = value;` part.

        let name = if let CsToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected variable name".into());
        };
        self.next_token();

        let mut value = None;
        if self.current_token == CsToken::Assign {
            self.next_token();
            let expr = self.parse_expression(0)?;
            value = Some(TypedExpr { expr, inferred_type: None });
        }
        if self.current_token == CsToken::Semicolon {
            self.next_token();
        }

        // Initialize with defaults; is_public and is_exposed are set by the caller (parse_script or parse_class_def)
        Ok(Variable {
            name,
            typ: explicit_type.map(|t| self.map_type(t)),
            value,
            is_exposed: false, // Set by caller
            is_public: false,  // Set by caller
        })
    }

    fn parse_function(&mut self, return_type: String, name: String, is_public_func: bool) -> Result<Function, String> {
        // Parameters are consumed in the initial loop that checks `self.current_token != CsToken::RParen`.
        // For a more complete parser, you'd parse each parameter type and name here.
        let mut params = Vec::new(); 

        if self.current_token != CsToken::RParen {
            // Consume parameters for now without parsing them deeply
            self.next_token(); // consume first parameter's type/name or just advance past stuff
            while self.current_token != CsToken::RParen && self.current_token != CsToken::Eof {
                self.next_token();
            }
        }

        self.expect(CsToken::RParen)?;
        self.expect(CsToken::LBrace)?;
        let body = self.parse_block()?;

        let is_trait = name.to_lowercase() == "init" || name.to_lowercase() == "update"; // Perro specific
        let locals = self.collect_locals(&body);

        Ok(Function {
            name,
            params, // Currently empty as not parsing params deeply
            locals,
            body,
            is_trait_method: is_trait,
            uses_self: false, // Needs to be determined by an AST walk
            return_type: self.map_type(return_type),
        })
    }

    fn collect_locals(&self, body: &[Stmt]) -> Vec<Variable> {
        body.iter().filter_map(|stmt| match stmt {
            Stmt::VariableDecl(v) => Some(v.clone()),
            _ => None,
        }).collect()
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut out = Vec::new();
        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            out.push(self.parse_statement()?);
        }
        self.expect(CsToken::RBrace)?;
        Ok(out)
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        // Consume access modifiers for local variables inside blocks (e.g., `public` is invalid here, but if present in source)
        while matches!(self.current_token, CsToken::AccessModifier(_)) {
            self.next_token();
        }
        // Consume attributes for local variables inside blocks (e.g., `[Expose]` is invalid here, but if present in source)
        while self.current_token == CsToken::LBracket {
            self.next_token();
            while self.current_token != CsToken::RBracket && self.current_token != CsToken::Eof {
                self.next_token();
            }
            self.expect(CsToken::RBracket)?;
        }


        // C# `var` keyword (for local variable type inference)
        if self.current_token == CsToken::Var {
            let mut var_decl = self.parse_variable_decl(None)?;
            // Local variables in C# are not exposed or public in the same way class members are.
            var_decl.is_public = false; // Local C# `var` is not a public member
            var_decl.is_exposed = false; // Local C# `var` is not exposed
            let stmt = Stmt::VariableDecl(var_decl);
            if self.current_token == CsToken::Semicolon {
                self.next_token();
            }
            return Ok(stmt);
        }

        // Handle explicit type declarations for local variables
        if matches!(&self.current_token, CsToken::Type(_)) {
            let type_str = match &self.current_token {
                CsToken::Type(t) => t.clone(),
                _ => unreachable!(),
            };
            self.next_token(); // Consume type
            let mut var_decl = self.parse_variable_decl(Some(type_str))?;
            // Local variables in C# are not exposed or public.
            var_decl.is_public = false;
            var_decl.is_exposed = false;
            let stmt = Stmt::VariableDecl(var_decl);
            if self.current_token == CsToken::Semicolon {
                self.next_token();
            }
            return Ok(stmt);
        }


        let left = self.parse_expression(0)?;
        if let Some(op) = self.take_assign_op() {
            let right = self.parse_expression(0)?;
            let stmt = self.make_assign_stmt(left, op, right)?;
            if self.current_token == CsToken::Semicolon {
                self.next_token();
            }
            return Ok(stmt);
        }
        if self.current_token == CsToken::Semicolon {
            self.next_token();
        }
        Ok(Stmt::Expr(TypedExpr { expr: left, inferred_type: None }))
    }

    fn take_assign_op(&mut self) -> Option<Option<Op>> {
        let op = match self.current_token {
            CsToken::Assign => Some(None),
            CsToken::PlusEq => Some(Some(Op::Add)),
            CsToken::MinusEq => Some(Some(Op::Sub)),
            CsToken::MulEq => Some(Some(Op::Mul)),
            CsToken::DivEq => Some(Some(Op::Div)),
            _ => None,
        };
        if op.is_some() {
            self.next_token();
        }
        op
    }

    fn make_assign_stmt(&mut self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
        let typed_rhs = TypedExpr { expr: rhs, inferred_type: None };
        match lhs {
            Expr::Ident(name) => Ok(match op {
                None => Stmt::Assign(name, typed_rhs),
                Some(op) => Stmt::AssignOp(name, op, typed_rhs),
            }),
            Expr::MemberAccess(obj, field) => {
                let typed_lhs = TypedExpr { expr: Expr::MemberAccess(obj, field), inferred_type: None };
                Ok(match op {
                    None => Stmt::MemberAssign(typed_lhs, typed_rhs),
                    Some(op) => Stmt::MemberAssignOp(typed_lhs, op, typed_rhs),
                })
            }
            Expr::Index(obj, key) => Ok(match op {
                None => Stmt::IndexAssign(obj, key, typed_rhs),
                Some(bop) => Stmt::IndexAssignOp(obj, key, bop, typed_rhs),
            }),
            _ => Err("Invalid LHS for assignment".into()),
        }
    }

    fn parse_expression(&mut self, precedence: u8) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while precedence < self.get_precedence() {
            left = self.parse_infix(left)?;
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match &self.current_token {
            CsToken::This => {
                self.next_token();
                Ok(Expr::SelfAccess)
            }
            CsToken::Base => {
                self.next_token();
                Ok(Expr::BaseAccess)
            }
            CsToken::New => {
                self.next_token();
                match &self.current_token {
                    // Anonymous object: new { foo = 1, ... } (C# specific)
                    CsToken::LBrace => {
                        self.next_token();
                        let mut pairs = Vec::new();
                        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
                            let key = match &self.current_token {
                                CsToken::Ident(k) => k.clone(),
                                CsToken::String(k) => k.clone(),
                                other => return Err(format!("Expected property name, got {:?}", other)),
                            };
                            self.next_token();
                            self.expect(CsToken::Assign)?; // C# uses '=' for property initialization
                            let expr = self.parse_expression(0)?;
                            pairs.push((Some(key), expr));
                            if self.current_token == CsToken::Comma {
                                self.next_token();
                            } else {
                                break;
                            }
                        }
                        self.expect(CsToken::RBrace)?;
                        Ok(Expr::ObjectLiteral(pairs))
                    }
                    // new T[] { ... } OR new T[expr] (C# specific array initialization)
                    CsToken::Ident(ty_ident_val) => { 
                        let next_token_peek = self.lexer.peek_token(); // This is the crucial line!

                        if next_token_peek == CsToken::LBracket {
                            // It IS `new Type[`...
                            let element_type_str = ty_ident_val.clone(); // Capture the type (e.g., "int")
                            self.next_token(); // Consume CsToken::Ident (e.g., `int`)
                            self.expect(CsToken::LBracket)?; // Consume `[`

                            let mut initializers = Vec::new();
                            let mut array_size: Option<usize> = None;

                            // Check if it's `new Type[size]` or `new Type[] { ... }`
                            if self.current_token != CsToken::RBracket { // If not `[]` immediately (empty declaration)
                                let size_expr_token = self.current_token.clone();
                                if let CsToken::Number(n_str) = size_expr_token {
                                    array_size = n_str.parse::<usize>().ok();
                                    self.next_token(); // consume number
                                } else {
                                    // If not a number, it's an expression for size.
                                    self.next_token(); 
                                    array_size = None; // Dynamic size (Vec<T>)
                                }
                            }
                            self.expect(CsToken::RBracket)?; // consume `]`

                            // After `new Type[]` or `new Type[size]`, check for `{ ... }` initializer
                            if self.current_token == CsToken::LBrace {
                                self.next_token(); // consume `{`
                                while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
                                    let val = self.parse_expression(0)?;
                                    initializers.push(val);
                                    if self.current_token == CsToken::Comma {
                                        self.next_token();
                                    } else {
                                        break;
                                    }
                                }
                                self.expect(CsToken::RBrace)?; // consume `}`

                                let len_from_initializer = initializers.len();
                                Ok(Expr::ContainerLiteral(
                                    ContainerKind::FixedArray(len_from_initializer), // Size is count of initializers
                                    ContainerLiteralData::FixedArray(len_from_initializer, initializers)
                                ))
                            } else {
                                if let Some(size) = array_size {
                                    Ok(Expr::ContainerLiteral(
                                        ContainerKind::FixedArray(size),
                                        ContainerLiteralData::FixedArray(size, initializers) // initializers will be empty
                                    ))
                                } else {
                                    Ok(Expr::ContainerLiteral(
                                        ContainerKind::Array,
                                        ContainerLiteralData::Array(initializers) // initializers will be empty
                                    ))
                                }
                            }
                        } else { // It's not `new Type[` (i.e., `new Dictionary<K,V>()`, `new MyCustomClass()`)
                            // The `ty_ident_val` (e.g. `Dictionary`) has NOT been consumed by `self.next_token()` in this branch.
                            // So, we need to consume it here *first*.
                            
                            let type_name = ty_ident_val.clone(); // The identifier string
                            self.next_token(); // Consume the identifier (e.g. `Dictionary`)

                            // Parse generic angle brackets <...>
                            if self.current_token == CsToken::Lt {
                                while self.current_token != CsToken::Gt && self.current_token != CsToken::Eof {
                                    self.next_token();
                                }
                                self.expect(CsToken::Gt)?;
                            }
                            // Consume `()` for constructor call
                            if self.current_token == CsToken::LParen {
                                self.next_token();
                                while self.current_token != CsToken::RParen && self.current_token != CsToken::Eof {
                                    self.next_token();
                                }
                                self.expect(CsToken::RParen)?;
                            }

                            // Determine the type of container being constructed
                            if type_name.starts_with("Dictionary") {
                                return Ok(Expr::ContainerLiteral(
                                    ContainerKind::Map,
                                    ContainerLiteralData::Map(vec![])
                                ));
                            } else if type_name.starts_with("List") {
                                return Ok(Expr::ContainerLiteral(
                                    ContainerKind::Array,
                                    ContainerLiteralData::Array(vec![])
                                ));
                            }
                            // Default to a generic struct constructor call
                            Ok(Expr::StructNew(type_name, vec![]))
                        }
                    }
                    _ => Err("Unexpected token after 'new'".into())
                }
            }
            CsToken::Ident(n) => {
                let name = n.clone();
                self.next_token();
                if self.current_token == CsToken::LParen { // Function/Method call
                    self.next_token();
                    let mut args = Vec::new();
                    if self.current_token != CsToken::RParen {
                        args.push(self.parse_expression(0)?);
                        while self.current_token == CsToken::Comma {
                            self.next_token();
                            args.push(self.parse_expression(0)?);
                        }
                    }
                    self.expect(CsToken::RParen)?;
                    Ok(Expr::Call(Box::new(Expr::Ident(name)), args))
                } else { // Just an identifier (variable reference)
                    Ok(Expr::Ident(name))
                }
            }
            CsToken::Number(n) => {
                let raw = n.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::Number(raw.to_string())))
            }
            CsToken::String(s) => {
                let val = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::String(val)))
            }
            // Add C# boolean literals
            CsToken::True => {
                self.next_token();
                Ok(Expr::Literal(Literal::Bool(true)))
            }
            CsToken::False => {
                self.next_token();
                Ok(Expr::Literal(Literal::Bool(false)))
            }
            CsToken::LParen => {
                self.next_token();
                let expr = self.parse_expression(0)?;
                self.expect(CsToken::RParen)?;
                Ok(expr)
            }
            _ => Err(format!("Unexpected primary {:?}", self.current_token)),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, String> {
        match &self.current_token {
            CsToken::LBracket => { // Array/List indexing
                self.next_token();
                let index = self.parse_expression(0)?;
                self.expect(CsToken::RBracket)?;
                Ok(Expr::Index(Box::new(left), Box::new(index)))
            }
            CsToken::LParen => { // Method call on an expression (e.g. `obj.Method()`)
                self.next_token();
                let mut args = Vec::new();
                if self.current_token != CsToken::RParen {
                    args.push(self.parse_expression(0)?);
                    while self.current_token == CsToken::Comma {
                        self.next_token();
                        args.push(self.parse_expression(0)?);
                    }
                }
                self.expect(CsToken::RParen)?;
                if let Expr::MemberAccess(obj, method) = &left {
                    if let Expr::Ident(module) = &**obj {
                        if let Some(api_sem) = CSharpAPI::resolve(module, method) {
                            return Ok(Expr::ApiCall(api_sem, args));
                        }
                    }
                }
                Ok(Expr::Call(Box::new(left), args))
            }
            CsToken::Dot => { // Member access (e.g., `obj.field`)
                self.next_token();
                let field = if let CsToken::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected member name after '.'".into());
                };
                self.next_token();
                Ok(Expr::MemberAccess(Box::new(left), field))
            }
            // Binary Operators
            CsToken::Star => {
                self.next_token();
                let right = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Mul, Box::new(right)))
            }
            CsToken::Slash => {
                self.next_token();
                let right = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Div, Box::new(right)))
            }
            CsToken::Plus => {
                self.next_token();
                let right = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Add, Box::new(right)))
            }
            CsToken::Minus => {
                self.next_token();
                let right = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Sub, Box::new(right)))
            }
            _ => Ok(left)
        }
    }

    fn get_precedence(&self) -> u8 {
        match &self.current_token {
            CsToken::LBracket => 6, // Indexing has higher precedence than calls or member access
            CsToken::LParen => 5,   // Method calls
            CsToken::Dot => 4,      // Member access
            CsToken::Star | CsToken::Slash => 3, // Multiplication, Division
            CsToken::Plus | CsToken::Minus => 2, // Addition, Subtraction
            _ => 0,
        }
    }

    fn map_type(&self, t: String) -> Type {
        match t.as_str() {
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
            "char" => Type::Number(NumberKind::Unsigned(16)), // C# char is UTF-16 code unit
            "string" => Type::String,
            "object" => Type::Object, // C# `object` maps to `serde_json::Value` or similar dynamic object in Perro
            ty if ty.starts_with("Dictionary") =>
                        // Default to dynamic (String->Object) if generics aren't deeply parsed
                        Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                    ty if ty.starts_with("List") =>
                        // Default to dynamic (Object) if generics aren't deeply parsed
                        Type::Container(ContainerKind::Array, vec![Type::Object]), 
                    ty if ty.ends_with("[]") => {
                        // For `int[]`, `string[]` etc. We need to extract the base type.
                        // Simplified for now, assuming element type is `Object` if not inferred.
                        let base_type_str = ty.trim_end_matches("[]").to_string();
                        let element_type = self.map_type(base_type_str); // Recursively map base type
                        Type::Container(ContainerKind::Array, vec![element_type]) // Map C# `T[]` to Rust `Vec<T>`
                    }
                    _ => Type::Custom(t) // Default to custom type if not recognized
                }
            }
}