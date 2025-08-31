use std::{collections::HashMap, default};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ast::{FurAnchor, FurElement, FurNode, FurStyle, ValueOrPercent};
use crate::{Color, Transform2D, Vector2};

// =================== PARSER ===================

use crate::lexer::{Lexer, Token};

pub struct FurParser {
    lexer: Lexer,
    current_token: Token,
}

impl FurParser {
    pub fn new(input: &str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let first_token = lexer.next_token()?;
        Ok(Self {
            lexer,
            current_token: first_token,
        })
    }

    fn next_token(&mut self) -> Result<(), String> {
        self.current_token = self.lexer.next_token()?;
        Ok(())
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.current_token == expected {
            self.next_token()
        } else {
            Err(format!("Expected {:?}, found {:?}", expected, self.current_token))
        }
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
            Token::Text(text) => {
                let txt = text.clone();
                self.next_token()?;
                Ok(FurNode::Text(txt))
            }
            other => Err(format!("Unexpected token when parsing node: {:?}", other)),
        }
    }

    fn parse_element(&mut self) -> Result<FurNode, String> {
        self.expect(Token::LBracket)?;

        let is_closing = if self.current_token == Token::Slash {
            self.next_token()?;
            true
        } else {
            false
        };

        let tag_name = match &self.current_token {
            Token::Identifier(name) => {
                let name = name.clone();
                self.next_token()?;
                name
            }
            _ => return Err(format!("Expected tag name, found {:?}", self.current_token)),
        };

        if is_closing {
            self.expect(Token::RBracket)?;
            return Err(format!("Unexpected closing tag without matching opening: {}", tag_name));
        }

        let mut attributes = HashMap::new();
        let mut style: Option<FurStyle> = None;

        while let Token::Identifier(attr_name) = &self.current_token {
            let key = attr_name.clone();
            self.next_token()?;
            self.expect(Token::Equals)?;

            if let Token::StringLiteral(val) = &self.current_token {
                if key == "style" {
                    style = Some(parse_style_string(val)?);
                } else {
                    attributes.insert(key, val.clone());
                }
                self.next_token()?;
            } else {
                return Err(format!("Expected string literal for attribute value, found {:?}", self.current_token));
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
            style: style.unwrap_or_default(),
        }))
    }
}

// =================== STYLE PARSING ===================

fn parse_style_string(input: &str) -> Result<FurStyle, String> {
    let mut style = FurStyle::default();

    // Map the keywords to numeric values
    let size_map: HashMap<&str, f32> = [
        ("xs", 4.0),
        ("sm", 8.0),
        ("md", 12.0),
        ("lg", 16.0),
        ("xl", 24.0),
        ("2xl", 32.0),
        ("3xl", 48.0),
        ("4xl", 64.0),
        ("full", 9999.9),
    ]
    .iter()
    .cloned()
    .collect();

    let parse_value = |val: &str| -> Option<ValueOrPercent> {
        if let Some(percent_str) = val.strip_suffix('%') {
            percent_str.parse::<f32>().ok().map(ValueOrPercent::Percent)
        } else if let Ok(num) = val.parse::<f32>() {
            Some(ValueOrPercent::Abs(num))
        } else if let Some(num) = size_map.get(val) {
            Some(ValueOrPercent::Abs(*num))
        } else {
            None
        }
    };

    fn parse_abs_value(val: &str, size_map: &HashMap<&str, f32>) -> f32 {
        if let Ok(num) = val.parse::<f32>() {
            num
        } else if let Some(v) = size_map.get(val) {
            *v
        } else {
            0.0
        }
    }

    for token in input.split_whitespace() {
        let (key, value) = token.split_once('=').ok_or_else(|| format!("Invalid style token '{}', expected key=value", token))?;
        match key {
            "bg" => style.background_color = Some(parse_color_with_opacity(value)?),
            "mod" => style.modulate = Some(parse_color_with_opacity(value)?),

            "m" => {
                if let Some(v) = parse_value(value) {
                    style.margin.top = Some(v);
                    style.margin.right = Some(v);
                    style.margin.bottom = Some(v);
                    style.margin.left = Some(v);
                }
            }
            "mt" => style.margin.top = parse_value(value),
            "mr" => style.margin.right = parse_value(value),
            "mb" => style.margin.bottom = parse_value(value),
            "ml" => style.margin.left = parse_value(value),

            "p" => {
                if let Some(v) = parse_value(value) {
                    style.padding.top = Some(v);
                    style.padding.right = Some(v);
                    style.padding.bottom = Some(v);
                    style.padding.left = Some(v);
                }
            }
            "pt" => style.padding.top = parse_value(value),
            "pr" => style.padding.right = parse_value(value),
            "pb" => style.padding.bottom = parse_value(value),
            "pl" => style.padding.left = parse_value(value),

            "tx" => style.translation.x = parse_value(value),
            "ty" => style.translation.y = parse_value(value),

            "sz" => {
                if let Some(v) = parse_value(value) {
                    style.size.x = Some(v);
                    style.size.y = Some(v);
                }
            }
            "w" | "sz-x" => style.size.x = parse_value(value),
            "h" | "sz-y" => style.size.y = parse_value(value),

            "scl" => {
                if let Some(v) = parse_value(value) {
                    style.transform.scale.x = Some(v);
                    style.transform.scale.y = Some(v);
                }
            }
            "scl-x" => style.transform.scale.x = parse_value(value),
            "scl-y" => style.transform.scale.y = parse_value(value),

            "rounding" => {
                let v = parse_abs_value(value, &size_map);
                style.corner_radius.top_left = v;
                style.corner_radius.top_right = v;
                style.corner_radius.bottom_left = v;
                style.corner_radius.bottom_right = v;
            },
            "rounding-t" => {
                let v = parse_abs_value(value, &size_map);
                style.corner_radius.top_left = v;
                style.corner_radius.top_right = v;
            },
            "rounding-b" => {
                let v = parse_abs_value(value, &size_map);
                style.corner_radius.bottom_left = v;
                style.corner_radius.bottom_right = v;
            },
            "rounding-l" => {
                let v = parse_abs_value(value, &size_map);
                style.corner_radius.top_left = v;
                style.corner_radius.bottom_left = v;
            },
            "rounding-r" => {
                let v = parse_abs_value(value, &size_map);
                style.corner_radius.top_right = v;
                style.corner_radius.bottom_right = v;
            },
            "rounding-tl" => style.corner_radius.top_left = parse_abs_value(value, &size_map),
            "rounding-tr" => style.corner_radius.top_right = parse_abs_value(value, &size_map),
            "rounding-bl" => style.corner_radius.bottom_left = parse_abs_value(value, &size_map),
            "rounding-br" => style.corner_radius.bottom_right = parse_abs_value(value, &size_map),
            "border" => style.border = parse_abs_value(value, &size_map),

            "border-color" | "border-c" => style.border_color = Some(parse_color_with_opacity(value)?),

            "anchor" => {
                style.anchor = match value {
                    "c" => FurAnchor::Center,
                    "t" => FurAnchor::Top,
                    "b" => FurAnchor::Bottom,
                    "l" => FurAnchor::Left,
                    "r" => FurAnchor::Right,
                    "tl" => FurAnchor::TopLeft,
                    "tr" => FurAnchor::TopRight,
                    "bl" => FurAnchor::BottomLeft,
                    "br" => FurAnchor::BottomRight,
                    _ => return Err(format!("Invalid anchor value '{}'", value)),
                };
            }

            _ => {}
        }
    }

    Ok(style)
}

fn parse_color_with_opacity(value: &str) -> Result<Color, String> {
    let mut parts = value.splitn(2, '/');
    let base = parts.next().unwrap();
    let opacity_part = parts.next();

    let mut color = if base.starts_with('#') {
        Color::from_hex(base).map_err(|e| format!("Invalid hex color '{}': {}", base, e))?
    } else {
        Color::from_preset(base).ok_or_else(|| format!("Unknown preset color '{}'", base))?
    };

    if let Some(opacity_str) = opacity_part {
        let opacity_percent = opacity_str.parse::<u8>().map_err(|_| format!("Invalid opacity value '{}', must be 0-100", opacity_str))?;
        if opacity_percent > 100 { return Err(format!("Opacity '{}' out of range 0-100", opacity_percent)); }
        let alpha = (opacity_percent as f32 / 100.0 * 255.0).round() as u8;
        color.a = alpha;
    }

    Ok(color)
}
