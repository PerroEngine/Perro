use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiLayoutContainer {
    pub base: UiNode,
    pub mode: UiLayoutMode,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub spacing_mode: UiLayoutSpacingMode,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
    pub columns: u32,
}

impl UiLayoutContainer {
    pub const fn new(mode: UiLayoutMode) -> Self {
        Self {
            base: UiNode::new(),
            mode,
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            spacing_mode: UiLayoutSpacingMode::Fixed,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
            columns: 1,
        }
    }

    pub const fn horizontal() -> Self {
        let mut value = Self::new(UiLayoutMode::H);
        value.base.layout.v_align = UiVerticalAlign::Center;
        value
    }

    pub const fn vertical() -> Self {
        let mut value = Self::new(UiLayoutMode::V);
        value.base.layout.h_align = UiHorizontalAlign::Center;
        value
    }

    pub const fn grid() -> Self {
        Self::new(UiLayoutMode::Grid)
    }
}

impl Default for UiLayoutContainer {
    fn default() -> Self {
        Self::new(UiLayoutMode::H)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiScrollContainer {
    pub base: UiNode,
    pub scroll: Vector2,
    pub scroll_animation: Option<UiScrollAnimation>,
    pub scroll_dir: UiScrollDirection,
    pub scroll_bar_side: UiScrollBarSide,
    pub scroll_bar_padding: f32,
}

impl UiScrollContainer {
    pub const fn new() -> Self {
        let mut base = UiNode::new();
        base.clip_children = true;
        Self {
            base,
            scroll: Vector2::ZERO,
            scroll_animation: None,
            scroll_dir: UiScrollDirection::Vertical,
            scroll_bar_side: UiScrollBarSide::Right,
            scroll_bar_padding: -1.0,
        }
    }

    pub fn scroll_to(&mut self, part: f32, duration: f32) {
        let part = if part.is_finite() {
            part.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let duration = if duration.is_finite() {
            duration.max(0.0)
        } else {
            0.0
        };
        self.scroll_animation = Some(UiScrollAnimation {
            start: self.scroll,
            target_part: part,
            elapsed: 0.0,
            duration,
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScrollAnimation {
    pub start: Vector2,
    pub target_part: f32,
    pub elapsed: f32,
    pub duration: f32,
}

impl Default for UiScrollContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiScrollContainer {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiScrollContainer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiScrollContainer {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiScrollDirection {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiScrollBarSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiFixedLayoutContainer {
    pub base: UiNode,
    pub spacing: f32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub spacing_mode: UiLayoutSpacingMode,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
    pub columns: u32,
}

impl UiFixedLayoutContainer {
    pub const fn new() -> Self {
        let mut value = Self {
            base: UiNode::new(),
            spacing: 0.0,
            h_spacing: 0.0,
            v_spacing: 0.0,
            spacing_mode: UiLayoutSpacingMode::Fixed,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
            columns: 1,
        };
        value.base.layout.v_align = UiVerticalAlign::Center;
        value
    }
}

impl Default for UiFixedLayoutContainer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiLayout {
    pub inner: UiLayoutContainer,
}

impl UiLayout {
    pub const fn new() -> Self {
        Self {
            inner: UiLayoutContainer::horizontal(),
        }
    }
}

impl Default for UiLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiLayout {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiLayout {
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiHLayout {
    pub inner: UiFixedLayoutContainer,
}

impl UiHLayout {
    pub const fn new() -> Self {
        Self {
            inner: UiFixedLayoutContainer::new(),
        }
    }

    pub const fn mode(&self) -> UiLayoutMode {
        UiLayoutMode::H
    }
}

impl Default for UiHLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiHLayout {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiHLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiHLayout {
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.inner.base
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiVLayout {
    pub inner: UiFixedLayoutContainer,
}

impl UiVLayout {
    pub const fn new() -> Self {
        let mut value = Self {
            inner: UiFixedLayoutContainer::new(),
        };
        value.inner.base.layout.h_align = UiHorizontalAlign::Center;
        value
    }

    pub const fn mode(&self) -> UiLayoutMode {
        UiLayoutMode::V
    }
}

impl Default for UiVLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiVLayout {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.inner.base
    }
}

impl DerefMut for UiVLayout {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner.base
    }
}

impl UiNodeBase for UiVLayout {
    fn ui_base(&self) -> &UiNode {
        &self.inner.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.inner.base
    }
}

pub type UiHBox = UiHLayout;
pub type UiVBox = UiVLayout;

#[derive(Clone, Debug, PartialEq)]
pub struct UiGrid {
    pub base: UiNode,
    pub columns: u32,
    pub h_spacing: f32,
    pub v_spacing: f32,
    pub h_spacing_mode: UiLayoutSpacingMode,
    pub v_spacing_mode: UiLayoutSpacingMode,
}

impl UiGrid {
    pub const fn new() -> Self {
        Self {
            base: UiNode::new(),
            columns: 1,
            h_spacing: 0.0,
            v_spacing: 0.0,
            h_spacing_mode: UiLayoutSpacingMode::Fixed,
            v_spacing_mode: UiLayoutSpacingMode::Fixed,
        }
    }
}

impl Default for UiGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiGrid {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiGrid {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiGrid {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
