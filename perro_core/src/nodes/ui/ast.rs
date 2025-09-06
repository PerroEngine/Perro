use std::{collections::HashMap, default};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum FurNode {
    Element(FurElement),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct FurElement {
    pub tag_name: String,                       
    pub id: String,       
    pub attributes: HashMap<String, String>,   
    pub children: Vec<FurNode>,                 
    pub self_closing: bool,                     
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
