use std::collections::HashMap;
use crate::lang::ast::*;
use crate::lang::api_modules::{ApiModule, NodeSugarApi};
use crate::lang::pup::api::{PupAPI, PupNodeSugar};

// =========================================================
// TOKENS & LEXER
// =========================================================

#[derive(Debug, Clone, PartialEq)]
pub enum PupToken {
    Import,
    Extends,
    Struct,
    New,
    Fn,
    Var,
    Pass,
    At,
    As,
    Dollar,
    Expose,
    SelfAccess,
    Super,
    Ident(String),
    Number(String),
    String(String),
    True,
    False,
    InterpolatedString(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LessThan,
    GreaterThan,
    Dot,
    Colon,
    DoubleColon,
    Semicolon,
    Assign,
    MinusEq,
    PlusEq,
    MulEq,
    DivEq,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Comma,
    Eof,
}

#[derive(Debug, Clone)]
pub struct PupLexer {
    input: Vec<char>,
    pos: usize,
    prev_token: Option<PupToken>,
}

impl PupLexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            prev_token: None,
        }
    }

    pub fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek();
        self.pos += 1;
        ch
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == '/' && self.input.get(self.pos + 1) == Some(&'/') {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

     fn read_number(&mut self) -> PupToken {
        let start = self.pos;

        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }

        let num: String = self.input[start..self.pos].iter().collect();
        PupToken::Number(num)
    }

    fn read_ident(&mut self, first: char) -> PupToken {
        let start = self.pos - 1;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        match s.as_str() {
            "import" => PupToken::Import,
            "extends" => PupToken::Extends,
            "struct" => PupToken::Struct,
            "new" => PupToken::New,
            "fn" => PupToken::Fn,
            "var" | "let" => PupToken::Var,
            "pass" => PupToken::Pass,
            "as" => PupToken::As,
            "expose" => PupToken::Expose,
            "true" => PupToken::True,
            "false" => PupToken::False,
            "self" => PupToken::SelfAccess,
            "super" => PupToken::Super,
            _ => PupToken::Ident(s),
        }
    }

     pub fn next_token(&mut self) -> PupToken {
        self.skip_whitespace();

        // peek first so we can decide before consuming
        let Some(ch) = self.peek() else { return PupToken::Eof; };

        // ---------- numbers -----------
        if ch.is_ascii_digit() {
            // do NOT consume the '(' that might precede it
            return self.read_number();
        }

        // ---------- everything else -----------
        let ch = self.advance().unwrap();
        let token = match ch {
            '(' => PupToken::LParen,
            ')' => PupToken::RParen,
            '{' => PupToken::LBrace,
            '}' => PupToken::RBrace,
            '[' => PupToken::LBracket,
            ']' => PupToken::RBracket,
            '<' => PupToken::LessThan,
            '>' => PupToken::GreaterThan,
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    PupToken::DoubleColon
                } else {
                    PupToken::Colon
                }
            }
            ';' => PupToken::Semicolon,
            ',' => PupToken::Comma,
            '.' => PupToken::Dot,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Eq
                } else {
                    PupToken::Assign
                }
            }
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::PlusEq
                } else {
                    PupToken::Plus
                }
            }
            '-' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::MinusEq
                } else {
                    PupToken::Minus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::MulEq
                } else {
                    PupToken::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::DivEq
                } else {
                    PupToken::Slash
                }
            }
            '"' => {
                let start = self.pos;
                while let Some(c) = self.advance() {
                    if c == '"' {
                        break;
                    }
                }
                let s: String = self.input[start..self.pos - 1].iter().collect();
                PupToken::String(s)
            }
            c if c.is_ascii_alphabetic() || c == '_' => self.read_ident(c),
            '@' => PupToken::At,
            c => panic!("Unexpected character {c}"),
        };

        self.prev_token = Some(token.clone());
        token
    }
}