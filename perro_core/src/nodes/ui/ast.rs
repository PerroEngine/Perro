use std::collections::HashMap;

use crate::{Color, Transform2D, Vector2};

#[derive(Debug, Clone)]
pub enum FurNode {
    Element(FurElement),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct FurElement {
    pub tag_name: String,                       // e.g. "Panel"
    pub id: String,       // optional id, if provided
    pub style: FurStyle,
    pub attributes: HashMap<String, String>,   // e.g. style -> "bg-red, border-lg"
    pub children: Vec<FurNode>,                 // nested elements inside this tag
    pub self_closing: bool,                     // true if like [Panel/]
}

#[derive(Debug, Clone, Default)]
pub struct FurStyle {
    pub background_color: Option<Color>,
    pub modulate: Option<Color>,
    pub margin: EdgeValues,
    pub padding: EdgeValues,
    pub corner_radius: CornerValues,
    pub translation: TranslationValues,
    pub size: Vector2,
    pub transform: Transform2D,
    pub border: Option<f32>,
    pub border_color: Option<Color>,
    pub anchor: FurAnchor
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FurAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Debug, Clone, Default)]
pub struct EdgeValues {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct CornerValues {
    pub top_left: Option<f32>,
    pub top_right: Option<f32>,
    pub bottom_left: Option<f32>,
    pub bottom_right: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct TranslationValues {
    pub x: Option<f32>,
    pub y: Option<f32>,
}
