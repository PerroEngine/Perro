use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct UiTreeListItem {
    pub id: Cow<'static, str>,
    pub label: Cow<'static, str>,
    pub value: perro_variant::Variant,
    pub icon: TextureID,
    pub parent: Option<usize>,
    pub open: bool,
    pub selectable: bool,
}

impl UiTreeListItem {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        let label = label.into();
        Self {
            id: label.clone(),
            value: perro_variant::Variant::from(label.as_ref()),
            label,
            icon: TextureID::nil(),
            parent: None,
            open: true,
            selectable: true,
        }
    }

    pub fn child(mut self, parent: usize) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_id(mut self, id: impl Into<Cow<'static, str>>) -> Self {
        self.id = id.into();
        self
    }

    pub fn with_value(mut self, value: perro_variant::Variant) -> Self {
        self.value = value;
        self
    }

    pub fn with_icon(mut self, icon: TextureID) -> Self {
        self.icon = icon;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiTreeListVisibleItem {
    pub index: usize,
    pub depth: u32,
    pub has_children: bool,
    pub last_child: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiTreeList {
    pub base: UiNode,
    pub items: Vec<UiTreeListItem>,
    pub selected_index: Option<usize>,
    pub indent: f32,
    pub row_height: f32,
    pub v_spacing: f32,
    pub icon_size: f32,
    pub toggle_size: f32,
    pub line_width: f32,
    pub line_color: Color,
    pub triangle_color: Color,
    pub text_color: Color,
    pub row_style: UiStyle,
    pub row_hover_style: UiStyle,
    pub row_pressed_style: UiStyle,
    pub selected_style: UiStyle,
    pub internal_rows: Vec<NodeID>,
    pub internal_toggles: Vec<NodeID>,
    pub internal_icons: Vec<NodeID>,
    pub internal_labels: Vec<NodeID>,
    pub internal_lines: Vec<[NodeID; 2]>,
    pub selected_signals: Vec<SignalID>,
    pub toggled_signals: Vec<SignalID>,
}

impl UiTreeList {
    pub fn new() -> Self {
        let mut base = UiNode::new();
        base.layout.h_align = UiHorizontalAlign::Left;
        base.layout.v_align = UiVerticalAlign::Top;
        Self {
            base,
            items: Vec::new(),
            selected_index: None,
            indent: 16.0,
            row_height: 24.0,
            v_spacing: 0.0,
            icon_size: 16.0,
            toggle_size: 12.0,
            line_width: 1.0,
            line_color: Color::new(0.32, 0.36, 0.44, 1.0),
            triangle_color: Color::new(0.72, 0.76, 0.84, 1.0),
            text_color: Color::WHITE,
            row_style: UiStyle::button(),
            row_hover_style: UiStyle {
                fill: Color::new(0.24, 0.27, 0.32, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            row_pressed_style: UiStyle {
                fill: Color::new(0.12, 0.14, 0.18, 1.0),
                stroke: Color::new(0.42, 0.46, 0.54, 1.0),
                ..UiStyle::button()
            },
            selected_style: UiStyle {
                fill: Color::new(0.18, 0.28, 0.45, 1.0),
                stroke: Color::new(0.38, 0.55, 0.82, 1.0),
                corner_radii: UiCornerRadii::all(0.08),
                ..UiStyle::button()
            },
            internal_rows: Vec::new(),
            internal_toggles: Vec::new(),
            internal_icons: Vec::new(),
            internal_labels: Vec::new(),
            internal_lines: Vec::new(),
            selected_signals: Vec::new(),
            toggled_signals: Vec::new(),
        }
    }

    pub fn visible_items(&self) -> Vec<UiTreeListVisibleItem> {
        let mut out = Vec::new();
        self.push_visible_children(None, 0, &mut out);
        out
    }

    pub fn clear_items(&mut self) {
        self.items.clear();
        self.selected_index = None;
    }

    pub fn push_root(&mut self, item: UiTreeListItem) -> usize {
        self.push_item(None, item)
    }

    pub fn push_child(&mut self, parent: usize, item: UiTreeListItem) -> usize {
        self.push_item(Some(parent), item)
    }

    pub fn push_item(&mut self, parent: Option<usize>, mut item: UiTreeListItem) -> usize {
        let idx = self.items.len();
        item.parent = parent.filter(|parent| *parent < idx);
        self.items.push(item);
        idx
    }

    pub fn set_open(&mut self, index: usize, open: bool) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        if item.open == open {
            return false;
        }
        item.open = open;
        true
    }

    pub fn toggle_open(&mut self, index: usize) -> bool {
        let Some(item) = self.items.get_mut(index) else {
            return false;
        };
        item.open = !item.open;
        true
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected_index = index.filter(|index| *index < self.items.len());
    }

    fn push_visible_children(
        &self,
        parent: Option<usize>,
        depth: u32,
        out: &mut Vec<UiTreeListVisibleItem>,
    ) {
        let children = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| (item.parent == parent).then_some(idx))
            .collect::<Vec<_>>();
        for (pos, idx) in children.iter().copied().enumerate() {
            let has_children = self.items.iter().any(|item| item.parent == Some(idx));
            out.push(UiTreeListVisibleItem {
                index: idx,
                depth,
                has_children,
                last_child: pos + 1 == children.len(),
            });
            if has_children && self.items.get(idx).is_some_and(|item| item.open) {
                self.push_visible_children(Some(idx), depth.saturating_add(1), out);
            }
        }
    }
}

impl Default for UiTreeList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiTreeList {
    type Target = UiNode;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UiTreeList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UiNodeBase for UiTreeList {
    fn ui_base(&self) -> &UiNode {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiNode {
        &mut self.base
    }
}
