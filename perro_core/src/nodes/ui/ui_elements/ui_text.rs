use serde::{Deserialize, Serialize};

use crate::{impl_ui_element, structs::Color, ui_element::BaseUIElement};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UIText {
    pub base: BaseUIElement,
    pub props: TextProps,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextProps {
    pub content: String,
    pub font_size: f32,
    pub color: Color,
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_size: 12.0,
            color: Color::default(),
        }
    }
}

impl_ui_element!(UIText);

impl UIText {
    /// Get the text content
    pub fn get_content(&self) -> &str {
        &self.props.content
    }

    /// Set the text content
    pub fn set_content(&mut self, content: &str) {
        self.props.content = content.to_string();
    }

    /// Get the font size
    pub fn get_font_size(&self) -> f32 {
        self.props.font_size
    }

    /// Set the font size
    pub fn set_font_size(&mut self, size: f32) {
        self.props.font_size = size;
    }

    /// Get the text color
    pub fn get_color(&self) -> &Color {
        &self.props.color
    }

    /// Set the text color
    pub fn set_color(&mut self, color: Color) {
        self.props.color = color;
    }
}
