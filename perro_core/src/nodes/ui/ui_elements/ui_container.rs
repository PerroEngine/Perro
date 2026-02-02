use crate::structs::Color;
use crate::nodes::ui::ui_element::BaseUIElement;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIPanel {
    pub base: BaseUIElement,
    pub props: UIPanelProps,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIPanelProps {
    pub background_color: Option<Color>,
    pub corner_radius: CornerRadius,
    pub border_color: Option<Color>,
    pub border_thickness: f32,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

fn default_opacity() -> f32 {
    1.0
}

impl Default for UIPanelProps {
    fn default() -> Self {
        Self {
            background_color: None,
            corner_radius: CornerRadius::default(),
            border_color: None,
            border_thickness: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
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

impl Default for UIPanel {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            props: UIPanelProps::default(),
        }
    }
}
