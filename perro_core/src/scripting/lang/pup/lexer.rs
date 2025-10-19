// scripting/pup/lexer.rs

#[derive(Debug, Clone, PartialEq)]
pub enum PupToken {
    Extends,
    Struct,
    Fn,
    Let,
    Pass,
    At,
    Dollar,
    Export,
    SelfAccess,
    Super,
    Ident(String),
    Type(String),
    Number(f32),
    String(String),
    InterpolatedString(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
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
}

impl PupLexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
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
        PupToken::Number(num.parse().unwrap())
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

    pub fn next_token(&mut self) -> PupToken {
        self.skip_whitespace();

        if self.peek().is_none() {
            return PupToken::Eof;
        }

        let ch = self.advance().unwrap();
        match ch {
            '$' => {
                if self.peek() == Some('"') {
                    self.advance(); // consume the quote
                    let start = self.pos;
                    while let Some(c) = self.advance() {
                        if c == '"' {
                            break;
                        }
                    }
                    let s: String = self.input[start..self.pos - 1].iter().collect();
                    PupToken::InterpolatedString(s)
                } else {
                    PupToken::Dollar
                }
            }

            '@' => PupToken::At,
            '{' => PupToken::LBrace,
            '}' => PupToken::RBrace,
            '(' => PupToken::LParen,
            ')' => PupToken::RParen,
            '.' => PupToken::Dot,
            ';' => PupToken::Semicolon,
            ':' => {
                if self.peek() == Some(':') {
                    self.advance();
                    PupToken::DoubleColon
                } else {
                    PupToken::Colon
                }
            }
            ',' => PupToken::Comma,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::Eq
                } else {
                    PupToken::Assign
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
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    PupToken::PlusEq
                } else {
                    PupToken::Plus
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
            _ if ch.is_ascii_digit() => {
                self.pos -= 1; 
                self.read_number()
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                self.pos -= 1; 
                let ident = self.read_identifier();
                match ident.as_str() {
                    "extends" => PupToken::Extends,
                    "struct" => PupToken::Struct,
                    "export" => PupToken::Export,
                    "fn" => PupToken::Fn,
                    "super" => PupToken::Super,
                    "self" => PupToken::SelfAccess,
                    "let" => PupToken::Let,
                    "pass" => PupToken::Pass,
                    "delta" => PupToken::Ident("delta".to_string()),
                    "float" => PupToken::Type("float".to_string()),
                    "int" => PupToken::Type("int".to_string()),
                    "number" => PupToken::Type("number".to_string()),
                    "string" => PupToken::Type("string".to_string()),
                    "bool" => PupToken::Type("bool".to_string()),
                    _ => PupToken::Ident(ident),
                }
            }
            _ => panic!("Unexpected character: {}", ch),
        }
    }
}