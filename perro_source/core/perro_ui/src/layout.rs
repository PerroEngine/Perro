use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedUiRect {
    pub center: Vector2,
    pub size: Vector2,
}

impl ComputedUiRect {
    pub const fn new(center: Vector2, size: Vector2) -> Self {
        Self { center, size }
    }

    pub fn min(self) -> Vector2 {
        self.center - self.size * 0.5
    }

    pub fn max(self) -> Vector2 {
        self.center + self.size * 0.5
    }

    pub fn contains(self, point: Vector2) -> bool {
        let min = self.min();
        let max = self.max();
        point.x >= min.x && point.x <= max.x && point.y >= min.y && point.y <= max.y
    }

    pub fn contains_rounded(self, point: Vector2, corner_radius: f32) -> bool {
        if !self.contains(point) {
            return false;
        }

        let ratio = if corner_radius.is_finite() {
            corner_radius.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let radius = self.size.x.min(self.size.y).max(0.0) * 0.5 * ratio;
        if radius <= 0.0 {
            return true;
        }

        let half = self.size * 0.5;
        let local = Vector2::new(
            (point.x - self.center.x).abs(),
            (point.y - self.center.y).abs(),
        );
        let inner = Vector2::new((half.x - radius).max(0.0), (half.y - radius).max(0.0));
        let q = Vector2::new((local.x - inner.x).max(0.0), (local.y - inner.y).max(0.0));
        q.x * q.x + q.y * q.y <= radius * radius
    }

    pub fn inset(self, inset: UiRect) -> Self {
        let min = self.min() + Vector2::new(inset.left, inset.bottom);
        let max = self.max() - Vector2::new(inset.right, inset.top);
        let size = Vector2::new((max.x - min.x).max(0.0), (max.y - min.y).max(0.0));
        Self::new(min + size * 0.5, size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTransform {
    pub position: UiVector2,
    pub pivot: UiVector2,
    pub translation: Vector2,
    pub self_translation: Vector2,
    pub scale: Vector2,
    pub rotation: f32,
}

impl UiTransform {
    pub const fn new() -> Self {
        Self {
            position: UiVector2::percent(50.0, 50.0),
            pivot: UiVector2::percent(50.0, 50.0),
            translation: Vector2::ZERO,
            self_translation: Vector2::ZERO,
            scale: Vector2::ONE,
            rotation: 0.0,
        }
    }

    pub fn resolved_position(&self, parent_size: Vector2) -> Vector2 {
        self.position.resolve(parent_size) + self.translation
    }

    pub fn translation_offset(&self, parent_size: Vector2, resolved_size: Vector2) -> Vector2 {
        Vector2::new(
            self.translation.x * parent_size.x + self.self_translation.x * resolved_size.x,
            self.translation.y * parent_size.y + self.self_translation.y * resolved_size.y,
        )
    }

    pub fn scale_size(&self, size: Vector2) -> Vector2 {
        Vector2::new(size.x * self.scale.x, size.y * self.scale.y)
    }

    pub fn resolved_pivot_offset(&self, resolved_size: Vector2) -> Vector2 {
        self.pivot.resolve(resolved_size)
    }
}

impl Default for UiTransform {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiLayoutData {
    pub anchor: UiAnchor,
    pub size: UiVector2,
    pub min_size: Vector2,
    pub max_size: Vector2,
    pub min_size_scale: Vector2,
    pub max_size_scale: Vector2,
    pub margin: UiRect,
    pub padding: UiRect,
    pub h_size: UiSizeMode,
    pub v_size: UiSizeMode,
    pub h_align: UiHorizontalAlign,
    pub v_align: UiVerticalAlign,
    pub z_index: i32,
}

impl UiLayoutData {
    pub const NO_MAX_SIZE: Vector2 = Vector2::new(f32::INFINITY, f32::INFINITY);

    pub const fn new() -> Self {
        Self {
            anchor: UiAnchor::Center,
            size: UiVector2::ZERO,
            min_size: Vector2::ZERO,
            max_size: Self::NO_MAX_SIZE,
            min_size_scale: Vector2::ZERO,
            max_size_scale: Vector2::new(f32::INFINITY, f32::INFINITY),
            margin: UiRect::ZERO,
            padding: UiRect::ZERO,
            h_size: UiSizeMode::Fixed,
            v_size: UiSizeMode::Fixed,
            h_align: UiHorizontalAlign::Center,
            v_align: UiVerticalAlign::Center,
            z_index: 0,
        }
    }

    pub fn resolved_size(&self, parent_size: Vector2) -> Vector2 {
        self.size.resolve(parent_size)
    }

    pub fn clamp_size(&self, size: Vector2) -> Vector2 {
        Vector2::new(
            size.x.clamp(self.min_size.x, self.max_size.x),
            size.y.clamp(self.min_size.y, self.max_size.y),
        )
    }

    pub fn resolved_scaled_size(&self, transform: &UiTransform, parent_size: Vector2) -> Vector2 {
        let size = self.resolved_size(parent_size);
        transform.scale_size(size)
    }

    pub fn resolved_origin(&self, transform: &UiTransform, parent_size: Vector2) -> Vector2 {
        let size = self.resolved_size(parent_size);
        transform.resolved_position(parent_size) - transform.resolved_pivot_offset(size)
    }

    pub fn compute_rect(&self, transform: &UiTransform, parent: ComputedUiRect) -> ComputedUiRect {
        let size = self.resolved_scaled_size(transform, parent.size);
        self.compute_rect_with_size(transform, parent, size)
    }

    pub fn compute_rect_with_size(
        &self,
        transform: &UiTransform,
        parent: ComputedUiRect,
        size: Vector2,
    ) -> ComputedUiRect {
        let anchor = self.anchor.direction();
        let anchor_point = parent.center
            + Vector2::new(
                parent.size.x * 0.5 * anchor.x,
                parent.size.y * 0.5 * anchor.y,
            );
        let anchored_size = Vector2::new(size.x.min(parent.size.x), size.y.min(parent.size.y));
        let inward_from_edge = Vector2::new(
            anchored_size.x * 0.5 * anchor.x,
            anchored_size.y * 0.5 * anchor.y,
        );
        let position = transform.position.resolve_centered(parent.size);

        ComputedUiRect::new(
            anchor_point - inward_from_edge
                + position
                + transform.translation_offset(parent.size, size),
            size,
        )
    }
}

impl Default for UiLayoutData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiNode {
    pub transform: UiTransform,
    pub layout: UiLayoutData,
    pub visible: bool,
    pub modulate: perro_structs::NodeModulate,
    pub input_enabled: bool,
    pub mouse_filter: UiMouseFilter,
    pub clip_children: bool,
}

impl UiNode {
    pub const fn new() -> Self {
        Self {
            transform: UiTransform::new(),
            layout: UiLayoutData::new(),
            visible: true,
            modulate: perro_structs::NodeModulate::WHITE,
            input_enabled: true,
            mouse_filter: UiMouseFilter::Stop,
            clip_children: false,
        }
    }
}

impl Default for UiNode {
    fn default() -> Self {
        Self::new()
    }
}

pub trait UiNodeBase {
    fn ui_base(&self) -> &UiNode;
    fn ui_base_mut(&mut self) -> &mut UiNode;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UiMouseFilter {
    #[default]
    Stop,
    Pass,
    Ignore,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CursorIcon {
    #[default]
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
    DndAsk,
    AllResize,
}
