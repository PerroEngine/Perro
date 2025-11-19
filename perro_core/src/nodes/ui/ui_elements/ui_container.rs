use crate::structs::Color;
use crate::structs2d::Vector2;
use crate::{impl_ui_element, ui_element::BaseUIElement};
use serde::{Deserialize, Serialize};
/// =========================
/// 1. Placeholder container
/// =========================

/// Just a generic holder — no layout logic, no visuals
#[derive(Serialize, Deserialize, Clone, Debug, Default)]

pub struct BoxContainer {
    pub base: BaseUIElement,
}

impl_ui_element!(BoxContainer);

/// =========================
/// 2. Layout containers
/// =========================

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Container {
    pub mode: ContainerMode,            // Horizontal, Vertical, Grid
    pub gap: Vector2,                   // spacing between children
    pub distribution: DistributionMode, // pack or even spacing
}

impl Default for Container {
    fn default() -> Self {
        Self {
            mode: ContainerMode::Horizontal,
            gap: Vector2::new(0.0, 0.0),
            distribution: DistributionMode::Pack,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Debug, Copy)]
pub enum ContainerMode {
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum DistributionMode {
    Pack,
    Even,
}

/// Horizontal/Vertical layout
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Layout {
    pub base: BaseUIElement,
    pub container: Container,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            container: Container::default(),
        }
    }
}

impl_ui_element!(Layout);

/// Grid layout
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GridLayout {
    pub base: BaseUIElement,
    pub container: Container,
    pub cols: usize,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            container: Container {
                mode: ContainerMode::Grid,
                ..Default::default()
            },
            cols: 1,
        }
    }
}

impl_ui_element!(GridLayout);

/// =========================
/// 3. Visual container
/// =========================

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

impl_ui_element!(UIPanel);
