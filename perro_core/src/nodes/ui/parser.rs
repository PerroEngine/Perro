use std::{collections::HashMap, default};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ast::{FurAnchor, FurElement, FurNode, FurStyle, ValueOrPercent};
use crate::{Color, Transform2D, Vector2};

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
        Self {
            input: input.to_string(),
            pos: 0,
        }
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
                c if c.is_alphabetic() => {
                    let ident = self.consume_while(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_');
                    Ok(Token::Identifier(ident))
                }
                _ => {
                    let txt = self.consume_while(|ch| ch != '[' && ch != ']');
                    Ok(Token::Text(txt))
                }
            }
        } else { Ok(Token::Eof) }
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

        if is_closing { self.expect(Token::RBracket)?; return Err(format!("Unexpected closing tag without matching opening: {}", tag_name)); }

        let mut attributes = HashMap::new();
        let mut style: Option<FurStyle> = None;

        while let Token::Identifier(attr_name) = &self.current_token {
            let key = attr_name.clone(); self.next_token()?; self.expect(Token::Equals)?;
            if let Token::StringLiteral(val) = &self.current_token {
                if key == "style" { style = Some(parse_style_string(val)?); } else { attributes.insert(key, val.clone()); }
                self.next_token()?;
            } else { return Err(format!("Expected string literal for attribute value, found {:?}", self.current_token)); }
        }

        let self_closing = if self.current_token == Token::Slash { self.next_token()?; self.expect(Token::RBracket)?; true } else { self.expect(Token::RBracket)?; false };

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
                            if *close_name == tag_name { self.next_token()?; self.expect(Token::RBracket)?; break; }
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
            style: style.unwrap_or_default(),
        }))
    }
}

// =================== STYLE PARSING ===================

fn parse_style_string(input: &str) -> Result<FurStyle, String> {
    let mut style = FurStyle::default();

    // General size map for margin/padding/scale
    let general_size_map: HashMap<&str, f32> = [
        ("none", 0.0), ("xs", 4.0), ("sm", 8.0), ("md", 16.0), ("lg", 24.0),
        ("xl", 32.0), ("2xl", 48.0), ("3xl", 64.0), ("4xl", 96.0),
    ].iter().cloned().collect();

    // Percentage keyword map
    let percentage_map: HashMap<&str, f32> = [
        ("full", 100.0), ("half", 50.0), ("third", 33.333), ("quart", 25.0),
    ].iter().cloned().collect();

    // Rounding map
    let rounding_map: HashMap<&str, f32> = [
        ("none", 0.0), ("xs", 0.12), ("sm", 0.15), ("md", 0.22), ("lg", 0.35),
        ("xl", 0.5), ("2xl", 0.6), ("3xl", 0.75), ("4xl", 0.85),
        ("full", 1.0)
    ].iter().cloned().collect();

    let parse_value = |val: &str| -> Option<ValueOrPercent> {
        // Check for explicit percentage (e.g., "50%")
        if let Some(p) = val.strip_suffix('%') { 
            p.parse::<f32>().ok().map(ValueOrPercent::Percent) 
        }
        // Check for percentage keywords (full, half, third, quart)
        else if let Some(percent) = percentage_map.get(val) {
            Some(ValueOrPercent::Percent(*percent))
        }
        // Check for numeric values
        else if let Ok(n) = val.parse::<f32>() { 
            Some(ValueOrPercent::Abs(n)) 
        }
        // Check for size keywords (xs, sm, md, etc.)
        else if let Some(n) = general_size_map.get(val) { 
            Some(ValueOrPercent::Abs(*n)) 
        }
        else { None }
    };

    fn parse_abs_value(val: &str, general_map: &HashMap<&str, f32>) -> f32 {
        if let Ok(n) = val.parse::<f32>() { n } else { *general_map.get(val).unwrap_or(&0.0) }
    }

    for token in input.split_whitespace() {
        let (key, value) = token.split_once('=').ok_or_else(|| format!("Invalid style token '{}'", token))?;
        match key {
            "bg" => style.background_color = Some(parse_color_with_opacity(value)?),
            "mod" => style.modulate = Some(parse_color_with_opacity(value)?),

            "m" => if let Some(v) = parse_value(value) { style.margin.top = Some(v); style.margin.right = Some(v); style.margin.bottom = Some(v); style.margin.left = Some(v); },
            "mt" => style.margin.top = parse_value(value),
            "mr" => style.margin.right = parse_value(value),
            "mb" => style.margin.bottom = parse_value(value),
            "ml" => style.margin.left = parse_value(value),

            "p" => if let Some(v) = parse_value(value) { style.padding.top = Some(v); style.padding.right = Some(v); style.padding.bottom = Some(v); style.padding.left = Some(v); },
            "pt" => style.padding.top = parse_value(value),
            "pr" => style.padding.right = parse_value(value),
            "pb" => style.padding.bottom = parse_value(value),
            "pl" => style.padding.left = parse_value(value),

            "tx" => style.translation.x = parse_value(value),
            "ty" => style.translation.y = parse_value(value),

            "sz" => if let Some(v) = parse_value(value) { style.size.x = Some(v); style.size.y = Some(v); },
            "w" | "sz-x" => style.size.x = parse_value(value),
            "h" | "sz-y" => style.size.y = parse_value(value),

            "scl" => if let Some(v) = parse_value(value) { style.transform.scale.x = Some(v); style.transform.scale.y = Some(v); },
            "scl-x" => style.transform.scale.x = parse_value(value),
            "scl-y" => style.transform.scale.y = parse_value(value),

            "rounding" => { let v = parse_abs_value(value, &rounding_map); style.corner_radius.top_left = v; style.corner_radius.top_right = v; style.corner_radius.bottom_left = v; style.corner_radius.bottom_right = v; },
            "rounding-t" => { let v = parse_abs_value(value, &rounding_map); style.corner_radius.top_left = v; style.corner_radius.top_right = v; },
            "rounding-b" => { let v = parse_abs_value(value, &rounding_map); style.corner_radius.bottom_left = v; style.corner_radius.bottom_right = v; },
            "rounding-l" => { let v = parse_abs_value(value, &rounding_map); style.corner_radius.top_left = v; style.corner_radius.bottom_left = v; },
            "rounding-r" => { let v = parse_abs_value(value, &rounding_map); style.corner_radius.top_right = v; style.corner_radius.bottom_right = v; },
            "rounding-tl" => style.corner_radius.top_left = parse_abs_value(value, &rounding_map),
            "rounding-tr" => style.corner_radius.top_right = parse_abs_value(value, &rounding_map),
            "rounding-bl" => style.corner_radius.bottom_left = parse_abs_value(value, &rounding_map),
            "rounding-br" => style.corner_radius.bottom_right = parse_abs_value(value, &rounding_map),

            "border" => style.border = parse_abs_value(value, &general_size_map),
            "border-color" | "border-c" => style.border_color = Some(parse_color_with_opacity(value)?),

            "anchor" => style.anchor = match value {
                "c" => FurAnchor::Center, "t" => FurAnchor::Top, "b" => FurAnchor::Bottom,
                "l" => FurAnchor::Left, "r" => FurAnchor::Right,
                "tl" => FurAnchor::TopLeft, "tr" => FurAnchor::TopRight,
                "bl" => FurAnchor::BottomLeft, "br" => FurAnchor::BottomRight,
                _ => return Err(format!("Invalid anchor value '{}'", value))
            },

            _ => {}
        }
    }

    Ok(style)
}

fn parse_color_with_opacity(value: &str) -> Result<Color, String> {
    let mut parts = value.splitn(2, '/');
    let base = parts.next().unwrap();
    let opacity_part = parts.next();

    let mut color = if base.starts_with('#') { Color::from_hex(base).map_err(|e| format!("Invalid hex color '{}': {}", base, e))? }
    else { Color::from_preset(base).ok_or_else(|| format!("Unknown preset color '{}'", base))? };

    if let Some(opacity_str) = opacity_part {
        let opacity_percent = opacity_str.parse::<u8>().map_err(|_| format!("Invalid opacity '{}'", opacity_str))?;
        if opacity_percent > 100 { return Err(format!("Opacity '{}' out of range 0-100", opacity_percent)); }
        color.a = (opacity_percent as f32 / 100.0 * 255.0).round() as u8;
    }

    Ok(color)
}