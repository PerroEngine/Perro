use super::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeModulate {
    pub modulate: Color,
    pub self_modulate: Color,
    pub children_modulate: Color,
}

impl NodeModulate {
    pub const WHITE: Self = Self::new(Color::WHITE, Color::WHITE, Color::WHITE);

    pub const fn new(modulate: Color, self_modulate: Color, children_modulate: Color) -> Self {
        Self {
            modulate,
            self_modulate,
            children_modulate,
        }
    }
}

impl Default for NodeModulate {
    fn default() -> Self {
        Self::WHITE
    }
}
