use crate::lang::pup::api::{PupAPI, PupNodeSugar};
use std::collections::HashMap;

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
    If,
    Else,
    For,
    In,
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
    DotDot,
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
    PlusPlus,   // ++
    MinusMinus, // --
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
                // Skip // comments until newline
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
            } else if ch == '#' {
                // Skip # comments until newline
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

    fn read_number(&mut self, is_negative: bool) -> PupToken {
        let start_pos = self.pos;
        let mut num_str = String::new();

        if is_negative {
            num_str.push('-');
        }

        // Consume leading digits
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                num_str.push(self.advance().unwrap());
            } else {
                break;
            }
        }

        // Consume a single decimal point, but only if it's not part of `..`
        if self.peek() == Some('.') {
            // Check if the next character is also `.` (making it `..`)
            if self.input.get(self.pos + 1) == Some(&'.') {
                // This is `..`, don't consume the `.` as part of the number
                // The `.` will be handled by the `..` operator parsing
            } else {
                num_str.push(self.advance().unwrap());
                // Consume digits after the decimal point
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_digit() {
                        num_str.push(self.advance().unwrap());
                    } else {
                        break;
                    }
                }
            }
        }

        // Handle thousands separators (like Rust's `1_000_000`)
        // Keep them in the string; the parser or later stages will handle them.
        while self.peek() == Some('_') {
            num_str.push(self.advance().unwrap()); // Consume the underscore
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() {
                    num_str.push(self.advance().unwrap());
                } else {
                    break;
                }
            }
        }

        // After consuming all parts of the number, create the token
        PupToken::Number(num_str)
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
            "if" => PupToken::If,
            "else" => PupToken::Else,
            "for" => PupToken::For,
            "in" => PupToken::In,
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
        let Some(ch) = self.peek() else {
            return PupToken::Eof;
        };

        // ---------- numbers -----------
        if ch == '-'
            && self
                .input
                .get(self.pos + 1)
                .map_or(false, |next_ch| next_ch.is_ascii_digit())
        {
            self.advance(); // Consume the '-'
            return self.read_number(true); // Call read_number, indicating it's already negative
        } else if ch.is_ascii_digit() {
            return self.read_number(false); // Positive number
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
            '<' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Le
                } else {
                    PupToken::LessThan
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Ge
                } else {
                    PupToken::GreaterThan
                }
            }
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
            '.' => {
                if self.peek() == Some('.') {
                    self.advance();
                    PupToken::DotDot
                } else {
                    PupToken::Dot
                }
            }
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Eq
                } else {
                    PupToken::Assign
                }
            }
            '!' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Ne
                } else {
                    panic!("Unexpected character '!'");
                }
            }
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::PlusEq
                } else if self.peek() == Some('+') {
                    self.advance();
                    PupToken::PlusPlus
                } else {
                    PupToken::Plus
                }
            }
            '-' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::MinusEq
                } else if self.peek() == Some('-') {
                    self.advance();
                    PupToken::MinusMinus
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
