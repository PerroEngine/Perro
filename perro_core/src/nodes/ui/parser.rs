use std::{borrow::Cow, collections::HashMap};
use crate::uid32::{Uid32, UIElementID};

use crate::fur_ast::{FurElement, FurNode};

// =================== TOKENS ===================

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

// =================== LEXER ===================

pub struct Lexer<'a> {
    pub input: &'a str,
    pub pos: usize,
    len: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            len: input.len(),
        }
    }

    pub fn next_token(&mut self) -> Result<Token<'a>, String> {
        self.skip_whitespace_and_comments()?;
        if self.pos >= self.len {
            return Ok(Token::Eof);
        }

        let c = self.peek_char().unwrap();
        match c {
            '[' => {
                self.advance();
                Ok(Token::LBracket)
            }
            ']' => {
                self.advance();
                Ok(Token::RBracket)
            }
            '/' => {
                self.advance();
                Ok(Token::Slash)
            }
            '=' => {
                self.advance();
                Ok(Token::Equals)
            }
            '"' => {
                self.advance();
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if ch == '"' {
                        break;
                    }
                    self.advance();
                }
                let s = &self.input[start..self.pos];
                self.expect_char('"')?;
                Ok(Token::StringLiteral(s))
            }
            c if Self::is_ident_start(c) => {
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if !Self::is_ident_char(ch) {
                        break;
                    }
                    self.advance();
                }
                Ok(Token::Identifier(&self.input[start..self.pos]))
            }
            _ => {
                let start = self.pos;
                while let Some(ch) = self.peek_char() {
                    if ch == '[' || ch == ']' {
                        break;
                    }
                    self.advance();
                }
                Ok(Token::Text(&self.input[start..self.pos]))
            }
        }
    }

    fn is_ident_start(c: char) -> bool {
        c.is_alphabetic() || c == '_' || c == '-' || c == '#' || c.is_numeric()
    }

    fn is_ident_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '-' || c == '%' || c == '.' || c == ',' || c == '#'
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), String> {
        loop {
            while let Some(c) = self.peek_char() {
                if c.is_whitespace() {
                    self.advance();
                } else {
                    break;
                }
            }

            // Single-line comments //
            if self.peek_char() == Some('/') && self.peek_next_char() == Some('/') {
                self.advance();
                self.advance();
                while let Some(c) = self.peek_char() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
                continue;
            }

            // Multi-line comments /* */
            if self.peek_char() == Some('/') && self.peek_next_char() == Some('*') {
                self.advance();
                self.advance();
                while let Some(c) = self.peek_char() {
                    if c == '*' && self.peek_next_char() == Some('/') {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                continue;
            }

            break;
        }
        Ok(())
    }

    pub fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut iter = self.input[self.pos..].chars();
        iter.next();
        iter.next()
    }

    pub fn advance(&mut self) {
        if let Some(c) = self.peek_char() {
            self.pos += c.len_utf8();
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), String> {
        if self.peek_char() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(format!("Expected '{}'", expected))
        }
    }
}

// =================== PARSER ===================

pub struct FurParser<'a> {
    lexer: Lexer<'a>,
    current_token: Token<'a>,
    element_stack: Vec<String>,
}

impl<'a> FurParser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let first_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token: first_token,
            element_stack: Vec::new(),
        })
    }

    fn next_token(&mut self) -> Result<(), String> {
        self.current_token = self.lexer.next_token()?;
        Ok(())
    }

    pub fn parse(&mut self) -> Result<Vec<FurNode>, String> {
        let mut nodes = Vec::new();
        while self.current_token != Token::Eof {
            nodes.push(self.parse_node()?);
        }
        Ok(nodes)
    }

    fn parse_node(&mut self) -> Result<FurNode, String> {
        match &self.current_token {
            Token::LBracket => self.parse_element(),
            Token::Text(txt) => {
                let t = txt.to_string();
                self.next_token()?;
                Ok(FurNode::Text(Cow::Owned(t)))
            }
            // Identifiers between element tags should be treated as text content
            // (e.g., "Hello" in [Button]Hello[/Button])
            Token::Identifier(ident) => {
                let t = ident.to_string();
                self.next_token()?;
                Ok(FurNode::Text(Cow::Owned(t)))
            }
            other => Err(format!("Unexpected token when parsing node: {:?}", other)),
        }
    }

    fn parse_element(&mut self) -> Result<FurNode, String> {
        if self.current_token != Token::LBracket {
            return Err(format!("Expected LBracket, found {:?}", self.current_token));
        }
        self.next_token()?; // consume '['

        let is_closing = if self.current_token == Token::Slash {
            self.next_token()?;
            true
        } else {
            false
        };

        let tag_name = match &self.current_token {
            Token::Identifier(name) => {
                let n = *name;
                self.next_token()?;
                n
            }
            _ => return Err(format!("Expected tag name, found {:?}", self.current_token)),
        };

        if is_closing {
            if self.current_token != Token::RBracket {
                return Err(format!(
                    "Expected RBracket after closing tag, found {:?}",
                    self.current_token
                ));
            }
            self.next_token()?;
            return Err(format!(
                "Unexpected closing tag without matching opening: {}",
                tag_name
            ));
        }

        self.element_stack.push(tag_name.to_string());

        // ---- collect attributes
        let mut attributes: HashMap<Cow<'static, str>, Cow<'static, str>> = HashMap::new();
        while let Token::Identifier(attr_name) = &self.current_token {
            let key = *attr_name;
            self.next_token()?;
            if self.current_token != Token::Equals {
                return Err(format!("Expected '=', found {:?}", self.current_token));
            }
            self.next_token()?;
            match &self.current_token {
                Token::StringLiteral(val) | Token::Identifier(val) => {
                    let resolved_val = resolve_value(val, key.starts_with("rounding"));
                    attributes.insert(Cow::Owned(key.to_string()), Cow::Owned(resolved_val));
                    self.next_token()?;
                }
                other => {
                    return Err(format!(
                        "Expected string literal or identifier for attribute value, found {:?}",
                        other
                    ));
                }
            }
        }

        // ---- self-closing?
        let self_closing = if self.current_token == Token::Slash {
            self.next_token()?;
            if self.current_token != Token::RBracket {
                return Err(format!(
                    "Expected RBracket after self-closing '/', found {:?}",
                    self.current_token
                ));
            }
            self.next_token()?;
            true
        } else {
            if self.current_token != Token::RBracket {
                return Err(format!("Expected RBracket, found {:?}", self.current_token));
            }
            // DON'T consume ']' yet for Text elements!
            if tag_name != "Text" {
                self.next_token()?; // consume ']'
            }
            false
        };

        // ---- RAW TEXT HANDLING - COMPLETELY BYPASS TOKENIZATION
        if tag_name == "Text" && !self_closing {
            // For Text elements, extract_raw_text_content will handle consuming ']' and extracting content
            let content = self.extract_raw_text_content(tag_name)?;

            self.element_stack.pop();

            return Ok(FurNode::Element(FurElement {
                tag_name: "Text".into(),
                id: attributes
                    .get("id")
                    .cloned()
                    .unwrap_or_else(|| format!("{}_{}", tag_name, UIElementID::new()).into())
                    .into(),
                attributes,
                children: vec![FurNode::Text(content.into())],
                self_closing: false,
            }));
        }

        // ---- normal children (for non-Text elements)
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
                                if self.current_token != Token::RBracket {
                                    return Err(format!(
                                        "Expected RBracket after closing tag, found {:?}",
                                        self.current_token
                                    ));
                                }
                                self.next_token()?;
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

        self.element_stack.pop();

        let id = attributes
            .get("id")
            .cloned()
            .unwrap_or_else(|| format!("{}_{}", tag_name, UIElementID::new()).into());

        Ok(FurNode::Element(FurElement {
            tag_name: Cow::Owned(tag_name.to_string()),
            id,
            attributes,
            children,
            self_closing,
        }))
    }

    // NEW METHOD: Extract text content without any tokenization
    fn extract_raw_text_content(&mut self, tag_name: &str) -> Result<String, String> {
        // When current_token is RBracket, the lexer has already advanced past ']'
        // So lexer.pos is already pointing at the first character of content
        // Use it directly as content_start
        let input = self.lexer.input;
        let content_start = self.lexer.pos;
        let closing_tag = format!("[/{}]", tag_name);

        // Find the closing tag position in the remaining input
        let remaining_input = &input[content_start..];

        if let Some(closing_pos) = remaining_input.find(&closing_tag) {
            // Extract the EXACT raw content - no processing yet
            let raw_content = &remaining_input[..closing_pos];

            // Process the content to remove structural whitespace
            let processed_content = process_text_content(raw_content);

            // Update lexer position past the closing tag
            self.lexer.pos = content_start + closing_pos + closing_tag.len();

            // Update current token
            self.current_token = if self.lexer.pos >= self.lexer.len {
                Token::Eof
            } else {
                self.lexer.next_token()?
            };

            Ok(processed_content)
        } else {
            Err(format!("Missing closing tag [/{}]", tag_name))
        }
    }
}

// =================== TEXT PROCESSING ===================

fn process_text_content(raw_content: &str) -> String {
    // If it's just whitespace, return empty
    if raw_content.trim().is_empty() {
        return String::new();
    }

    // Split into lines
    let lines: Vec<&str> = raw_content.split('\n').collect();

    // Find first and last non-empty lines
    let first_content = lines
        .iter()
        .position(|line| !line.trim().is_empty())
        .unwrap_or(0);

    let last_content = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(lines.len());

    if first_content >= last_content {
        return String::new();
    }

    // Get the content lines
    let content_lines = &lines[first_content..last_content];

    // If only one line, just trim and return
    if content_lines.len() == 1 {
        return content_lines[0].trim().to_string();
    }

    // For multi-line, find common indentation
    let min_indent = content_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    // Remove common indentation
    let processed_lines: Vec<String> = content_lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if line.len() >= min_indent {
                line[min_indent..].to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    processed_lines.join("\n")
}

// =================== VALUE RESOLUTION ===================

use once_cell::sync::Lazy;

static ROUNDING_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("none", "0.0"),
        ("xs", "0.12"),
        ("sm", "0.15"),
        ("md", "0.22"),
        ("lg", "0.35"),
        ("xl", "0.5"),
        ("2xl", "0.6"),
        ("3xl", "0.75"),
        ("4xl", "0.85"),
        ("half", "0.5"),
        ("third", "0.333"),
        ("quart", "0.25"),
        ("3q", "0.75"),
        ("full", "1.0"),
    ]
    .iter()
    .cloned()
    .collect()
});

static GENERAL_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        ("none", "0"),
        ("xs", "4"),
        ("sm", "8"),
        ("md", "16"),
        ("lg", "24"),
        ("xl", "32"),
        ("2xl", "48"),
        ("3xl", "64"),
        ("4xl", "96"),

    ]
    .iter()
    .cloned()
    .collect()
});

pub fn resolve_value(val: &str, is_rounding: bool) -> String {
    val.split(',')
        .map(|p| p.trim())
        .map(|part| {
            if is_rounding && part.ends_with('%') {
                // remove the '%' and parse as f32
                let number_str = &part[..part.len() - 1];
                match number_str.parse::<f32>() {
                    Ok(num) => return (num / 100.0).to_string(),
                    Err(_) => return part.to_string(), // fallback if parse fails
                }
            }

            // regular mapping
            let map = if is_rounding {
                &*ROUNDING_MAP
            } else {
                &*GENERAL_MAP
            };
            map.get(part).unwrap_or(&part).to_string()
        })
        .collect::<Vec<_>>()
        .join(",")
}
