#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Ident(&'a str), // name, position, Sprite2D
    Number(f32),
    String(String),

    At,      // @
    Dollar,  // $
    Percent, // %
    Equals,  // =
    Comma,   // ,
    LParen,  // (
    RParen,  // )
    LBrace,  // {
    RBrace,  // }
    Colon,   // :

    LBracket, // [
    RBracket, // ]

    Slash, // /
    Newline,
    Eof,

    True,
    False,
}

pub struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.bump();
        }
    }

    fn skip_until_newline(&mut self) {
        while let Some(c) = self.peek() {
            if c == '\n' || c == '\r' {
                break;
            }
            self.bump();
        }
    }

    pub fn next_token(&mut self) -> Token<'a> {
        self.skip_ws();

        let start = self.pos;
        let c = match self.bump() {
            Some(c) => c,
            None => return Token::Eof,
        };

        match c {
            '@' => Token::At,
            '$' => Token::Dollar,
            '%' => Token::Percent,
            '=' => Token::Equals,
            ',' => Token::Comma,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ':' => Token::Colon,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            '/' => {
                // Support // line comments while preserving '/' token for closing tags.
                if self.peek() == Some('/') {
                    self.bump();
                    self.skip_until_newline();
                    return self.next_token();
                }
                Token::Slash
            }
            '#' => {
                // Support # line comments.
                self.skip_until_newline();
                self.next_token()
            }

            '"' => {
                let mut s = String::new();
                while let Some(c) = self.bump() {
                    if c == '\\' && self.peek() == Some('"') {
                        self.bump();
                        s.push('"');
                        continue;
                    }
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                Token::String(s)
            }

            c if c.is_ascii_digit()
                || (c == '-'
                    && matches!(self.peek(), Some(p) if p.is_ascii_digit() || p == '.')) =>
            {
                while matches!(self.peek(), Some(p) if p.is_ascii_digit() || p == '.' || p == 'e' || p == 'E' || p == '+' || p == '-')
                {
                    self.bump();
                }
                match self.src[start..self.pos].parse::<f32>() {
                    Ok(v) => Token::Number(v),
                    Err(_) => self.next_token(),
                }
            }

            c if c.is_alphanumeric() || c == '_' => {
                while matches!(self.peek(), Some(p) if p.is_alphanumeric() || p == '_') {
                    self.bump();
                }
                match &self.src[start..self.pos] {
                    "true" => Token::True,
                    "false" => Token::False,
                    ident => Token::Ident(ident),
                }
            }

            _ => self.next_token(),
        }
    }
}
