use serde::{Deserialize, Serialize};

use crate::{impl_ui_element, ui_element::BaseUIElement, Vector2};

/// Shared layout data, not a real UIElement
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Container {
    pub mode: ContainerMode,                  // Horizontal, Vertical, Grid
    pub gap: Vector2,                         // spacing between children
    pub alignment: Alignment,                 // start, center, end
    pub distribution: DistributionMode,       // pack or even
}

impl Default for Container {
    fn default() -> Self {
        Self {
            mode: ContainerMode::Horizontal,         // horizontal by default
            gap: Vector2::new(0.0, 0.0),             // no gap by default
            alignment: Alignment::Center,            // default alignment is Center
            distribution: DistributionMode::Pack,    // default distribution
        }
    }
}

/// Defines which layout a Container is doing
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ContainerMode {
    Horizontal,
    Vertical,
    Grid,
}

/// How children are aligned inside container
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Alignment {
    Start,
    Center,
    End,
}

/// How children are distributed inside container
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DistributionMode {
    Pack,
    Even,
}

/// Horizontal/Vertical Box container UI element
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoxContainer {
    pub base: BaseUIElement,
    pub container: Container,  
}

impl Default for BoxContainer {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            container: Container {
                mode: ContainerMode::Horizontal,    // horizontal by default
                ..Default::default()
            },
        }
    }
}

/// Grid container UI element
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GridContainer {
    pub base: BaseUIElement,
    pub container: Container,  
    pub cols: usize,
}

impl Default for GridContainer {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            container: Container {
                mode: ContainerMode::Grid,          // grid by default
                ..Default::default()
            },
            cols: 1,                                // default to 1 column
        }
    }
}


impl_ui_element!(BoxContainer);
impl_ui_element!(GridContainer);

