use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap};

#[derive(Debug, Clone)]
pub enum FurNode {
    Element(FurElement),
    Text(Cow<'static, str>),
}

#[derive(Debug, Clone)]
pub struct FurElement {
    pub tag_name: Cow<'static, str>,
    pub id: Cow<'static, str>,
    pub attributes: HashMap<Cow<'static, str>, Cow<'static, str>>,
    pub children: Vec<FurNode>,
    pub self_closing: bool,
}

#[derive(Hash, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
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
