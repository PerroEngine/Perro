use std::ops::{Deref, DerefMut};

use serde::{Serialize, Deserialize};

use crate::{impl_ui_element, ui_element::BaseUIElement, Color};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIPanel {
    pub base: BaseUIElement,
    pub style: UIStyle,

}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CornerRadius {
    #[serde(default)]
    pub top_left: f32,
    #[serde(default)]
    pub top_right: f32,
    #[serde(default)]
    pub bottom_right: f32,
    #[serde(default)]
    pub bottom_left: f32,
}

impl CornerRadius {
    pub fn uniform(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}



#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIStyle {
    /// Background color as RGBA (or use your own Color type)
    #[serde(default)]
    pub background_color: Option<Color>,

    #[serde(default)]
    pub modulate: Option<Color>,

    /// Corner radius for rounded edges
    #[serde(default)]
    pub corner_radius: CornerRadius,
    
    /// Optional border properties (color, thickness)
    #[serde(default)]
    pub border_color: Option<Color>,
    
    #[serde(default)]
    pub border_thickness: f32,
}

impl Default for UIPanel {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            style: UIStyle::default(),
        }
    }
}


impl Default for UIStyle {
    fn default() -> Self {
        Self {
            background_color: Some(Color { r: 255, g: 255, b: 255, a: 255 }), // white opaque
            modulate: Some(Color { r: 255, g: 255, b: 255, a: 255 }), // white opaque
            corner_radius: CornerRadius::uniform(5.0),
            border_color: None,
            border_thickness: 0.0,
        }
    }
}


impl Deref for UIPanel {
    type Target = BaseUIElement;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UIPanel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl_ui_element!(UIPanel);
