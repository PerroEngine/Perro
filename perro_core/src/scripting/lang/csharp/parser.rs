use crate::lang::ast::*;
use crate::lang::csharp::lexer::{CsLexer, CsToken};
use crate::lang::csharp::api::CSharpAPI;

pub struct CsParser {
    lexer: CsLexer,
    current_token: CsToken,
}

impl CsParser {
    pub fn new(input: &str) -> Self {
        let mut lex = CsLexer::new(input);
        let cur = lex.next_token();
        Self {
            lexer: lex,
            current_token: cur,
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
        // Skip 'using', 'namespace', access modifiers
        while matches!(self.current_token, CsToken::Using | CsToken::Namespace) {
            while self.current_token != CsToken::Semicolon && self.current_token != CsToken::Eof {
                self.next_token();
            }
            self.next_token();
        }
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
        if self.current_token == CsToken::Colon {
            self.next_token();
            if let CsToken::Ident(base) = &self.current_token {
                node_type = base.clone();
                self.next_token();
            }
        }

        self.expect(CsToken::LBrace)?;

        let mut variables = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            while matches!(self.current_token, CsToken::AccessModifier(_)) {
                self.next_token();
            }

            match &self.current_token {
                CsToken::Class => {
                    structs.push(self.parse_class_def()?);
                }
                CsToken::Var => {
                    variables.push(self.parse_variable_decl(None)?);
                }
                CsToken::Type(t) => {
                    let type_name = t.clone();
                    self.next_token();
                    let name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected identifier after type".into());
                    };
                    self.next_token();
                    if self.current_token == CsToken::LParen {
                        functions.push(self.parse_function(type_name, name)?);
                    } else {
                        variables.push(self.parse_variable_decl(Some(type_name))?);
                    }
                }
                CsToken::Void => {
                    let type_name = "void".to_string();
                    self.next_token();
                    let name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected function name after 'void'".into());
                    };
                    self.next_token();
                    self.expect(CsToken::LParen)?;
                    functions.push(self.parse_function(type_name, name)?);
                }
                _ => {
                    self.next_token();
                }
            }
        }

        self.expect(CsToken::RBrace)?;

        Ok(Script {
            node_type,
            exposed: vec![],
            variables,
            functions,
            structs,
            verbose: true,
        })
    }

    fn parse_class_def(&mut self) -> Result<StructDef, String> {
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
            while matches!(self.current_token, CsToken::AccessModifier(_)) {
                self.next_token();
            }
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
                    if self.current_token == CsToken::LParen {
                        methods.push(self.parse_function(typ_str, field_name)?);
                    } else {
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
        self.next_token();
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
        Ok(Variable {
            name,
            typ: explicit_type.map(|t| self.map_type(t)),
            value,
        })
    }

    fn parse_function(&mut self, return_type: String, name: String) -> Result<Function, String> {
        let mut params = Vec::new();

        if self.current_token != CsToken::RParen {
            self.next_token();
            while self.current_token != CsToken::RParen && self.current_token != CsToken::Eof {
                self.next_token();
            }
        }

        self.expect(CsToken::RParen)?;
        self.expect(CsToken::LBrace)?;
        let body = self.parse_block()?;

        let is_trait = name.to_lowercase() == "init" || name.to_lowercase() == "update";
        let locals = self.collect_locals(&body);

        Ok(Function {
            name,
            params,
            locals,
            body,
            is_trait_method: is_trait,
            uses_self: false,
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
                    // Anonymous object: new { foo = 1, ... }
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
                            self.expect(CsToken::Assign)?; // '=' in C#
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
                    // new T[] { ... } OR new T[expr]
                    CsToken::Ident(ty) if self.lexer.peek() == Some('[') => {
                        let element_type = ty.clone();
                        self.next_token();
                        self.expect(CsToken::LBracket)?;
                        if self.current_token == CsToken::RBracket {
                            self.next_token();
                            if self.current_token == CsToken::LBrace {
                                self.next_token();
                                let mut elems = Vec::new();
                                while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
                                    let val = self.parse_expression(0)?;
                                    elems.push(val);
                                    if self.current_token == CsToken::Comma {
                                        self.next_token();
                                    } else {
                                        break;
                                    }
                                }
                                self.expect(CsToken::RBrace)?;
                                let len = elems.len();
                                Ok(Expr::ContainerLiteral(ContainerKind::Array, ContainerLiteralData::Array(elems)))
                            } else {
                                Err("Expected array initializer for new T[]".into())
                            }
                        } else {
                            // new T[expr]
                            if let CsToken::Number(n) = &self.current_token {
                                let size = n.parse::<usize>().unwrap_or(0);
                                self.next_token();
                                self.expect(CsToken::RBracket)?;
                                Ok(Expr::ContainerLiteral(
                                    ContainerKind::FixedArray(size),
                                    ContainerLiteralData::FixedArray(size, vec![])
                                ))
                            } else {
                                Err("Expected array size or initializer".into())
                            }
                        }
                    }
                    // new Dictionary<K,V>() / List<T>()
                    CsToken::Ident(name) => {
                        let type_name = name.clone();
                        self.next_token();
                        // Parse generic angle brackets <...>
                        if self.current_token == CsToken::Lt {
                            while self.current_token != CsToken::Gt && self.current_token != CsToken::Eof {
                                self.next_token();
                            }
                            self.expect(CsToken::Gt)?;
                        }
                        // Support new Dictionary<,>() and new List<>()
                        if self.current_token == CsToken::LParen {
                            self.next_token();
                            while self.current_token != CsToken::RParen && self.current_token != CsToken::Eof {
                                self.next_token();
                            }
                            self.expect(CsToken::RParen)?;
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
                        }
                        Ok(Expr::Call(Box::new(Expr::Ident(type_name)), vec![]))
                    }
                    _ => Err("Unexpected token after 'new'".into())
                }
            }
            CsToken::Ident(n) => {
                let name = n.clone();
                self.next_token();
                if self.current_token == CsToken::LParen {
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
                } else {
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
            CsToken::LParen => {
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
            CsToken::Dot => {
                self.next_token();
                let field = if let CsToken::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected member name after '.'".into());
                };
                self.next_token();
                Ok(Expr::MemberAccess(Box::new(left), field))
            }
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
            CsToken::LParen => 5,
            CsToken::Dot => 4,
            CsToken::Star | CsToken::Slash => 3,
            CsToken::Plus | CsToken::Minus => 2,
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
            "char" => Type::Number(NumberKind::Unsigned(16)),
            "string" => Type::String,
            "object" => Type::Custom("object".to_string()),
            ty if ty.starts_with("Dictionary") =>
                        // By default, fallback to dynamic (String->Object), but you may want to parse generic args
                        Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                    ty if ty.starts_with("List") =>
                        Type::Container(ContainerKind::Array, vec![Type::Object]), // Optionally extract T
                    ty if ty.ends_with("[]") => {
                        Type::Container(ContainerKind::FixedArray(0), vec![Type::Object])
                    }
                    _ => Type::Custom(t)
                }
            }
}