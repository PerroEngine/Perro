use crate::lang::ast::*;
use crate::lang::csharp::lexer::{CsLexer, CsToken};
use crate::lang::csharp::api::CSharpAPI;

pub struct CsParser {
    lexer: CsLexer,
    current_token: CsToken,
}

enum AssignKind {
    Set,
    Add,
    Sub,
    Mul,
    Div,
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
        let mut node_type = String::new();
        let mut variables = Vec::new();
        let mut functions = Vec::new();

        // skip preambles
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
        if let CsToken::Ident(_class_name) = &self.current_token {
            self.next_token();
        }

        // base class
        if self.current_token == CsToken::Colon {
            self.next_token();
            if let CsToken::Ident(base) = &self.current_token {
                node_type = base.clone();
                self.next_token();
            } else {
                return Err("Expected base identifier after ':'".into());
            }
        }

        self.expect(CsToken::LBrace)?;

        while self.current_token != CsToken::RBrace && self.current_token != CsToken::Eof {
            while matches!(self.current_token, CsToken::AccessModifier(_)) {
                self.next_token();
            }

            match &self.current_token {
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
                    self.next_token();
                }
            }
        }

        self.expect(CsToken::RBrace)?;

        Ok(Script {
            node_type,
            exports: vec![],
            variables,
            functions,
        })
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
            value = Some(self.parse_expression(0)?);
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

        Ok(Function {
            name: trait_name,
            params,
            body,
            is_trait_method: is_trait,
            return_type: self.map_type(return_type),
        })
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
    if let Some(kind) = self.take_assign_op() {
        let right = self.parse_expression(0)?;
        let stmt = self.make_assign_stmt(left, kind, right)?;
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

    Ok(Stmt::Expr(left))
}

    // ------------------------------------------------------
    //  Assignment operator helpers
    // ------------------------------------------------------
    fn take_assign_op(&mut self) -> Option<AssignKind> {
        let kind = match self.current_token {
            CsToken::Assign => AssignKind::Set,
            CsToken::PlusEq => AssignKind::Add,
            CsToken::MinusEq => AssignKind::Sub,
            CsToken::MulEq => AssignKind::Mul,
            CsToken::DivEq => AssignKind::Div,
            _ => return None,
        };
        self.next_token();
        Some(kind)
    }

    fn make_assign_stmt(&mut self, lhs: Expr, kind: AssignKind, rhs: Expr) -> Result<Stmt, String> {
        match lhs {
            Expr::Ident(name) => Ok(match kind {
                AssignKind::Set => Stmt::Assign(name, rhs),
                AssignKind::Add => Stmt::AssignOp(name, Op::Add, rhs),
                AssignKind::Sub => Stmt::AssignOp(name, Op::Sub, rhs),
                AssignKind::Mul => Stmt::AssignOp(name, Op::Mul, rhs),
                AssignKind::Div => Stmt::AssignOp(name, Op::Div, rhs),
            }),
            Expr::MemberAccess(obj, field) => {
                let ma = Expr::MemberAccess(obj, field);
                Ok(match kind {
                    AssignKind::Set => Stmt::MemberAssign(ma, rhs),
                    AssignKind::Add => Stmt::MemberAssignOp(ma, Op::Add, rhs),
                    AssignKind::Sub => Stmt::MemberAssignOp(ma, Op::Sub, rhs),
                    AssignKind::Mul => Stmt::MemberAssignOp(ma, Op::Mul, rhs),
                    AssignKind::Div => Stmt::MemberAssignOp(ma, Op::Div, rhs),
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

            CsToken::Number(n) => {
                let v = *n;
                self.next_token();
                Ok(Expr::Literal(Literal::Float(v)))
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
            "float" | "double" => Type::Float,
            "int" => Type::Int,
            "bool" => Type::Bool,
            "string" => Type::String,
            _ => Type::Custom(t),
        }
    }
}