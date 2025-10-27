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
    Type(String),
    Number(String),
    String(String),
    True,
    False,
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
    prev_token: Option<PupToken>, // Track previous token
}

impl PupLexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            prev_token: None,
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
            } else if ch == '/' {
                // Check for comments
                if self.input.get(self.pos + 1) == Some(&'/') {
                    // Single line comment - skip to end of line
                    self.skip_single_line_comment();
                } else if self.input.get(self.pos + 1) == Some(&'*') {
                    // Multi-line comment - skip to */
                    self.skip_multi_line_comment();
                } else {
                    // Not a comment, break out
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn skip_single_line_comment(&mut self) {
        // Skip the '//'
        self.advance();
        self.advance();
        
        // Skip until newline or EOF
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                self.advance(); // Consume the newline
                break;
            }
            self.advance();
        }
    }

    fn skip_multi_line_comment(&mut self) {
        // Skip the '/*'
        self.advance();
        self.advance();
        
        // Skip until we find '*/'
        while let Some(ch) = self.peek() {
            if ch == '*' && self.input.get(self.pos + 1) == Some(&'/') {
                // Found end of comment
                self.advance(); // consume '*'
                self.advance(); // consume '/'
                break;
            }
            self.advance();
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
        let num_str: String = self.input[start..self.pos].iter().collect();
        PupToken::Number(num_str)
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

    fn is_type_keyword(s: &str) -> bool {
        matches!(s,
            // Core types
            "float" | "double" | "int" | "uint" | "string" | "bool" |
            // Extended
            "decimal" | "bigint" | "big" | "big_int" |
            // Explicit sizes for the stable ones
            "float_32" | "float_64" |
            "int_8" | "int_16" | "int_32" | "int_64" | "int_128" |
            "uint_8" | "uint_16" | "uint_32" | "uint_64" | "uint_128"
        )
    }

    pub fn next_token(&mut self) -> PupToken {
        self.skip_whitespace();

        if self.peek().is_none() {
            self.prev_token = Some(PupToken::Eof);
            return PupToken::Eof;
        }

        let ch = self.advance().unwrap();
        let tok = match ch {
            '$' => {
                if self.peek() == Some('"') {
                    self.advance(); // consume the quote
                    let start = self.pos;
                    while let Some(c) = self.advance() {
                        if c == '"' { break; }
                    }
                    let s: String = self.input[start..self.pos - 1].iter().collect();
                    PupToken::InterpolatedString(s)
                } else { PupToken::Dollar }
            }
            '@' => PupToken::At,
            '{' => PupToken::LBrace,
            '}' => PupToken::RBrace,
            '(' => PupToken::LParen,
            ')' => PupToken::RParen,
            '.' => PupToken::Dot,
            ';' => PupToken::Semicolon,
            ':' => if self.peek() == Some(':') { self.advance(); PupToken::DoubleColon } else { PupToken::Colon },
            ',' => PupToken::Comma,
            '=' => if self.peek() == Some('=') { self.advance(); PupToken::Eq } else { PupToken::Assign },
            '-' => if self.peek() == Some('=') { self.advance(); PupToken::MinusEq } else { PupToken::Minus },
            '+' => if self.peek() == Some('=') { self.advance(); PupToken::PlusEq } else { PupToken::Plus },
            '*' => if self.peek() == Some('=') { self.advance(); PupToken::MulEq } else { PupToken::Star },
            '/' => if self.peek() == Some('=') { self.advance(); PupToken::DivEq } else { PupToken::Slash },
            '"' => {
                let start = self.pos;
                while let Some(c) = self.advance() {
                    if c == '"' { break; }
                }
                let s: String = self.input[start..self.pos - 1].iter().collect();
                PupToken::String(s)
            }
            _ if ch.is_ascii_digit() => { self.pos -= 1; self.read_number() }
            _ if ch.is_alphabetic() || ch == '_' => {
                self.pos -= 1;
                let ident = self.read_identifier();

                match ident.as_str() {
                    "import"  => PupToken::Import,
                    "extends" => PupToken::Extends,
                    "struct"  => PupToken::Struct,
                    "new"     => PupToken::New,
                    "as" => PupToken::As,
                    "expose"  => PupToken::Expose,
                    "fn"      => PupToken::Fn,
                    "super"   => PupToken::Super,
                    "true" => PupToken::True,
                    "false" => PupToken::False,
                    "self"    => PupToken::SelfAccess,
                    "let" | "var"=> PupToken::Var,
                    "pass"    => PupToken::Pass,
                    "script"  => PupToken::Ident("script".to_string()),
                    "delta"   => PupToken::Ident("delta".to_string()),
                    s if Self::is_type_keyword(s) => PupToken::Type(s.to_string()),
                    _ => PupToken::Ident(ident),
                }
            }
            _ => panic!("Unexpected character: {}", ch),
        };

        self.prev_token = Some(tok.clone());
        tok
    }
}