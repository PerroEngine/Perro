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

    // ------------------------------------------------------
    //  Parse C# class:   public class Player : Node2D { ... }
    // ------------------------------------------------------
    pub fn parse_script(&mut self) -> Result<Script, String> {
        // Skip modifiers/usings etc.
        while matches!(self.current_token, CsToken::Using | CsToken::Namespace) {
            while self.current_token != CsToken::Semicolon && self.current_token != CsToken::Eof {
                self.next_token();
            }
            self.next_token();
        }
        while matches!(self.current_token, CsToken::AccessModifier(_)) {
            self.next_token();
        }

        // --- Parse the top-level script wrapper ---
        self.expect(CsToken::Class)?;
        if let CsToken::Ident(_) = &self.current_token {
            self.next_token(); // skip class name
        }

        // Parse base type for our Script.node_type
        let mut node_type = String::new();
        if self.current_token == CsToken::Colon {
            self.next_token();
            if let CsToken::Ident(base) = &self.current_token {
                node_type = base.clone();
                self.next_token();
            } else {
                return Err("Expected base identifier after ':'".into());
            }
        }

        // Class body
        self.expect(CsToken::LBrace)?;

        let mut variables = Vec::new();
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            while matches!(self.current_token, CsToken::AccessModifier(_)) {
                self.next_token();
            }

            match &self.current_token {
                // ⚙️ detect inner class definitions
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
                    let return_type = "void".to_string();
                    self.next_token();
                    let name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected function name after 'void'".into());
                    };
                    self.next_token();
                    self.expect(CsToken::LParen)?;
                    functions.push(self.parse_function(return_type, name)?);
                }

                _ => {
                    self.next_token(); // skip stray tokens
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

            verbose: true
        })
    }

    fn parse_class_def(&mut self) -> Result<StructDef, String> {
        self.expect(CsToken::Class)?;
        let mut base: Option<String> = None;

        // Parse class name
        let name = if let CsToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected class name".into());
        };
        self.next_token();

        // Optional inheritance: class Foo : Bar
        if self.current_token == CsToken::Colon {
            self.next_token();
            if let CsToken::Ident(base_name) = &self.current_token {
                base = Some(base_name.clone());
                self.next_token();
            } else {
                return Err("Expected base name after ':'".into());
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
                        CsToken::Void => "void".into(),
                        CsToken::Type(t) => t.clone(),
                        _ => unreachable!(),
                    };
                    self.next_token();

                    let name = if let CsToken::Ident(n) = &self.current_token {
                        n.clone()
                    } else {
                        return Err("Expected name after type".into());
                    };
                    self.next_token();

                    if self.current_token == CsToken::LParen {
                        methods.push(self.parse_function(typ_str, name)?);
                    } else {
                        fields.push(StructField {
                            name,
                            typ: self.map_type(typ_str),
                        });
                        if self.current_token == CsToken::Semicolon {
                            self.next_token();
                        }
                    }
                }

                _ => {
                    self.next_token();
                }
            }
        }

        self.expect(CsToken::RBrace)?;
        Ok(StructDef { name, fields, methods, base })
    }

    // ------------------------------------------------------
    //  Variable declarations
    // ------------------------------------------------------
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
            value = Some(TypedExpr { 
                expr, 
                inferred_type: None 
            });
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

    // ------------------------------------------------------
    //  Function definitions: void Init() { ... }
    // ------------------------------------------------------
    fn parse_function(&mut self, return_type: String, name: String) -> Result<Function, String> {
        let mut params = Vec::new();

        // parameters
        if self.current_token != CsToken::RParen {
            self.next_token();
            while self.current_token != CsToken::RParen && self.current_token != CsToken::Eof {
                self.next_token(); // simplified: ignore param details for now
            }
        }

        self.expect(CsToken::RParen)?;
        self.expect(CsToken::LBrace)?;
        let body = self.parse_block()?;

        let trait_name = name.to_lowercase();
        let is_trait = trait_name == "init" || trait_name == "update";

        let locals = self.collect_locals(&body);

        Ok(Function {
            name: trait_name,
            params,
            locals,
            body,
            is_trait_method: is_trait,
            uses_self: false,
            return_type: self.map_type(return_type),
        })
    }

    fn collect_locals(&self, body: &[Stmt]) -> Vec<Variable> {
        let mut out = Vec::new();
        for stmt in body {
            if let Stmt::VariableDecl(v) = stmt {
                out.push(v.clone());
            }
        }
        out
    }

    // ------------------------------------------------------
    //  Block { ... }
    // ------------------------------------------------------
    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        let mut stmts = Vec::new();
        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            stmts.push(self.parse_statement()?);
        }
        self.expect(CsToken::RBrace)?;
        Ok(stmts)
    }

    // ------------------------------------------------------
    //  Statement-level parsing
    // ------------------------------------------------------
    fn parse_statement(&mut self) -> Result<Stmt, String> {
        // parse potential variable, assignment, or expression
        let left = self.parse_expression(0)?;

        // handle assignment operators (=, +=, etc.)
        if let Some(op) = self.take_assign_op() {
            let right = self.parse_expression(0)?;
            let stmt = self.make_assign_stmt(left, op, right)?;
            // gobble the trailing semicolon if present
            if self.current_token == CsToken::Semicolon {
                self.next_token();
            }
            return Ok(stmt);
        }

        // plain expression statement, optional semicolon
        if self.current_token == CsToken::Semicolon {
            self.next_token();
        }

        // Wrap in TypedExpr
        Ok(Stmt::Expr(TypedExpr { 
            expr: left, 
            inferred_type: None 
        }))
    }

    // ------------------------------------------------------
    //  Assignment operator helpers - FIXED to use Op directly
    // ------------------------------------------------------
    fn take_assign_op(&mut self) -> Option<Option<Op>> {
        let op = match self.current_token {
            CsToken::Assign  => Some(None),           // Regular assignment
            CsToken::PlusEq  => Some(Some(Op::Add)),  // Op assignment
            CsToken::MinusEq => Some(Some(Op::Sub)),
            CsToken::MulEq   => Some(Some(Op::Mul)),
            CsToken::DivEq   => Some(Some(Op::Div)),
            _ => None,
        };
        if op.is_some() {
            self.next_token();
        }
        op
    }

    fn make_assign_stmt(&mut self, lhs: Expr, op: Option<Op>, rhs: Expr) -> Result<Stmt, String> {
        let typed_rhs = TypedExpr { 
            expr: rhs, 
            inferred_type: None 
        };

        match lhs {
            Expr::Ident(name) => Ok(match op {
                None => Stmt::Assign(name, typed_rhs),
                Some(op) => Stmt::AssignOp(name, op, typed_rhs),
            }),

            Expr::MemberAccess(obj, field) => {
                let typed_lhs = TypedExpr { 
                    expr: Expr::MemberAccess(obj, field), 
                    inferred_type: None 
                };
                Ok(match op {
                    None => Stmt::MemberAssign(typed_lhs, typed_rhs),
                    Some(op) => Stmt::MemberAssignOp(typed_lhs, op, typed_rhs),
                })
            }

            _ => Err("Invalid LHS for assignment".into()),
        }
    }

    // ------------------------------------------------------
    //  Core expression grammar
    // ------------------------------------------------------
    fn parse_expression(&mut self, precedence: u8) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while precedence < self.get_precedence() {
            left = self.parse_infix(left)?;
        }
        Ok(left)
    }

    // ------------------------------------------------------
    //  Primary expressions: identifiers, literals, this
    // ------------------------------------------------------
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
                self.next_token(); // consume 'new'

                // Anonymous object literal: new { foo = "bar", baz = 69 }
                if self.current_token == CsToken::LBrace {
                    self.next_token();
                    let mut pairs = Vec::new();

                    while self.current_token != CsToken::RBrace
                        && self.current_token != CsToken::Eof
                    {
                        // parse property name/identifier
                        let key = match &self.current_token {
                            CsToken::Ident(k) => k.clone(),
                            CsToken::String(k) => k.clone(),
                            other => {
                                return Err(format!(
                                    "Expected property name in anonymous object, got {:?}",
                                    other
                                ))
                            }
                        };
                        self.next_token();

                        self.expect(CsToken::Assign)?; // '=' instead of ':'

                        let value = self.parse_expression(0)?;
                        pairs.push((key, value));

                        if self.current_token == CsToken::Comma {
                            self.next_token();
                        } else {
                            break;
                        }
                    }

                    self.expect(CsToken::RBrace)?;
                    Ok(Expr::ObjectLiteral(pairs))
                } else {
                    // Constructor call case: new SomeType(...)
                    let type_name = if let CsToken::Ident(t) = &self.current_token {
                        t.clone()
                    } else {
                        return Err(format!(
                            "Expected type name or '{{' after 'new', got {:?}",
                            self.current_token
                        ));
                    };
                    self.next_token();

                    // Handle parentheses (args): new MyType(1, 2)
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
                        Ok(Expr::Call(Box::new(Expr::Ident(type_name)), args))
                    }
                    // Handle object init syntax: new MyType { x = 1, y = 2 }
                    else if self.current_token == CsToken::LBrace {
                        self.next_token();
                        let mut pairs = Vec::new();

                        while self.current_token != CsToken::RBrace
                            && self.current_token != CsToken::Eof
                        {
                            let key = match &self.current_token {
                                CsToken::Ident(k) => k.clone(),
                                CsToken::String(k) => k.clone(),
                                other => {
                                    return Err(format!(
                                        "Expected field name in object init, got {:?}",
                                        other
                                    ))
                                }
                            };
                            self.next_token();
                            self.expect(CsToken::Assign)?;
                            let value = self.parse_expression(0)?;
                            pairs.push((key, value));

                            if self.current_token == CsToken::Comma {
                                self.next_token();
                            } else {
                                break;
                            }
                        }

                        self.expect(CsToken::RBrace)?;
                        // Treat `new MyType { ... }` as ObjectLiteral for now — you can later
                        // wrap this in a new Expr::ConstructWithInit variant if desired
                        Ok(Expr::ObjectLiteral(pairs))
                    } else {
                        Err("Expected '(' or '{' after type name in 'new' expression".into())
                    }
                }
            }

            CsToken::Ident(n) => {
                let name = n.clone();
                self.next_token();

                // function call?
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

            // FIXED: Use Literal::Number instead of Literal::Float
            CsToken::Number(n) => {
                let raw = n.to_string();
                self.next_token();
                Ok(Expr::Literal(Literal::Number(raw)))
            }

            CsToken::String(s) => {
                let v = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::String(v)))
            }

            CsToken::LParen => {
                self.next_token();
                let e = self.parse_expression(0)?;
                self.expect(CsToken::RParen)?;
                Ok(e)
            }

            CsToken::LBrace => {
                self.next_token();
                let mut pairs = Vec::new();

                while self.current_token != CsToken::RBrace
                    && self.current_token != CsToken::Eof
                {
                    let key = match &self.current_token {
                        CsToken::Ident(k) => k.clone(),
                        CsToken::String(k) => k.clone(),
                        other => {
                            return Err(format!(
                                "Expected key in object literal, got {:?}",
                                other
                            ))
                        }
                    };
                    self.next_token();
                    self.expect(CsToken::Colon)?;
                    let value = self.parse_expression(0)?;
                    pairs.push((key, value));

                    if self.current_token == CsToken::Comma {
                        self.next_token();
                    } else {
                        break;
                    }
                }

                self.expect(CsToken::RBrace)?;
                Ok(Expr::ObjectLiteral(pairs))
            }

            _ => Err(format!("Unexpected primary {:?}", self.current_token)),
        }
    }

    // ------------------------------------------------------
    //  Infix parsers (.member, (), operators)
    // ------------------------------------------------------
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

            _ => Ok(left),
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

    // ------------------------------------------------------
    //  Map type keywords
    // ------------------------------------------------------
    fn map_type(&self, t: String) -> Type {
        match t.as_str() {
            "void" => Type::Void,
            
            // Floating point types
            "Half" => Type::Number(NumberKind::Float(16)),
            "float" => Type::Number(NumberKind::Float(32)),
            "double" => Type::Number(NumberKind::Float(64)),
            "decimal" => Type::Number(NumberKind::Decimal),
            
            // Signed integer types
            "sbyte" => Type::Number(NumberKind::Signed(8)),
            "short" => Type::Number(NumberKind::Signed(16)),
            "int" => Type::Number(NumberKind::Signed(32)),
            "long" => Type::Number(NumberKind::Signed(64)),
            "Int128" => Type::Number(NumberKind::Signed(128)),
            
            // Unsigned integer types
            "byte" => Type::Number(NumberKind::Unsigned(8)),
            "ushort" => Type::Number(NumberKind::Unsigned(16)),
            "uint" => Type::Number(NumberKind::Unsigned(32)),
            "ulong" => Type::Number(NumberKind::Unsigned(64)),
            "UInt128" => Type::Number(NumberKind::Unsigned(128)),
            
            // Other types
            "bool" => Type::Bool,
            "char" => Type::Number(NumberKind::Unsigned(16)), // C# char is 16-bit Unicode
            "string" => Type::String,
            
            // Native-sized integers (context-dependent, default to 64-bit)
            "nint" => Type::Number(NumberKind::Signed(64)),
            "nuint" => Type::Number(NumberKind::Unsigned(64)),
            
            "object" | "dynamic" => Type::Custom(t), // treat as custom for now
            // Custom types
            _ => Type::Custom(t),
        }
    }
}