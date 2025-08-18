// scripting/pup/parser.rs

use crate::lang::pup::lexer::{Lexer, Token};
use crate::lang::ast::*;

pub struct PupParser {
    lexer: Lexer,
    current_token: Token,
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
        let mut lex = Lexer::new(input);
        let cur = lex.next_token();
        Self { lexer: lex, current_token: cur }
    }

    fn next_token(&mut self) {
        self.current_token = self.lexer.next_token();
    }

    fn expect(&mut self, tok: Token) -> Result<(), String> {
        if self.current_token == tok {
            self.next_token();
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", tok, self.current_token))
        }
    }

    pub fn parse_script(&mut self) -> Result<Script, String> {
        self.expect(Token::Extends)?;
        let node_type = if let Token::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after extends".into());
        };
        self.next_token();

        let mut exports   = Vec::new();
        let mut variables = Vec::new();
        let mut functions = Vec::new();

        while self.current_token != Token::Eof {
            match &self.current_token {
                Token::At => {
                    self.next_token();
                    self.expect(Token::Export)?;
                    exports.push(self.parse_export()?);
                }
                Token::Let => {
                    variables.push(self.parse_variable_decl()?);
                }
                Token::Fn => {
                    functions.push(self.parse_function()?);
                }
                other => {
                    return Err(format!("Unexpected top‐level token {:?}", other));
                }
            }
        }

        Ok(Script { node_type, exports, variables, functions })
    }

    fn parse_export(&mut self) -> Result<Variable, String> {
    // '@' and 'export' consumed
    self.expect(Token::Let)?;
    
    let name = if let Token::Ident(n) = &self.current_token {
        n.clone()
    } else {
        return Err("Expected identifier after export let".into());
    };
    self.next_token();

    self.expect(Token::Colon)?;
    let typ = Some(self.parse_type()?);

    // exports likely don't have initial values, so value is None
    Ok(Variable {
        name,
        typ,
        value: None,
    })
}


    fn parse_function(&mut self) -> Result<Function, String> {
        self.expect(Token::Fn)?;
        let name = if let Token::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected function name".into());
        };
        self.next_token();

        self.expect(Token::LParen)?;
        let mut params = Vec::new();
        if self.current_token != Token::RParen {
            params.push(self.parse_param()?);
            while self.current_token == Token::Comma {
                self.next_token();
                params.push(self.parse_param()?);
            }
        }
        self.expect(Token::RParen)?;

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
        let name = if let Token::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected parameter name".into());
        };
        self.next_token();
        self.expect(Token::Colon)?;
        let typ = self.parse_type()?;
        Ok(Param { name, typ })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let ty = match &self.current_token {
            Token::Type(t) if t=="float"  => Type::Float,
            Token::Type(t) if t=="int"    => Type::Int,
            Token::Type(t) if t=="number" => Type::Number,
            Token::Type(t) if t=="bool"   => Type::Bool,
            Token::Type(t) if t=="string" => Type::String,
            Token::Ident(n)                => Type::Custom(n.clone()),
            _ => return Err("Expected type".into()),
        };
        self.next_token();
        Ok(ty)
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.expect(Token::LBrace)?;
        let mut stmts = Vec::new();
        while self.current_token != Token::RBrace
           && self.current_token != Token::Eof
        {
            stmts.push(self.parse_statement()?);
        }
        self.expect(Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt, String> {
        // let / pass
        if self.current_token == Token::Let {
            return self.parse_variable_decl().map(Stmt::VariableDecl) ;
        }

        if self.current_token == Token::Pass {
            self.next_token();
            return Ok(Stmt::Pass);
        }

        // 1) parse LHS as an expression
        let lhs = self.parse_expression(0)?;

        // 2) if next is an assignment operator, build an assign‐stmt
        if let Some(kind) = self.take_assign_op() {
            let rhs = self.parse_expression(0)?;
            return self.make_assign_stmt(lhs, kind, rhs);
        }

        // 3) otherwise it's a bare expr‐stmt
        Ok(Stmt::Expr(lhs))
    }



    /// consume =, +=, -=, *= or /= and return its kind
    fn take_assign_op(&mut self) -> Option<AssignKind> {
        let k = match self.current_token {
            Token::Assign  => AssignKind::Set,
            Token::PlusEq  => AssignKind::Add,
            Token::MinusEq => AssignKind::Sub,
            Token::MulEq  => AssignKind::Mul,
            Token::DivEq => AssignKind::Div,
            _ => return None,
        };
        self.next_token();
        Some(k)
    }

    /// build the correct Stmt from lhs‐expr, assign‐kind, and rhs‐expr
    fn make_assign_stmt(
        &mut self,
        lhs: Expr,
        kind: AssignKind,
        rhs: Expr
    ) -> Result<Stmt, String> {
        match lhs {
            Expr::Ident(name) => {
                Ok(match kind {
                    AssignKind::Set => Stmt::Assign(name, rhs),
                    AssignKind::Add => Stmt::AssignOp(name, Op::Add, rhs),
                    AssignKind::Sub => Stmt::AssignOp(name, Op::Sub, rhs),
                    AssignKind::Mul => Stmt::AssignOp(name, Op::Mul, rhs),
                    AssignKind::Div => Stmt::AssignOp(name, Op::Div, rhs),
                })
            }

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
        self.expect(Token::Let)?;
        
        let name = if let Token::Ident(n) = &self.current_token {
            n.clone()
        } else {
            return Err("Expected identifier after let".into());
        };
        self.next_token();

        let mut typ: Option<Type> = None;
        let mut value: Option<Expr> = None;

        if self.current_token == Token::Colon {
            self.next_token();
            typ = Some(self.parse_type()?);
        }

        if self.current_token == Token::Assign {
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
            Token::Ident(s) if s == "self" => {
                self.next_token();
                Ok(Expr::SelfAccess)
            }
            Token::Ident(n) => {
                let name = n.clone();
                self.next_token();
                if self.current_token == Token::LParen {
                    // function call
                    self.next_token();
                    let mut args = Vec::new();
                    if self.current_token != Token::RParen {
                        args.push(self.parse_expression(0)?);
                        while self.current_token == Token::Comma {
                            self.next_token();
                            args.push(self.parse_expression(0)?);
                        }
                    }
                    self.expect(Token::RParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            Token::Number(n) => {
                let v = *n;
                self.next_token();
                Ok(Expr::Literal(Literal::Float(v)))
            }
            Token::String(s) => {
                let v = s.clone();
                self.next_token();
                Ok(Expr::Literal(Literal::String(v)))
            }
            Token::LParen => {
                self.next_token();
                let e = self.parse_expression(0)?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            other => Err(format!("Unexpected primary {:?}", other)),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, String> {
        match &self.current_token {
            Token::Dot => {
                self.next_token();
                let f = if let Token::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected field after .".into());
                };
                self.next_token();
                Ok(Expr::MemberAccess(Box::new(left), f))
            }
            Token::DoubleColon => {
                self.next_token();
                let f = if let Token::Ident(n) = &self.current_token {
                    n.clone()
                } else {
                    return Err("Expected ident after ::".into());
                };
                self.next_token();
                Ok(Expr::ScriptAccess(Box::new(left), f))
            }
            Token::Star => {
                self.next_token();
                let r = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Mul, Box::new(r)))
            }
            Token::Slash => {
                self.next_token();
                let r = self.parse_expression(2)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Div, Box::new(r)))
            }
            Token::Plus => {
                self.next_token();
                let r = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Add, Box::new(r)))
            }
            Token::Minus => {
                self.next_token();
                let r = self.parse_expression(1)?;
                Ok(Expr::BinaryOp(Box::new(left), Op::Sub, Box::new(r)))
            }
            _ => Ok(left),
        }
    }

    fn get_precedence(&self) -> u8 {
        match &self.current_token {
            Token::Dot | Token::DoubleColon => 4,
            Token::Star | Token::Slash      => 3,
            Token::Plus | Token::Minus      => 2,
            _ => 0,
        }
    }
}