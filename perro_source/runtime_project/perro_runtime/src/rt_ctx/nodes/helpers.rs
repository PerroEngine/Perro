use super::*;

pub(super) struct PreparedNode {
    pub(super) node: SceneNode,
    pub(super) node_type: NodeType,
    pub(super) tag_ids: Vec<TagID>,
}

pub(super) fn prepare_created_node(
    request: &NodeCreationTemplate,
    parent_id: NodeID,
) -> PreparedNode {
    let mut node = SceneNode::new(request.scene_node_data());
    if let Some(name) = request.name.clone() {
        node.set_name(name);
    }
    node.set_tags(Some(request.tags.clone()));
    node.parent = parent_id;
    let tag_ids = request.tags.iter().map(|tag| tag.id()).collect();

    PreparedNode {
        node,
        node_type: request.node_type,
        tag_ids,
    }
}

#[inline]
pub(super) fn cached_slot_for(
    runtime: &mut Runtime,
    id: perro_ids::NodeID,
) -> Option<(usize, u32)> {
    if id.is_nil() {
        return None;
    }

    if let Some(&(_, active_id)) = runtime.script_runtime.active_script_stack.last()
        && active_id == id
    {
        let resolved = (active_id.index() as usize, active_id.generation());
        runtime.script_runtime.last_node_lookup = Some((active_id, resolved.0, resolved.1));
        return Some(resolved);
    }

    if let Some((cached_id, cached_index, cached_generation)) =
        runtime.script_runtime.last_node_lookup
        && cached_id == id
    {
        return Some((cached_index, cached_generation));
    }

    let resolved = (id.index() as usize, id.generation());
    runtime.script_runtime.last_node_lookup = Some((id, resolved.0, resolved.1));
    Some(resolved)
}

impl Runtime {
    pub(super) fn mark_ui_base_change(
        &mut self,
        id: perro_ids::NodeID,
        before: &UiBox,
        after: &UiBox,
    ) {
        let flags = classify_ui_base_change(before, after);
        if flags != 0 {
            self.mark_ui_dirty(id, flags);
        }
        if before.visible != after.visible {
            self.mark_ui_visibility_dirty_subtree(id);
        }
    }

    pub(super) fn mark_ui_data_change(
        &mut self,
        id: perro_ids::NodeID,
        before: &SceneNodeData,
        after: &SceneNodeData,
    ) {
        let mut flags = match (ui_base_from_data(before), ui_base_from_data(after)) {
            (Some(before), Some(after)) => classify_ui_base_change(before, after),
            _ => 0,
        };
        flags |= classify_ui_node_payload_change(before, after);
        if flags != 0 {
            self.mark_ui_dirty(id, flags);
        }
        if let (Some(before), Some(after)) = (ui_base_from_data(before), ui_base_from_data(after))
            && before.visible != after.visible
        {
            self.mark_ui_visibility_dirty_subtree(id);
        }
    }

    pub(super) fn mark_ui_reparent_dirty(
        &mut self,
        child_id: perro_ids::NodeID,
        old_parent: perro_ids::NodeID,
        new_parent: perro_ids::NodeID,
    ) {
        let mut stack = vec![child_id];
        while let Some(id) = stack.pop() {
            let Some((is_ui, children)) = self.nodes.get(id).map(|node| {
                (
                    ui_base_from_data(&node.data).is_some(),
                    node.get_children_ids().to_vec(),
                )
            }) else {
                continue;
            };
            if is_ui {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_TRANSFORM
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
            stack.extend(children);
        }

        let mut seen_ui_parents = std::collections::HashSet::new();
        for ui_parent_id in [
            self.closest_ui_ancestor(old_parent),
            self.closest_ui_ancestor(new_parent),
        ]
        .into_iter()
        .flatten()
        {
            if seen_ui_parents.insert(ui_parent_id) {
                self.mark_ui_dirty(
                    ui_parent_id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    pub(super) fn closest_ui_ancestor(
        &self,
        mut node_id: perro_ids::NodeID,
    ) -> Option<perro_ids::NodeID> {
        while !node_id.is_nil() {
            let node = self.nodes.get(node_id)?;
            if ui_base_from_data(&node.data).is_some() {
                return Some(node_id);
            }
            node_id = node.parent;
        }
        None
    }

    pub(super) fn mark_ui_visibility_dirty_subtree(&mut self, root: perro_ids::NodeID) {
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some((is_ui, children, tree_refs)) = self.nodes.get(id).map(|node| {
                let tree_refs = match &node.data {
                    SceneNodeData::UiList(tree) => ui_tree_all_nodes_flat(tree),
                    _ => Vec::new(),
                };
                (
                    ui_base_from_data(&node.data).is_some(),
                    node.get_children_ids().to_vec(),
                    tree_refs,
                )
            }) else {
                continue;
            };

            if is_ui {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_TRANSFORM
                        | Self::UI_DIRTY_COMMANDS,
                );
            }

            stack.extend(children);
            stack.extend(tree_refs);
        }
    }
}

pub(super) fn ui_tree_all_nodes_flat(tree: &perro_ui::UiList) -> Vec<perro_ids::NodeID> {
    let mut out = Vec::new();
    out.extend(tree.roots.iter().copied());
    for branch in &tree.branches {
        out.extend(branch.children.iter().copied());
    }
    out.sort_unstable_by_key(|id| id.as_u64());
    out.dedup();
    out
}

pub(super) fn classify_ui_base_change(before: &UiBox, after: &UiBox) -> u16 {
    let mut flags = 0;
    if before.transform != after.transform {
        flags |= Runtime::UI_DIRTY_TRANSFORM | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.visible != after.visible {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF
            | Runtime::UI_DIRTY_LAYOUT_PARENT
            | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.modulate != after.modulate {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.size != after.layout.size
        || before.layout.min_size != after.layout.min_size
        || before.layout.max_size != after.layout.max_size
        || before.layout.min_size_scale != after.layout.min_size_scale
        || before.layout.max_size_scale != after.layout.max_size_scale
        || before.layout.margin != after.layout.margin
        || before.layout.h_size != after.layout.h_size
        || before.layout.v_size != after.layout.v_size
    {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF
            | Runtime::UI_DIRTY_LAYOUT_PARENT
            | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.padding != after.layout.padding
        || before.layout.h_align != after.layout.h_align
        || before.layout.v_align != after.layout.v_align
        || before.layout.anchor != after.layout.anchor
    {
        flags |= Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.layout.z_index != after.layout.z_index {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    if before.input_enabled != after.input_enabled || before.mouse_filter != after.mouse_filter {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    flags
}

pub(super) fn classify_ui_node_payload_change(
    before: &SceneNodeData,
    after: &SceneNodeData,
) -> u16 {
    match (before, after) {
        (SceneNodeData::UiPanel(before), SceneNodeData::UiPanel(after))
            if before.style != after.style =>
        {
            Runtime::UI_DIRTY_COMMANDS
        }
        (SceneNodeData::UiButton(before), SceneNodeData::UiButton(after)) => {
            let mut flags = 0;
            if before.style != after.style
                || before.pressed_style != after.pressed_style
                || before.hover_style != after.hover_style
                || before.disabled != after.disabled
            {
                flags |= Runtime::UI_DIRTY_COMMANDS;
            }
            flags
        }
        (SceneNodeData::UiImageButton(before), SceneNodeData::UiImageButton(after)) => {
            let mut flags = 0;
            if before.texture != after.texture
                || before.texture_region != after.texture_region
                || before.tint != after.tint
                || before.hover_tint != after.hover_tint
                || before.pressed_tint != after.pressed_tint
                || before.scale_mode != after.scale_mode
                || before.h_align != after.h_align
                || before.v_align != after.v_align
                || before.aspect_ratio != after.aspect_ratio
                || before.disabled != after.disabled
            {
                flags |= Runtime::UI_DIRTY_COMMANDS;
            }
            flags
        }
        (SceneNodeData::UiLabel(before), SceneNodeData::UiLabel(after)) => {
            let mut flags = 0;
            if before.text != after.text
                || before.font_size != after.font_size
                || before.text_size_ratio != after.text_size_ratio
                || before.font_sizing != after.font_sizing
            {
                flags |= Runtime::UI_DIRTY_TEXT
                    | Runtime::UI_DIRTY_LAYOUT_SELF
                    | Runtime::UI_DIRTY_LAYOUT_PARENT
                    | Runtime::UI_DIRTY_COMMANDS;
            }
            if before.color != after.color
                || before.h_align != after.h_align
                || before.v_align != after.v_align
            {
                flags |= Runtime::UI_DIRTY_COMMANDS;
            }
            flags
        }
        (SceneNodeData::UiTextBox(before), SceneNodeData::UiTextBox(after)) => {
            classify_text_edit_change(&before.inner, &after.inner)
        }
        (SceneNodeData::UiTextBlock(before), SceneNodeData::UiTextBlock(after)) => {
            classify_text_edit_change(&before.inner, &after.inner)
        }
        (SceneNodeData::UiLayout(before), SceneNodeData::UiLayout(after))
            if before.inner.mode != after.inner.mode
                || before.inner.spacing != after.inner.spacing
                || before.inner.h_spacing != after.inner.h_spacing
                || before.inner.v_spacing != after.inner.v_spacing
                || before.inner.columns != after.inner.columns =>
        {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        (SceneNodeData::UiHLayout(before), SceneNodeData::UiHLayout(after))
            if before.inner.spacing != after.inner.spacing
                || before.inner.h_spacing != after.inner.h_spacing
                || before.inner.v_spacing != after.inner.v_spacing
                || before.inner.columns != after.inner.columns =>
        {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        (SceneNodeData::UiVLayout(before), SceneNodeData::UiVLayout(after))
            if before.inner.spacing != after.inner.spacing
                || before.inner.h_spacing != after.inner.h_spacing
                || before.inner.v_spacing != after.inner.v_spacing
                || before.inner.columns != after.inner.columns =>
        {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        (SceneNodeData::UiGrid(before), SceneNodeData::UiGrid(after))
            if before.columns != after.columns
                || before.h_spacing != after.h_spacing
                || before.v_spacing != after.v_spacing =>
        {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        (SceneNodeData::UiList(before), SceneNodeData::UiList(after))
            if before.roots != after.roots
                || before.branches != after.branches
                || before.collapsed != after.collapsed
                || before.indent != after.indent
                || before.v_spacing != after.v_spacing =>
        {
            Runtime::UI_DIRTY_LAYOUT_SELF | Runtime::UI_DIRTY_COMMANDS
        }
        _ => 0,
    }
}

pub(super) fn classify_text_edit_change(
    before: &perro_ui::UiTextEdit,
    after: &perro_ui::UiTextEdit,
) -> u16 {
    let mut flags = 0;
    if before.text != after.text
        || before.font_size != after.font_size
        || before.text_size_ratio != after.text_size_ratio
        || before.font_sizing != after.font_sizing
    {
        flags |= Runtime::UI_DIRTY_TEXT
            | Runtime::UI_DIRTY_LAYOUT_SELF
            | Runtime::UI_DIRTY_LAYOUT_PARENT
            | Runtime::UI_DIRTY_COMMANDS;
    }
    if before.style != after.style
        || before.focused_style != after.focused_style
        || before.placeholder != after.placeholder
        || before.color != after.color
        || before.placeholder_color != after.placeholder_color
        || before.selection_color != after.selection_color
        || before.caret_color != after.caret_color
        || before.padding != after.padding
        || before.h_scroll != after.h_scroll
        || before.v_scroll != after.v_scroll
        || before.caret != after.caret
        || before.anchor != after.anchor
        || before.editable != after.editable
    {
        flags |= Runtime::UI_DIRTY_COMMANDS;
    }
    flags
}

pub(super) fn ui_base_from_data(data: &SceneNodeData) -> Option<&UiBox> {
    match data {
        SceneNodeData::UiBox(root) => Some(root),
        SceneNodeData::UiPanel(node) => Some(&node.base),
        SceneNodeData::UiButton(node) => Some(&node.base),
        SceneNodeData::UiImage(node) => Some(&node.base),
        SceneNodeData::UiImageButton(node) => Some(&node.base),
        SceneNodeData::UiNineSlice(node) => Some(&node.base),
        SceneNodeData::UiAnimatedImage(node) => Some(&node.base),
        SceneNodeData::UiLabel(node) => Some(&node.base),
        SceneNodeData::UiTextBox(node) => Some(&node.inner.base),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner.base),
        SceneNodeData::UiLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiHLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiVLayout(node) => Some(&node.inner.base),
        SceneNodeData::UiGrid(node) => Some(&node.base),
        SceneNodeData::UiList(node) => Some(&node.base),
        SceneNodeData::UiListIndent(node) => Some(&node.base),
        _ => None,
    }
}
