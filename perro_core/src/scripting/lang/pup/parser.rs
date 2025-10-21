// scripting/pup/parser.rs

use crate::lang::ast::*;
use crate::lang::pup::lexer::{PupLexer, PupToken};
use crate::lang::pup::api::PupAPI;

pub struct PupParser {
    lexer: PupLexer,
    current_token: PupToken,
}

/// Different kinds of assignment
enum AssignKind {
    Set,
    Add,
    Sub,
    Mul,
    Div,
}

impl PupParser {
    pub fn new(input: &str) -> Self {
        let mut lex = PupLexer::new(input);
        let cur = lex.next_token();
        Self { lexer: lex, current_token: cur }
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

    pub fn parse_script(&mut self) -> Result<Script, String> {
        self.expect(PupToken::Extends)?;
        let node_type = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after extends".into());
        };
        self.next_token();

        let mut exports   = Vec::new();
        let mut variables = Vec::new();
        let mut functions = Vec::new();
        let mut structs   = Vec::new();

        while self.current_token != PupToken::Eof {
            match &self.current_token {
                PupToken::At => {
                    self.next_token();
                    self.expect(PupToken::Export)?;
                    exports.push(self.parse_export()?);
                }
                PupToken::Struct => {
                    structs.push(self.parse_struct_def()?);
                }
                PupToken::Let => {
                    variables.push(self.parse_variable_decl()?);
                }
                PupToken::Fn => {
                    functions.push(self.parse_function()?);
                }
                other => {
                    return Err(format!("Unexpected top‐level token {:?}", other));
                }
            }
        }

        Ok(Script { node_type, exports, variables, functions, structs })
    }

fn parse_struct_def(&mut self) -> Result<StructDef, String> {
    self.expect(PupToken::Struct)?;

    // Parse struct name
    let name = if let PupToken::Ident(n) = &self.current_token {
        n.clone()
    } else {
        return Err("Expected struct name after 'struct'".into());
    };
    self.next_token();

    // ✅ Optional inheritance: "extends BaseStruct"
    let mut base: Option<String> = None;
    if self.current_token == PupToken::Extends {
        self.next_token();
        if let PupToken::Ident(base_name) = &self.current_token {
            base = Some(base_name.clone());
            self.next_token();
        } else {
            return Err("Expected base struct name after 'extends'".into());
        }
    }

    // Struct body
    self.expect(PupToken::LBrace)?;
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    while self.current_token != PupToken::RBrace && self.current_token != PupToken::Eof {
        match &self.current_token {
            // Functions
            PupToken::Fn => {
                methods.push(self.parse_function()?);
            }

            // Fields
            PupToken::Ident(_) | PupToken::Type(_) => {
                fields.push(self.parse_field()?);
                if self.current_token == PupToken::Comma {
                    self.next_token();
                }
            }

            PupToken::Let => {
                self.next_token();
                fields.push(self.parse_field()?);
                if self.current_token == PupToken::Comma {
                    self.next_token();
                }
            }

            other => {
                return Err(format!("Unexpected token {:?} in struct {}", other, name));
            }
        }
    }

    self.expect(PupToken::RBrace)?;

    // ✅ Include base in StructDef
    Ok(StructDef { name, fields, methods, base })
}

fn parse_field(&mut self) -> Result<StructField, String> {
    let field_name = if let PupToken::Ident(n) = &self.current_token {
        n.clone()
    } else {
        return Err("Expected field name".into());
    };
    self.next_token();

    self.expect(PupToken::Colon)?;
    let typ = self.parse_type()?;

    Ok(StructField { name: field_name, typ })
}

    fn parse_export(&mut self) -> Result<Variable, String> {
        // '@' and 'export' consumed
        self.expect(PupToken::Let)?;
        
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after export let".into());
        };
        self.next_token();

        self.expect(PupToken::Colon)?;
        let typ = Some(self.parse_type()?);

        Ok(Variable { name, typ, value: None })
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
        Ok(Function {
            name,
            params,
            body,
            is_trait_method: is_trait,
            return_type: Type::Void,
        })
    }

    fn parse_param(&mut self) -> Result<Param, String> {
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected parameter name".into());
        };
        self.next_token();
        self.expect(PupToken::Colon)?;
        let typ = self.parse_type()?;
        Ok(Param { name, typ })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let ty = match &self.current_token {
            PupToken::Type(t) if t == "float"  => Type::Float,
            PupToken::Type(t) if t == "int"    => Type::Int,
            PupToken::Type(t) if t == "number" => Type::Number,
            PupToken::Type(t) if t == "bool"   => Type::Bool,
            PupToken::Type(t) if t == "string" => Type::String,
            PupToken::Type(t) if t == "script" => Type::Script,
            PupToken::Ident(n) => Type::Custom(n.clone()),
            _ => return Err("Expected type".into()),
        };
        self.next_token();
        Ok(ty)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(PupToken::LBrace)?;
        let mut stmts = Vec::new();
        while self.current_token != PupToken::RBrace
           && self.current_token != PupToken::Eof
        {
            stmts.push(self.parse_statement()?);
        }
        self.expect(PupToken::RBrace)?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        if self.current_token == PupToken::Let {
            return self.parse_variable_decl().map(Stmt::VariableDecl);
        }

        if self.current_token == PupToken::Pass {
            self.next_token();
            return Ok(Stmt::Pass);
        }

        let lhs = self.parse_expression(0)?;

        if let Some(kind) = self.take_assign_op() {
            let rhs = self.parse_expression(0)?;
            return self.make_assign_stmt(lhs, kind, rhs);
        }

        Ok(Stmt::Expr(lhs))
    }

    fn take_assign_op(&mut self) -> Option<AssignKind> {
        let k = match self.current_token {
            PupToken::Assign  => AssignKind::Set,
            PupToken::PlusEq  => AssignKind::Add,
            PupToken::MinusEq => AssignKind::Sub,
            PupToken::MulEq   => AssignKind::Mul,
            PupToken::DivEq   => AssignKind::Div,
            _ => return None,
        };
        self.next_token();
        Some(k)
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
            Expr::ScriptAccess(obj, field) => {
                if let Expr::Ident(var) = *obj {
                    Ok(match kind {
                        AssignKind::Set => Stmt::ScriptAssign(var, field, rhs),
                        AssignKind::Add => Stmt::ScriptAssignOp(var, field, Op::Add, rhs),
                        AssignKind::Sub => Stmt::ScriptAssignOp(var, field, Op::Sub, rhs),
                        AssignKind::Mul => Stmt::ScriptAssignOp(var, field, Op::Mul, rhs),
                        AssignKind::Div => Stmt::ScriptAssignOp(var, field, Op::Div, rhs),
                    })
                } else {
                    Err("Invalid LHS for script‐access".into())
                }
            }
            other => Err(format!("Invalid LHS of assignment: {:?}", other)),
        }
    }

    fn parse_variable_decl(&mut self) -> Result<Variable, String> {
        self.expect(PupToken::Let)?;
        
        let name = if let PupToken::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after let".into());
        };
        self.next_token();

        let mut typ: Option<Type> = None;
        let mut value: Option<Expr> = None;

        if self.current_token == PupToken::Colon {
            self.next_token();
            typ = Some(self.parse_type()?);
        }

        if self.current_token == PupToken::Assign {
            self.next_token();
            value = Some(self.parse_expression(0)?);
        }

        Ok(Variable { name, typ, value })
    }

    fn parse_expression(&mut self, prec: u8) -> Result<Expr, String> {
        let mut left = self.parse_primary()?;
        while prec < self.get_precedence() {
            left = self.parse_infix(left)?;
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match &self.current_token {
PupToken::New => {
    self.next_token();

    // Accept Ident (API name) for constructor
    let api_name = if let PupToken::Ident(n) = &self.current_token {
        n.clone()
    } else {
        return Err("Expected type/API name after 'new'".into());
    };
    self.next_token();

    self.expect(PupToken::LParen)?;
    let mut args = Vec::new();
    if self.current_token != PupToken::RParen {
        args.push(self.parse_expression(0)?);
        while self.current_token == PupToken::Comma {
            self.next_token();
            args.push(self.parse_expression(0)?);
        }
    }
    self.expect(PupToken::RParen)?;

    Ok(Expr::ApiCall(
        PupAPI::resolve(&api_name, "new")
            .ok_or_else(|| format!("Type/API '{}' has no .new() constructor", api_name))?,
        args,
    ))
}


            PupToken::SelfAccess => {
                self.next_token();
                Ok(Expr::SelfAccess)
            }
            PupToken::Super => {
                self.next_token();
                Ok(Expr::BaseAccess)
            }
            PupToken::Ident(n) => {
                let name = n.clone();
                self.next_token();

                if self.current_token == PupToken::LParen {
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
                    Ok(Expr::Call(Box::new(Expr::Ident(name)), args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            PupToken::Number(n) => {
                let v = *n;
                self.next_token();
                Ok(Expr::Literal(Literal::Float(v)))
            }
            PupToken::String(s) => {
                let v = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::String(v)))
            }
            PupToken::InterpolatedString(s) => {
                let v = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::Interpolated(v)))
            }
            PupToken::LParen => {
                self.next_token();
                let e = self.parse_expression(0)?;
                self.expect(PupToken::RParen)?;
                Ok(e)
            }
            PupToken::LBrace => {
                self.next_token();
                let mut pairs = Vec::new();

                while self.current_token != PupToken::RBrace && self.current_token != PupToken::Eof {
                    let key = match &self.current_token {
                        PupToken::Ident(k) => k.clone(),
                        PupToken::String(k) => k.clone(),
                        other => return Err(format!("Expected key in object literal, got {:?}", other)),
                    };
                    self.next_token();

                    self.expect(PupToken::Colon)?;
                    let value = self.parse_expression(0)?;
                    pairs.push((key, value));

                    if self.current_token == PupToken::Comma {
                        self.next_token();
                    } else {
                        break;
                    }
                }

                self.expect(PupToken::RBrace)?;
                Ok(Expr::ObjectLiteral(pairs))
            }
            other => Err(format!("Unexpected primary {:?}", other)),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, String> {
        match &self.current_token {
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

                if let Expr::MemberAccess(obj, method) = &left {
                    if let Expr::Ident(module_name) = &**obj {
                        if let Some(api_semantic) = PupAPI::resolve(module_name, method) {
                            return Ok(Expr::ApiCall(api_semantic, args));
                        }
                    }
                }

                Ok(Expr::Call(Box::new(left), args))
            }

            PupToken::Dot => {
                self.next_token();
                let f = if let PupToken::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected field after .".into());
                };
                self.next_token();
                Ok(Expr::MemberAccess(Box::new(left), f))
            }

            PupToken::DoubleColon => {
                self.next_token();
                let f = if let PupToken::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected ident after ::".into());
                };
                self.next_token();
                Ok(Expr::ScriptAccess(Box::new(left), f))
            }

            PupToken::Star => {
                self.next_token();
                let r = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Mul, Box::new(r)))
            }

            PupToken::Slash => {
                self.next_token();
                let r = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Div, Box::new(r)))
            }

            PupToken::Plus => {
                self.next_token();
                let r = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Add, Box::new(r)))
            }

            PupToken::Minus => {
                self.next_token();
                let r = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Sub, Box::new(r)))
            }

            _ => Ok(left),
        }
    }

    fn get_precedence(&self) -> u8 {
        match &self.current_token {
            PupToken::LParen => 5,
            PupToken::Dot | PupToken::DoubleColon => 4,
            PupToken::Star | PupToken::Slash => 3,
            PupToken::Plus | PupToken::Minus => 2,
            _ => 0,
        }
    }
}