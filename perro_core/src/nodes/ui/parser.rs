use std::{collections::HashMap};
use uuid::Uuid;

use crate::ast::{FurElement, FurNode};

// =================== LEXER ===================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LBracket,
    RBracket,
    Slash,
    Equals,
    Identifier(String),
    StringLiteral(String),
    Text(String),
    Eof,
}

pub struct Lexer {
    input: String,
    pub pos: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self { input: input.to_string(), pos: 0 }
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace_and_comments()?;

        if let Some(c) = self.peek_char() {
            match c {
                '[' => { self.advance(); Ok(Token::LBracket) }
                ']' => { self.advance(); Ok(Token::RBracket) }
                '/' => { self.advance(); Ok(Token::Slash) }
                '=' => { self.advance(); Ok(Token::Equals) }
                '"' => {
                    self.advance();
                    let mut s = String::new();
                    while let Some(ch) = self.peek_char() {
                        if ch == '"' { break; }
                        s.push(ch);
                        self.advance();
                    }
                    self.expect_char('"')?;
                    Ok(Token::StringLiteral(s))
                }
                c if Self::is_ident_start(c) => {
                    let ident = self.consume_while(Self::is_ident_char);
                    Ok(Token::Identifier(ident))
                }
                _ => {
                    // Everything else until [ or ] is considered text
                    let txt = self.consume_while(|ch| ch != '[' && ch != ']');
                    Ok(Token::Text(txt))
                }
            }
        } else { Ok(Token::Eof) }
    }

    fn is_ident_start(c: char) -> bool {
        c.is_alphabetic() || c == '_' || c == '-' || c.is_numeric()
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-' || c == '%' || c == '.'
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), String> {
        loop {
            while let Some(c) = self.peek_char() { if c.is_whitespace() { self.advance(); } else { break; } }

            // Line comments
            if self.peek_char() == Some('/') && self.peek_next_char() == Some('/') {
                self.advance(); self.advance();
                while let Some(c) = self.peek_char() { if c == '\n' { break; } self.advance(); }
                continue;
            }

            // Block comments
            if self.peek_char() == Some('/') && self.peek_next_char() == Some('*') {
                self.advance(); self.advance();
                while let Some(c) = self.peek_char() {
                    if c == '*' && self.peek_next_char() == Some('/') { self.advance(); self.advance(); break; }
                    self.advance();
                }
                continue;
            }

            break;
        }
        Ok(())
    }

    fn peek_char(&self) -> Option<char> { self.input[self.pos..].chars().next() }
    fn peek_next_char(&self) -> Option<char> { let mut iter = self.input[self.pos..].chars(); iter.next(); iter.next() }
    fn advance(&mut self) { if let Some(c) = self.peek_char() { self.pos += c.len_utf8(); } }
    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        if self.peek_char() == Some(expected) { self.advance(); Ok(()) } else { Err(format!("Expected '{}'", expected)) }
    }
    fn consume_while<F>(&mut self, mut cond: F) -> String where F: FnMut(char) -> bool {
        let mut result = String::new();
        while let Some(c) = self.peek_char() { if cond(c) { result.push(c); self.advance(); } else { break; } }
        result
    }
}

// =================== PARSER ===================

pub struct FurParser {
    lexer: Lexer,
    current_token: Token,
}

impl FurParser {
    pub fn new(input: &str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let first_token = lexer.next_token()?;
        Ok(Self { lexer, current_token: first_token })
    }

    fn next_token(&mut self) -> Result<(), String> { self.current_token = self.lexer.next_token()?; Ok(()) }
    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.current_token == expected { self.next_token() } else { Err(format!("Expected {:?}, found {:?}", expected, self.current_token)) }
    }

    pub fn parse(&mut self) -> Result<Vec<FurNode>, String> {
        let mut nodes = Vec::new();
        while self.current_token != Token::Eof { nodes.push(self.parse_node()?); }
        Ok(nodes)
    }

    fn parse_node(&mut self) -> Result<FurNode, String> {
        match &self.current_token {
            Token::LBracket => self.parse_element(),
            Token::Text(text) => { let txt = text.clone(); self.next_token()?; Ok(FurNode::Text(txt)) },
            other => Err(format!("Unexpected token when parsing node: {:?}", other)),
        }
    }

    fn parse_element(&mut self) -> Result<FurNode, String> {
    self.expect(Token::LBracket)?;
    let is_closing = if self.current_token == Token::Slash { self.next_token()?; true } else { false };

    let tag_name = match &self.current_token {
        Token::Identifier(name) => { let name = name.clone(); self.next_token()?; name },
        _ => return Err(format!("Expected tag name, found {:?}", self.current_token)),
    };

    if is_closing {
        self.expect(Token::RBracket)?;
        return Err(format!("Unexpected closing tag without matching opening: {}", tag_name));
    }

    let mut attributes = HashMap::new();

    while let Token::Identifier(attr_name) = &self.current_token {
        let key = attr_name.clone();
        self.next_token()?;
        self.expect(Token::Equals)?;

        match &self.current_token {
            Token::StringLiteral(val) | Token::Identifier(val) => {
                // Determine if this key is a "rounding" attribute
                let is_rounding = key.starts_with("rounding");
                // Resolve the value
                let resolved_val = resolve_value(val, is_rounding);
                attributes.insert(key, resolved_val);
                self.next_token()?;
            }
            other => return Err(format!("Expected string literal or identifier for attribute value, found {:?}", other)),
        }
    }

    let self_closing = if self.current_token == Token::Slash {
        self.next_token()?;
        self.expect(Token::RBracket)?;
        true
    } else {
        self.expect(Token::RBracket)?;
        false
    };

    let mut children = Vec::new();
    if !self_closing {
        loop {
            if self.current_token == Token::LBracket {
                let saved_pos = self.lexer.pos;
                let saved_token = self.current_token.clone();
                self.next_token()?;
                if self.current_token == Token::Slash {
                    self.next_token()?;
                    if let Token::Identifier(close_name) = &self.current_token {
                        if *close_name == tag_name {
                            self.next_token()?;
                            self.expect(Token::RBracket)?;
                            break;
                        }
                    }
                }
                self.lexer.pos = saved_pos;
                self.current_token = saved_token;
            }
            children.push(self.parse_node()?);
        }
    }

    let id = attributes.get("id").cloned().unwrap_or_else(|| format!("{}_{}", tag_name, Uuid::new_v4()));

    Ok(FurNode::Element(FurElement {
        tag_name,
        id,
        attributes,
        children,
        self_closing,
    }))
}

}

// =================== VALUE MAPS ===================

pub fn resolve_value(val: &str, is_rounding: bool) -> String {
    let rounding_map: HashMap<&str, f32> = [
        ("none", 0.0), ("xs", 0.12), ("sm", 0.15), ("md", 0.22), ("lg", 0.35),
        ("xl", 0.5), ("2xl", 0.6), ("3xl", 0.75), ("4xl", 0.85), ("full", 1.0)
    ].iter().cloned().collect();

    let general_map: HashMap<&str, &str> = [
        ("none", "0"), ("xs", "4"), ("sm", "8"), ("md", "16"), ("lg", "24"),
        ("xl", "32"), ("2xl", "48"), ("3xl", "64"), ("4xl", "96"),
        ("full", "100%"), ("half", "50%"), ("third", "33.333%"), ("quart", "25%")
    ].iter().cloned().collect();

    if is_rounding {
        if let Some(v) = rounding_map.get(val) { return v.to_string(); }
    } else {
        if let Some(v) = general_map.get(val) { return v.to_string(); }
    }

    val.to_string()
}
