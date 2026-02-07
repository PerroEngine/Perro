#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String), // name, position, Sprite2D
    Number(f32),
    String(String),

    At,     // @
    Equals, // =
    Comma,  // ,
    LParen, // (
    RParen, // )

    LBracket, // [
    RBracket, // ]

    Slash, // /
    Newline,
    Eof,
}

pub struct Lexer<'a> {
    src: &'a str,
    chars: std::str::Chars<'a>,
    peek: Option<char>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut chars = src.chars();
        let peek = chars.next();
        Self { src, chars, peek }
    }

    fn bump(&mut self) -> Option<char> {
        let cur = self.peek;
        self.peek = self.chars.next();
        cur
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek, Some(c) if c.is_whitespace()) {
            self.bump();
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_ws();

        let c = match self.bump() {
            Some(c) => c,
            None => return Token::Eof,
        };

        match c {
            '@' => Token::At,
            '=' => Token::Equals,
            ',' => Token::Comma,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            '/' => Token::Slash,

            '"' => {
                let mut s = String::new();
                while let Some(c) = self.bump() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                Token::String(s)
            }

            c if c.is_ascii_digit() || c == '-' => {
                let mut s = String::new();
                s.push(c);
                while matches!(self.peek, Some(p) if p.is_ascii_digit() || p == '.') {
                    s.push(self.bump().unwrap());
                }
                Token::Number(s.parse().unwrap())
            }

            c if c.is_alphanumeric() || c == '_' => {
                let mut s = String::new();
                s.push(c);
                while matches!(self.peek, Some(p) if p.is_alphanumeric() || p == '_') {
                    s.push(self.bump().unwrap());
                }
                Token::Ident(s)
            }

            _ => self.next_token(),
        }
    }
}
