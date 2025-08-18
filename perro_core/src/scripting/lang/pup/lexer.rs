// scripting/pup/lexer.rs

#[derive(Debug, Clone, PartialEq,)]
pub enum Token {
    Extends,
    Fn,
    Let,
    Pass,
    At,
    Export,
    Ident(String),
    Type(String),  // float, int, etc.
    Number(f32),
    String(String),
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
pub struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
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

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '.' {
                self.advance();
            } else {
                break;
            }
        }
        let num_str: String = self.input[start..self.pos].iter().collect();
        Token::Number(num_str.parse().unwrap())
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

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        
        if self.peek().is_none() {
            return Token::Eof;
        }

        let ch = self.advance().unwrap();
        match ch {
            '@' => Token::At,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '.' => Token::Dot,
            ';' => Token::Semicolon,
            ':' => {
                if self.peek() == Some(':') {
                    self.advance(); // consume second ':'
                    Token::DoubleColon
                } else {
                    Token::Colon
                }
            }
            ',' => Token::Comma,
            '=' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::Eq
                } else {
                    Token::Assign
                }
            }
            '-' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::MinusEq
                } else {
                    Token::Minus
                }
            }
            '+' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::PlusEq
                } else {
                    Token::Plus
                }
            }
            '*' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::MulEq
                } else {
                    Token::Star
                }
            }
            '/' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::DivEq
                } else {
                    Token::Slash
                }
            }
            '"' => {
                let start = self.pos;
                while let Some(ch) = self.advance() {
                    if ch == '"' {
                        break;
                    }
                }
                let s: String = self.input[start..self.pos-1].iter().collect();
                Token::String(s)
            }
            _ if ch.is_ascii_digit() => {
                self.pos -= 1; // backtrack
                self.read_number()
            }
            _ if ch.is_alphabetic() || ch == '_' => {
                self.pos -= 1; // backtrack
                let ident = self.read_identifier();
                match ident.as_str() {
                    "extends" => Token::Extends,
                    "export" => Token::Export,
                    "fn" => Token::Fn,
                    "let" => Token::Let,
                    "pass" => Token::Pass,
                    "delta" => Token::Ident("delta".to_string()),
                    "self" => Token::Ident("self".to_string()),
                    "float" => Token::Type("float".to_string()),
                    "int" => Token::Type("int".to_string()),
                    "number" => Token::Type("number".to_string()),
                    "string" => Token::Type("string".to_string()),
                    "bool" => Token::Type("bool".to_string()),
                    _ => Token::Ident(ident),
                }
            }
            _ => panic!("Unexpected character: {}", ch),
        }
    }
}