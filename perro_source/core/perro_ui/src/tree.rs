use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiList {
    pub base: UiNode,
    pub indent: f32,
    pub v_spacing: f32,
}

impl UiList {
    pub const fn new() -> Self {
        let mut base = UiNode::new();
        base.layout.h_align = UiHorizontalAlign::Left;
        base.layout.v_align = UiVerticalAlign::Top;
        Self {
            base,
            indent: 16.0,
            v_spacing: 0.0,
        }
    }
}

impl Default for UiList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiList {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiList {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiListIndent {
    pub base: UiNode,
}

impl UiListIndent {
    pub const fn new() -> Self {
        let mut base = UiNode::new();
        base.layout.h_align = UiHorizontalAlign::Left;
        base.layout.v_align = UiVerticalAlign::Top;
        Self { base }
    }
}

impl Default for UiListIndent {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiListIndent {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiListIndent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiListIndent {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
