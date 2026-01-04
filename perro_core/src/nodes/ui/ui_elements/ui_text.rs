use serde::{Deserialize, Serialize};

use crate::{impl_ui_element, structs::Color, ui_element::BaseUIElement};

/// How text flows relative to its anchor point
/// - Start: text starts at the anchor point (flows right/down)
/// - Center: text is centered on the anchor point
/// - End: text ends at the anchor point (flows left/up)
#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
pub enum TextFlow {
    Start,
    Center,
    End,
}

impl Default for TextFlow {
    fn default() -> Self {
        TextFlow::Center
    }
}

// Keep old TextAlignment for backward compatibility during transition
#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Top,
    Bottom,
}

impl Default for TextAlignment {
    fn default() -> Self {
        TextAlignment::Center
    }
}

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
    /// How text flows relative to its anchor point (start/center/end)
    /// This applies to both horizontal and vertical alignment
    #[serde(default)]
    pub align: TextFlow,
    // Keep old fields for backward compatibility, but they're deprecated
    #[serde(default)]
    #[deprecated(note = "Use align instead")]
    pub align_h: TextAlignment,
    #[serde(default)]
    #[deprecated(note = "Use align instead")]
    pub align_v: TextAlignment,
}

impl Default for TextProps {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_size: 12.0,
            color: Color::default(),
            align: TextFlow::Center,
            #[allow(deprecated)]
            align_h: TextAlignment::Center,
            #[allow(deprecated)]
            align_v: TextAlignment::Center,
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

    /// Get the text flow alignment (how text flows relative to anchor)
    pub fn get_align(&self) -> TextFlow {
        self.props.align
    }

    /// Set the text flow alignment
    pub fn set_align(&mut self, align: TextFlow) {
        self.props.align = align;
    }

    /// Get the horizontal alignment (deprecated - use get_align instead)
    #[deprecated(note = "Use get_align instead")]
    #[allow(deprecated)]
    pub fn get_align_h(&self) -> TextAlignment {
        self.props.align_h
    }

    /// Set the horizontal alignment (deprecated - use set_align instead)
    #[deprecated(note = "Use set_align instead")]
    #[allow(deprecated)]
    pub fn set_align_h(&mut self, align: TextAlignment) {
        self.props.align_h = align;
    }

    /// Get the vertical alignment (deprecated - use get_align instead)
    #[deprecated(note = "Use get_align instead")]
    #[allow(deprecated)]
    pub fn get_align_v(&self) -> TextAlignment {
        self.props.align_v
    }

    /// Set the vertical alignment (deprecated - use set_align instead)
    #[deprecated(note = "Use set_align instead")]
    #[allow(deprecated)]
    pub fn set_align_v(&mut self, align: TextAlignment) {
        self.props.align_v = align;
    }
}

