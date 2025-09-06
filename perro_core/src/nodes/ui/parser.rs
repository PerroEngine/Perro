use std::collections::HashMap;
use uuid::Uuid;
use crate::ast::{FurElement, FurNode};

// =================== LEXER ===================

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    LBracket,
    RBracket,
    Slash,
    Equals,
    Identifier(&'a str),
    StringLiteral(&'a str),
    Text(&'a str),
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    len: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0, len: input.len() }
    }

    pub fn next_token(&mut self) -> Result<Token<'a>, String> {
        self.skip_whitespace_and_comments()?;
        if self.pos >= self.len { return Ok(Token::Eof); }

        let c = self.peek_char().unwrap();
        match c {
            '[' => { self.advance(); Ok(Token::LBracket) }
            ']' => { self.advance(); Ok(Token::RBracket) }
            '/' => { self.advance(); Ok(Token::Slash) }
            '=' => { self.advance(); Ok(Token::Equals) }
            '"' => {
                self.advance();
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if ch == '"' { break; }
                    self.advance();
                }
                let s = &self.input[start..self.pos];
                self.expect_char('"')?;
                Ok(Token::StringLiteral(s))
            }
            c if Self::is_ident_start(c) => {
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if !Self::is_ident_char(ch) { break; }
                    self.advance();
                }
                Ok(Token::Identifier(&self.input[start..self.pos]))
            }
            _ => {
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if ch == '[' || ch == ']' { break; }
                    self.advance();
                }
                Ok(Token::Text(&self.input[start..self.pos]))
            }
        }
    }

    fn is_ident_start(c: char) -> bool { c.is_alphabetic() || c == '_' || c == '-' || c == '#' || c.is_numeric() }
    fn is_ident_char(c: char) -> bool { c.is_alphanumeric() || c == '_' || c == '-' || c == '%' || c == '.' || c == ',' || c == '#' }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), String> {
        loop {
            while let Some(c) = self.peek_char() { if c.is_whitespace() { self.advance(); } else { break; } }

            if self.peek_char() == Some('/') && self.peek_next_char() == Some('/') {
                self.advance(); self.advance();
                while let Some(c) = self.peek_char() { if c == '\n' { break; } self.advance(); }
                continue;
            }

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
}

// =================== PARSER ===================

pub struct FurParser<'a> {
    lexer: Lexer<'a>,
    current_token: Token<'a>,
}

impl<'a> FurParser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let first_token = lexer.next_token()?;
        Ok(Self { lexer, current_token: first_token })
    }

    fn next_token(&mut self) -> Result<(), String> { self.current_token = self.lexer.next_token()?; Ok(()) }
    fn expect(&mut self, expected: Token<'a>) -> Result<(), String> {
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
            Token::Text(txt) => { let t = *txt; self.next_token()?; Ok(FurNode::Text(t.to_string())) },
            other => Err(format!("Unexpected token when parsing node: {:?}", other)),
        }
    }

    fn parse_element(&mut self) -> Result<FurNode, String> {
        self.expect(Token::LBracket)?;
        let is_closing = if self.current_token == Token::Slash { self.next_token()?; true } else { false };

        let tag_name = match &self.current_token {
            Token::Identifier(name) => { let n = *name; self.next_token()?; n },
            _ => return Err(format!("Expected tag name, found {:?}", self.current_token)),
        };

        if is_closing {
            self.expect(Token::RBracket)?;
            return Err(format!("Unexpected closing tag without matching opening: {}", tag_name));
        }

        let mut attributes = HashMap::new();

        while let Token::Identifier(attr_name) = &self.current_token {
            let key = *attr_name;
            self.next_token()?;
            self.expect(Token::Equals)?;

            match &self.current_token {
                Token::StringLiteral(val) | Token::Identifier(val) => {
                    let resolved_val = resolve_value(val, key.starts_with("rounding"));
                    attributes.insert(key.to_string(), resolved_val);
                    self.next_token()?;
                }
                other => return Err(format!("Expected string literal or identifier for attribute value, found {:?}", other)),
            }
        }

        let self_closing = if self.current_token == Token::Slash {
            self.next_token()?; self.expect(Token::RBracket)?; true
        } else { self.expect(Token::RBracket)?; false };

        let mut children = Vec::new();
        if !self_closing {
            loop {
                if let Token::LBracket = self.current_token {
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
            tag_name: tag_name.to_string(),
            id,
            attributes,
            children,
            self_closing,
        }))
    }
}

// =================== VALUE RESOLUTION ===================

use once_cell::sync::Lazy;

static ROUNDING_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("none", "0.0"), ("xs", "0.12"), ("sm", "0.15"), ("md", "0.22"), ("lg", "0.35"),
        ("xl", "0.5"), ("2xl", "0.6"), ("3xl", "0.75"), ("4xl", "0.85"), ("full", "1.0")
    ].iter().cloned().collect()
});

static GENERAL_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("none", "0"), ("xs", "4"), ("sm", "8"), ("md", "16"), ("lg", "24"),
        ("xl", "32"), ("2xl", "48"), ("3xl", "64"), ("4xl", "96"),
        ("full", "100%"), ("half", "50%"), ("third", "33.333%"), ("quart", "25%"), ("3q", "75%")
    ].iter().cloned().collect()
});

pub fn resolve_value(val: &str, is_rounding: bool) -> String {
    val.split(',')
        .map(|p| p.trim())
        .map(|part| {
            let map = if is_rounding { &*ROUNDING_MAP } else { &*GENERAL_MAP };
            map.get(part).unwrap_or(&part).to_string()
        })
        .collect::<Vec<_>>()
        .join(",")
}
