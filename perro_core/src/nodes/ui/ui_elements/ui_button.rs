//! UI Button — clickable panel (compositional).

use serde::{Deserialize, Serialize};

use crate::nodes::ui::ui_element::BaseUIElement;
use crate::nodes::ui::ui_elements::ui_container::UIPanelProps;
use crate::structs::Color;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UIButton {
    pub base: BaseUIElement,
    pub props: UIPanelProps,
    pub hover_color: Option<Color>,
    pub pressed_color: Option<Color>,
    pub hover_border_color: Option<Color>,
    pub pressed_border_color: Option<Color>,
}
