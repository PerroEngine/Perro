use serde::{Serialize, Deserialize};

use crate::ui_element::BaseUIElement;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UIDirection {
    Horizontal,
    Vertical,
}

/// Example enum for alignment along the layout axis
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UIAlignment {
    Start,
    Center,
    End,
}


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UIContainer {
    pub base: BaseUIElement,
    
    pub direction: UIDirection,

    pub spacing: f32,

    pub alignment: UIAlignment,
}

impl Default for UIContainer {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            direction: UIDirection::Vertical,
            spacing: 0.0,
            alignment: UIAlignment::Center,
        }
    }
}

