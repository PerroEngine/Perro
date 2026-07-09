use std::fmt;

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

    Error(LexError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    MalformedNumber,
    UnknownCharacter(char),
    UnterminatedString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub span: Span,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            LexErrorKind::MalformedNumber => "malformed number".to_string(),
            LexErrorKind::UnknownCharacter(c) => format!("unknown character `{c}`"),
            LexErrorKind::UnterminatedString => "unterminated string".to_string(),
        };
        write!(
            f,
            "{message} at bytes {}..{}",
            self.span.start, self.span.end
        )
    }
}

impl std::error::Error for LexError {}

impl LexError {
    fn new(kind: LexErrorKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: Span { start, end },
        }
    }
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
        loop {
            self.skip_ws();
            match (self.peek(), self.src[self.pos..].chars().nth(1)) {
                (Some('#'), _) => {
                    self.bump();
                    self.skip_until_newline();
                }
                (Some('/'), Some('/')) => {
                    self.bump();
                    self.bump();
                    self.skip_until_newline();
                }
                _ => break,
            }
        }

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
            '/' => Token::Slash,

            '"' => {
                let mut s = String::new();
                let mut terminated = false;
                while let Some(c) = self.bump() {
                    if c == '\\' && self.peek() == Some('"') {
                        self.bump();
                        s.push('"');
                        continue;
                    }
                    if c == '"' {
                        terminated = true;
                        break;
                    }
                    s.push(c);
                }
                if terminated {
                    Token::String(s)
                } else {
                    Token::Error(LexError::new(
                        LexErrorKind::UnterminatedString,
                        start,
                        self.pos,
                    ))
                }
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
                    Ok(v) if v.is_finite() => Token::Number(v),
                    _ => Token::Error(LexError::new(
                        LexErrorKind::MalformedNumber,
                        start,
                        self.pos,
                    )),
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

            other => Token::Error(LexError::new(
                LexErrorKind::UnknownCharacter(other),
                start,
                self.pos,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_malformed_number_with_span() {
        let mut lexer = Lexer::new("12.3.4]");
        assert_eq!(
            lexer.next_token(),
            Token::Error(LexError {
                kind: LexErrorKind::MalformedNumber,
                span: Span { start: 0, end: 6 },
            })
        );
        assert_eq!(lexer.next_token(), Token::RBracket);
    }

    #[test]
    fn reports_unknown_multibyte_character_with_byte_span() {
        let mut lexer = Lexer::new(" 🐕ok");
        assert_eq!(
            lexer.next_token(),
            Token::Error(LexError {
                kind: LexErrorKind::UnknownCharacter('🐕'),
                span: Span { start: 1, end: 5 },
            })
        );
        assert_eq!(lexer.next_token(), Token::Ident("ok"));
    }

    #[test]
    fn reports_unterminated_string_with_span() {
        let mut lexer = Lexer::new("\"open");
        assert_eq!(
            lexer.next_token(),
            Token::Error(LexError {
                kind: LexErrorKind::UnterminatedString,
                span: Span { start: 0, end: 5 },
            })
        );
    }

    #[test]
    fn skips_many_comments_without_recursion() {
        let src = "# comment\n".repeat(100_000) + "done";
        let mut lexer = Lexer::new(&src);
        assert_eq!(lexer.next_token(), Token::Ident("done"));
    }
}
