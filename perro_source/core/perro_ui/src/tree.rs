use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiTreeBranch {
    pub parent: NodeID,
    pub children: Vec<NodeID>,
}

impl UiTreeBranch {
    pub fn new(parent: NodeID, children: Vec<NodeID>) -> Self {
        Self { parent, children }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiTreeList {
    pub base: UiBox,
    pub roots: Vec<NodeID>,
    pub branches: Vec<UiTreeBranch>,
    pub collapsed: Vec<NodeID>,
    pub indent: f32,
    pub v_spacing: f32,
}

impl UiTreeList {
    pub const fn new() -> Self {
        let mut base = UiBox::new();
        base.layout.h_align = UiHorizontalAlign::Left;
        base.layout.v_align = UiVerticalAlign::Top;
        Self {
            base,
            roots: Vec::new(),
            branches: Vec::new(),
            collapsed: Vec::new(),
            indent: 16.0,
            v_spacing: 0.0,
        }
    }

    pub fn set_roots(&mut self, roots: Vec<NodeID>) {
        self.roots = roots;
    }

    pub fn set_branch(&mut self, parent: NodeID, children: Vec<NodeID>) {
        if let Some(branch) = self
            .branches
            .iter_mut()
            .find(|branch| branch.parent == parent)
        {
            branch.children = children;
        } else {
            self.branches.push(UiTreeBranch::new(parent, children));
        }
    }

    pub fn children_of(&self, parent: NodeID) -> &[NodeID] {
        self.branches
            .iter()
            .find(|branch| branch.parent == parent)
            .map(|branch| branch.children.as_slice())
            .unwrap_or(&[])
    }

    pub fn is_collapsed(&self, item: NodeID) -> bool {
        self.collapsed.contains(&item)
    }
}

impl Default for UiTreeList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for UiTreeList {
    type Target = UiBox;

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
    fn ui_base(&self) -> &UiBox {
        &self.base
    }

    fn ui_base_mut(&mut self) -> &mut UiBox {
        &mut self.base
    }
}
