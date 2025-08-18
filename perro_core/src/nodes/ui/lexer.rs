#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LBracket,           // '['
    RBracket,           // ']'
    Slash,              // '/'
    Equals,             // '='
    Identifier(String), // tag names, attribute names
    StringLiteral(String),  // "some string"
    Text(String),       // text nodes (outside tags)
    Eof,
}

#[derive(Debug, Clone)]
pub struct Lexer {
    input: Vec<char>,
    pub pos: usize,
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

    fn peek_next(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
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

    // Reads identifiers (tag names, attribute names)
    fn read_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        self.input[start..self.pos].iter().collect()
    }

    // Reads quoted string literals like "foo bar"
    fn read_string_literal(&mut self) -> Result<String, String> {
        // We expect starting quote already consumed
        let mut value = String::new();
        while let Some(ch) = self.advance() {
            if ch == '"' {
                return Ok(value);
            } else if ch == '\\' {
                // Handle escape sequences if you want (basic support)
                if let Some(next_ch) = self.advance() {
                    value.push(match next_ch {
                        'n' => '\n',
                        't' => '\t',
                        '\\' => '\\',
                        '"' => '"',
                        other => other,
                    });
                } else {
                    return Err("Unexpected EOF in string escape".into());
                }
            } else {
                value.push(ch);
            }
        }
        Err("Unterminated string literal".into())
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();

        if let Some(ch) = self.advance() {
            match ch {
                '[' => Ok(Token::LBracket),
                ']' => Ok(Token::RBracket),
                '/' => Ok(Token::Slash),
                '=' => Ok(Token::Equals),
                '"' => {
                    let s = self.read_string_literal()?;
                    Ok(Token::StringLiteral(s))
                }
                ch if ch.is_alphanumeric() || ch == '-' || ch == '_' => {
                    self.pos -= 1; // put back first char
                    let ident = self.read_identifier();
                    Ok(Token::Identifier(ident))
                }
                // Anything else is text until next '[' or EOF
                _ => {
                    let mut text = String::new();
                    text.push(ch);
                    while let Some(&next_ch) = self.input.get(self.pos) {
                        if next_ch == '[' {
                            break;
                        }
                        text.push(next_ch);
                        self.pos += 1;
                    }
                    Ok(Token::Text(text.trim().to_string()))
                }
            }
        } else {
            Ok(Token::Eof)
        }
    }
}