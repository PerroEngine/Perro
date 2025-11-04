#[derive(Debug, Clone, PartialEq)]
pub enum CsToken {
    Using,
    Namespace,
    Class,
    New,
    Ident(String),
    Type(String),
    AccessModifier(String),
    Base,
    Var,
    Fn,
    Void,
    Number(String),
    String(String),
    This,

    // braces and punctuation
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Dot,
    Colon,
    Semicolon,
    Comma,

    // operators
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
    Eof,
}

#[derive(Debug, Clone)]
pub struct CsLexer {
    input: Vec<char>,
    pos: usize,
}

impl CsLexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
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

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.peek().is_some_and(|c| c.is_whitespace()) {
                self.advance();
            }

            // line comment //
            if self.peek() == Some('/') && self.input.get(self.pos + 1) == Some(&'/') {
                while let Some(c) = self.advance() {
                    if c == '\n' {
                        break;
                    }
                }
                continue;
            }

            // block comment /* ... */
            if self.peek() == Some('/') && self.input.get(self.pos + 1) == Some(&'*') {
                self.advance();
                self.advance();
                while self.pos + 1 < self.input.len()
                    && !(self.input[self.pos] == '*' && self.input[self.pos + 1] == '/')
                {
                    self.advance();
                }
                self.advance();
                self.advance();
                continue;
            }

            break;
        }
    }

    fn read_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        self.input[start..self.pos].iter().collect()
    }

    fn read_number(&mut self) -> CsToken {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        CsToken::Number(s)
    }

    pub fn next_token(&mut self) -> CsToken {
        self.skip_whitespace_and_comments();
        if self.peek().is_none() {
            return CsToken::Eof;
        }

        let ch = self.advance().unwrap();
        match ch {
            '{' => CsToken::LBrace,
            '}' => CsToken::RBrace,
            '(' => CsToken::LParen,
            ')' => CsToken::RParen,
            '[' => CsToken::LBracket,
            ']' => CsToken::RBracket,
            ';' => CsToken::Semicolon,
            ',' => CsToken::Comma,
            '.' => CsToken::Dot,
            ':' => CsToken::Colon,
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    CsToken::PlusEq
                } else {
                    CsToken::Plus
                }
            }
            '-' => {
                if self.peek() == Some('=') {
                    self.advance();
                    CsToken::MinusEq
                } else {
                    CsToken::Minus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    CsToken::MulEq
                } else {
                    CsToken::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    CsToken::DivEq
                } else {
                    CsToken::Slash
                }
            }
            '=' => CsToken::Assign,
            '"' => {
                let start = self.pos;
                while let Some(c) = self.advance() {
                    if c == '"' {
                        break;
                    }
                }
                let s: String = self.input[start..self.pos - 1].iter().collect();
                CsToken::String(s)
            }
            c if c.is_ascii_digit() => {
                self.pos -= 1;
                self.read_number()
            }
            c if c.is_alphabetic() || c == '_' => {
                self.pos -= 1;
                let ident = self.read_identifier();
                let lower = ident.to_lowercase();
                match lower.as_str() {
                    "using" => CsToken::Using,
                    "namespace" => CsToken::Namespace,
                    "class" => CsToken::Class,
                    "public" | "private" | "protected" | "internal" => {
                        CsToken::AccessModifier(lower)
                    }
                    "base" => CsToken::Base,
                    "new" => CsToken::New,
                    "this" => CsToken::This,
                    "var" => CsToken::Var,
                    "void" => CsToken::Void,
                    _ => CsToken::Ident(ident),
                }
            }
            _ => self.next_token(), // skip unexpected
        }
    }
}