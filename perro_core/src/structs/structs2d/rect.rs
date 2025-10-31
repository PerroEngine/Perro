use serde::{Serialize,Deserialize};

#[derive(Serialize,Deserialize,Clone,Copy,Debug)]
pub struct Rect { pub x:f32, pub y:f32, pub w:f32, pub h:f32 }