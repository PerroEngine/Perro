//! UI Button — clickable element (egui Button).

use serde::{Deserialize, Serialize};

use crate::nodes::ui::ui_element::BaseUIElement;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UIButton {
    pub base: BaseUIElement,
    /// Button label (defaults to base.name if not set)
    #[serde(default)]
    pub label: String,
}
