use std::collections::HashMap;

use crate::lang::api_modules::{ApiModule, NodeSugarApi};
use crate::lang::ast::*;
use crate::lang::pup::api::{PupAPI, PupNodeSugar, normalize_type_name};
use crate::lang::pup::lexer::{PupLexer, PupToken};

pub struct PupParser {
    lexer: PupLexer,
    current_token: PupToken,
    /// Variable name → inferred type (for local scope/type inference during parsing)
    type_env: HashMap<String, Type>,
    pub parsed_structs: Vec<StructDef>,
}

impl PupParser {
    pub fn new(input: &str) -> Self {
        let mut lex = PupLexer::new(input);
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

        while self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::At => {
                    self.next_token();
                    match &self.current_token {
                        PupToken::Expose => {
                            self.next_token();
                            let mut var = self.parse_variable_decl()?; // Parse `var name: type = value` part
                            var.is_exposed = true; // Mark this variable as exposed
                            var.is_public = true; // All top-level Pup variables are public
                            script_vars.push(var); // Add to the unified list
                        }
                        PupToken::Ident(name) => {
                            return Err(format!("Unknown directive @{}", name));
                        }
                        other => {
                            return Err(format!("Expected directive after '@', got {:?}", other));
                        }
                    }
                }
                PupToken::Struct => {
                    let def = self.parse_struct_def()?;
                    self.parsed_structs.push(def.clone());
                    structs.push(def);
                }
                PupToken::Var => {
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

        Ok(Script {
            node_type,
            variables: script_vars, // Pass the single, unified, and ordered list to the Script AST
            functions,
            structs,
            verbose: true,
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
        })
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        self.expect(PupToken::Fn)?;
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected function name".into());
        };
        self.next_token();

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

        let body = self.parse_block()?;
        let is_trait = name == "init" || name == "update";
        let locals = self.collect_locals(&body);

        Ok(Function {
            name,
            params,
            locals,
            body,
            is_trait_method: is_trait,
            uses_self: false,
            return_type: Type::Void,
        })
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

        let lhs = self.parse_expression(0)?;
        if let Some(op) = self.take_assign_op() {
            let rhs = self.parse_expression(0)?;
            return self.make_assign_stmt(lhs, op, rhs);
        }

        Ok(Stmt::Expr(TypedExpr {
            expr: lhs,
            inferred_type: None,
        }))
    }

    fn make_assign_stmt(&mut self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
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
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetVar), args) => {
                if args.len() == 2 {
                    let node = args[0].clone();
                    let field = args[1].clone();
                    Ok(Stmt::Expr(TypedExpr {
                        expr: Expr::ApiCall(
                            ApiModule::NodeSugar(NodeSugarApi::SetVar),
                            vec![node, field, typed_rhs.expr],
                        ),
                        inferred_type: None,
                    }))
                } else {
                    Err("Invalid NodeSugar get_var arg count".into())
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
        self.expect(PupToken::Var)?;
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
                    elements.push((self.parse_expression(0)?));
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

                        // Otherwise treat `new Something()` as an API call or method
                        if let Some(api) = PupAPI::resolve(&type_name, "new") {
                            return Ok(Expr::ApiCall(api, args));
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
                if let Expr::MemberAccess(obj, method) = &left {
                    if let Expr::Ident(mod_name) = &**obj {
                        if let Some(api) = PupAPI::resolve(mod_name, method) {
                            return Ok(Expr::ApiCall(api, args));
                        }
                    }

                    if let Some(api) = PupNodeSugar::resolve_method(method) {
                        let mut args_full = vec![*obj.clone()];
                        args_full.extend(args);
                        return Ok(Expr::ApiCall(api, args_full));
                    }
                    if let Expr::Ident(var_name) = &**obj {
                        if let Some(var_type) = self.type_env.get(var_name) {
                            let norm_type_name = normalize_type_name(var_type);
                            if !norm_type_name.is_empty() {
                                if let Some(api) = PupAPI::resolve(norm_type_name, method) {
                                    let mut call_args = vec![*obj.clone()];
                                    call_args.extend(args);
                                    return Ok(Expr::ApiCall(api, call_args));
                                }
                            }
                        }
                    }
                }
                Ok(Expr::Call(Box::new(left), args))
            }
            PupToken::Dot => {
                self.next_token();

                let field_name = match &self.current_token {
                    PupToken::Ident(n) => n.clone(),
                    PupToken::New => "new".to_string(), // ✅ allow `.new` keyword
                    PupToken::Struct => "struct".to_string(), // (optional future‑proof)
                    _ => {
                        return Err(format!(
                            "Expected field after '.', got {:?}",
                            self.current_token
                        ));
                    }
                };

                self.next_token();
                Ok(Expr::MemberAccess(Box::new(left), field_name))
            }
            PupToken::DoubleColon => {
                self.next_token();
                let f = if let PupToken::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected identifier after '::'".into());
                };
                self.next_token();
                if self.current_token == PupToken::Assign {
                    self.next_token();
                    let val = self.parse_expression(2)?;
                    Ok(Expr::ApiCall(
                        ApiModule::NodeSugar(NodeSugarApi::SetVar),
                        vec![left, Expr::Literal(Literal::String(f)), val],
                    ))
                } else {
                    Ok(Expr::ApiCall(
                        ApiModule::NodeSugar(NodeSugarApi::GetVar),
                        vec![left, Expr::Literal(Literal::String(f))],
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
            "script" => Type::Script,
            "Map" | "map" => Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
            "Array" | "array" => Type::Container(ContainerKind::Array, vec![Type::Object]),
            "Object" | "object" => Type::Object,
            _ => Type::Custom(t),
        }
    }
}
