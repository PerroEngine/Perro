use serde::{Serialize, Deserialize};
use std::ops::{Deref, DerefMut};

use crate::{impl_ui_element, ui_element::BaseUIElement, Color};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIPanel {
    pub base: BaseUIElement,
    pub props: UIPanelProps,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UIPanelProps {
    pub background_color: Option<Color>,
    pub corner_radius: CornerRadius,
    pub border_color: Option<Color>,
    pub border_thickness: f32,
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