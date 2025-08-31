use std::{collections::HashMap, default};
use serde::{Deserialize, Serialize};
use crate::{Color, Transform2D, Vector2};

#[derive(Debug, Clone)]
pub enum FurNode {
    Element(FurElement),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct FurElement {
    pub tag_name: String,                       
    pub id: String,       
    pub style: FurStyle,
    pub attributes: HashMap<String, String>,   
    pub children: Vec<FurNode>,                 
    pub self_closing: bool,                     
}

#[derive(Debug, Clone, Default)]
pub struct FurStyle {
    pub background_color: Option<Color>,
    pub modulate: Option<Color>,
    pub margin: EdgeValues,
    pub padding: EdgeValues,
    pub corner_radius: CornerValues,
    pub translation: TranslationValues,
    pub size: XYValue,
    pub transform: Transform2DXY,
    pub border: f32,
    pub border_color: Option<Color>,
    pub anchor: FurAnchor,
    pub z_index: i32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    pub top: Option<ValueOrPercent>,
    pub right: Option<ValueOrPercent>,
    pub bottom: Option<ValueOrPercent>,
    pub left: Option<ValueOrPercent>,
}

#[derive(Debug, Clone, Default)]
pub struct CornerValues {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_left: f32,
    pub bottom_right: f32,
}

#[derive(Debug, Clone, Default)]
pub struct TranslationValues {
    pub x: Option<ValueOrPercent>,
    pub y: Option<ValueOrPercent>,
}

#[derive(Debug, Clone, Default)]
pub struct XYValue {
    pub x: Option<ValueOrPercent>,
    pub y: Option<ValueOrPercent>,
}

#[derive(Debug, Clone, Default)]
pub struct Transform2DXY {
    pub scale: XYValue,
    pub rotation: Option<ValueOrPercent>,
    pub position: XYValue,
}

#[derive(Debug, Clone, Copy)]
pub enum ValueOrPercent {
    Abs(f32),
    Percent(f32),
}
