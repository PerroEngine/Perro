use serde::{Deserialize, Serialize};

use crate::{impl_ui_element, structs2d::Color, ui_element::BaseUIElement};

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
