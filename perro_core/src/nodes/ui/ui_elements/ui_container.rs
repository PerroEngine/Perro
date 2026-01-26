use crate::structs::Color;
use crate::structs2d::Vector2;
use crate::{impl_ui_element, ui_element::BaseUIElement, UIElementID};
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    pub fn uniform(padding: f32) -> Self {
        Self {
            top: padding,
            right: padding,
            bottom: padding,
            left: padding,
        }
    }
    
    /// Returns the total horizontal padding (left + right)
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }
    
    /// Returns the total vertical padding (top + bottom)
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Container {
    pub mode: ContainerMode,            // Horizontal, Vertical, Grid
    pub gap: Vector2,                   // extra spacing between children (added on top of default gap)
    pub distribution: DistributionMode, // pack or even spacing
    pub padding: Padding,               // padding that reduces effective parent size for children
    #[serde(default)]
    pub align: LayoutAlignment,        // alignment of children (start/center/end)
}

impl Default for Container {
    fn default() -> Self {
        Self {
            mode: ContainerMode::Horizontal,
            gap: Vector2::new(0.0, 0.0),
            distribution: DistributionMode::Pack,
            padding: Padding::default(),
            align: LayoutAlignment::Center,
        }
    }
}

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Debug, Copy)]
pub enum ContainerMode {
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistributionMode {
    Pack,
    Even,
}

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Debug, Copy)]
pub enum LayoutAlignment {
    Start,  // Align to start (left for horizontal, top for vertical)
    Center, // Center alignment (default)
    End,    // Align to end (right for horizontal, bottom for vertical)
}

impl Default for LayoutAlignment {
    fn default() -> Self {
        LayoutAlignment::Center
    }
}

/// Horizontal/Vertical layout (deprecated - use VLayout or HLayout instead)
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

/// Vertical layout - positions children vertically
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VLayout {
    pub base: BaseUIElement,
    pub gap: Vector2,                   // spacing between children
    pub distribution: DistributionMode, // pack or even spacing
    pub padding: Padding,               // padding that reduces effective parent size for children
    #[serde(default)]
    pub align: LayoutAlignment,        // alignment of children (start/center/end)
}

impl Default for VLayout {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            gap: Vector2::new(0.0, 0.0),
            distribution: DistributionMode::Pack,
            padding: Padding::default(),
            align: LayoutAlignment::Center,
        }
    }
}

impl_ui_element!(VLayout);

/// Horizontal layout - positions children horizontally
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HLayout {
    pub base: BaseUIElement,
    pub gap: Vector2,                   // spacing between children
    pub distribution: DistributionMode, // pack or even spacing
    pub padding: Padding,               // padding that reduces effective parent size for children
    #[serde(default)]
    pub align: LayoutAlignment,        // alignment of children (start/center/end)
}

impl Default for HLayout {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            gap: Vector2::new(0.0, 0.0),
            distribution: DistributionMode::Pack,
            padding: Padding::default(),
            align: LayoutAlignment::Center,
        }
    }
}

impl_ui_element!(HLayout);

/// Grid layout - positions children in a grid
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GridLayout {
    pub base: BaseUIElement,
    pub cols: usize,
    pub gap: Vector2,                   // spacing between grid cells
    pub padding: Padding,               // padding that reduces effective parent size for children
    #[serde(default)]
    pub align: LayoutAlignment,        // alignment of children within cells
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            base: BaseUIElement::default(),
            cols: 1,
            gap: Vector2::new(0.0, 0.0),
            padding: Padding::default(),
            align: LayoutAlignment::Center,
        }
    }
}

impl_ui_element!(GridLayout);

/// =========================
/// 3. Visual container - wraps egui Frame
/// =========================

/// UIPanel wraps egui Frame for visual styling
/// It provides background, border, and corner radius styling
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

impl_ui_element!(UIPanel);
