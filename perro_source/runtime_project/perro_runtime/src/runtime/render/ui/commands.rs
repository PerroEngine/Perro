use super::*;

impl Runtime {
    pub(super) fn ensure_tree_list_internal_nodes(&mut self) {
        // Full sync clones the item list and re-marks every internal row
        // dirty, so it must not run on unrelated extraction work (pointer
        // moves, other widgets): gate it on the tree node itself being
        // dirty. Engine-side mutations (row clicks) call
        // sync_tree_list_internal_nodes directly.
        let tree_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiTreeList(tree) => {
                    let never_synced = tree.internal_rows.is_empty() && !tree.items.is_empty();
                    Some((id, never_synced))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (tree_id, never_synced) in tree_ids {
            if never_synced || self.dirty.ui_flags_at(tree_id.index() as usize) != 0 {
                self.sync_tree_list_internal_nodes(tree_id);
            }
        }
    }

    pub(super) fn ensure_tree_list_internal_nodes_for(&mut self, tree_id: NodeID) {
        let Some((mut rows, mut toggles, mut icons, mut labels, mut lines, row_count)) =
            self.nodes.get(tree_id).and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some((
                    tree.internal_rows.clone(),
                    tree.internal_toggles.clone(),
                    tree.internal_icons.clone(),
                    tree.internal_labels.clone(),
                    tree.internal_lines.clone(),
                    tree.visible_items().len(),
                )),
                _ => None,
            })
        else {
            return;
        };

        for id in rows.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in toggles.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in icons.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for id in labels.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(id);
        }
        for pair in lines.iter().copied().skip(row_count) {
            self.hide_tree_list_internal_node(pair[0]);
            self.hide_tree_list_internal_node(pair[1]);
        }

        if rows.len() < row_count {
            rows.resize(row_count, NodeID::nil());
        }
        if toggles.len() < row_count {
            toggles.resize(row_count, NodeID::nil());
        }
        if icons.len() < row_count {
            icons.resize(row_count, NodeID::nil());
        }
        if labels.len() < row_count {
            labels.resize(row_count, NodeID::nil());
        }
        if lines.len() < row_count {
            lines.resize(row_count, [NodeID::nil(); 2]);
        }

        for idx in 0..row_count {
            if !self.tree_list_internal_valid(rows[idx], tree_id, "button") {
                rows[idx] = self.insert_tree_list_row(tree_id, idx);
            }
            if !self.tree_list_internal_valid(toggles[idx], rows[idx], "shape") {
                toggles[idx] = self.insert_tree_list_toggle(rows[idx], idx);
            }
            if !self.tree_list_internal_valid(icons[idx], rows[idx], "image") {
                icons[idx] = self.insert_tree_list_icon(rows[idx], idx);
            }
            if !self.tree_list_internal_valid(labels[idx], rows[idx], "label") {
                labels[idx] = self.insert_tree_list_label(rows[idx], idx);
            }
            for (line_idx, line) in lines[idx].iter_mut().enumerate() {
                if !self.tree_list_internal_valid(*line, rows[idx], "panel") {
                    *line = self.insert_tree_list_line(rows[idx], idx, line_idx);
                }
            }
        }

        if let Some(node) = self.nodes.get_mut_untracked(tree_id)
            && let SceneNodeData::UiTreeList(tree) = &mut node.data
        {
            tree.internal_rows = rows;
            tree.internal_toggles = toggles;
            tree.internal_icons = icons;
            tree.internal_labels = labels;
            tree.internal_lines = lines;
        }
    }

    pub(super) fn hide_tree_list_internal_node(&mut self, id: NodeID) {
        if let Some(node) = self.nodes.get_mut_untracked(id)
            && let Some(ui) = ui_root_mut_from_data(&mut node.data)
        {
            ui.visible = false;
        }
    }

    pub(super) fn tree_list_internal_valid(&self, id: NodeID, parent: NodeID, kind: &str) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && match kind {
                    "button" => matches!(node.data, SceneNodeData::UiButton(_)),
                    "shape" => matches!(node.data, SceneNodeData::UiShape(_)),
                    "image" => matches!(node.data, SceneNodeData::UiImage(_)),
                    "label" => matches!(node.data, SceneNodeData::UiLabel(_)),
                    "panel" => matches!(node.data, SceneNodeData::UiPanel(_)),
                    _ => false,
                }
        })
    }

    pub(super) fn insert_tree_list_row(&mut self, tree_id: NodeID, idx: usize) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.anchor = UiAnchor::Top;
        button.base.layout.z_index = 1;
        button.base.clip_children = false;
        button.style.fill = Color::TRANSPARENT;
        button.style.stroke = Color::TRANSPARENT;
        button.hover_style.fill = Color::new(0.18, 0.22, 0.30, 1.0);
        button.pressed_style.fill = Color::new(0.12, 0.16, 0.24, 1.0);
        self.insert_color_picker_internal_node(
            tree_id,
            format!("__perro_tree_list_row_{idx}"),
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    pub(super) fn insert_tree_list_toggle(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut shape = perro_ui::UiShape::new();
        shape.base.layout.anchor = UiAnchor::Left;
        shape.base.layout.z_index = 3;
        shape.base.input_enabled = false;
        shape.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        shape.kind = perro_ui::UiShapeKind::Triangle;
        self.insert_color_picker_internal_node(
            row_id,
            format!("__perro_tree_list_toggle_{idx}"),
            shape.into(),
        )
    }

    pub(super) fn insert_tree_list_icon(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut image = perro_ui::UiImage::new();
        image.base.layout.anchor = UiAnchor::Left;
        image.base.layout.z_index = 3;
        image.base.input_enabled = false;
        image.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        self.insert_color_picker_internal_node(
            row_id,
            format!("__perro_tree_list_icon_{idx}"),
            SceneNodeData::UiImage(Box::new(image)),
        )
    }

    pub(super) fn insert_tree_list_label(&mut self, row_id: NodeID, idx: usize) -> NodeID {
        let mut label = perro_ui::UiLabel::new();
        label.base.layout.anchor = UiAnchor::Left;
        label.base.layout.z_index = 3;
        label.base.input_enabled = false;
        label.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        label.h_align = perro_ui::UiTextAlign::Start;
        label.v_align = perro_ui::UiTextAlign::Center;
        label.text_size_ratio = 0.62;
        self.insert_color_picker_internal_node(
            row_id,
            format!("__perro_tree_list_label_{idx}"),
            SceneNodeData::UiLabel(Box::new(label)),
        )
    }

    pub(super) fn insert_tree_list_line(
        &mut self,
        row_id: NodeID,
        idx: usize,
        line_idx: usize,
    ) -> NodeID {
        let mut panel = UiPanel::new();
        panel.base.layout.anchor = UiAnchor::Left;
        panel.base.layout.z_index = 2;
        panel.base.input_enabled = false;
        panel.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        panel.style.stroke_width = 0.0;
        self.insert_color_picker_internal_node(
            row_id,
            format!("__perro_tree_list_line_{idx}_{line_idx}"),
            SceneNodeData::UiPanel(Box::new(panel)),
        )
    }

    pub(super) fn sync_tree_list_internal_nodes(&mut self, tree_id: NodeID) {
        self.ensure_tree_list_internal_nodes_for(tree_id);
        let Some(snapshot) = self.nodes.get(tree_id).and_then(|node| match &node.data {
            SceneNodeData::UiTreeList(tree) => Some((
                tree.visible,
                tree.visible_items(),
                tree.items.clone(),
                tree.selected_index,
                tree.indent,
                tree.row_height,
                tree.v_spacing,
                tree.icon_size,
                tree.toggle_size,
                tree.line_width,
                tree.line_color,
                tree.triangle_color,
                tree.text_color,
                tree.row_style.clone(),
                tree.row_hover_style.clone(),
                tree.row_pressed_style.clone(),
                tree.selected_style.clone(),
                tree.internal_rows.clone(),
                tree.internal_toggles.clone(),
                tree.internal_icons.clone(),
                tree.internal_labels.clone(),
                tree.internal_lines.clone(),
            )),
            _ => None,
        }) else {
            return;
        };
        let (
            visible,
            rows,
            items,
            selected_index,
            indent,
            row_height,
            v_spacing,
            icon_size,
            toggle_size,
            line_width,
            line_color,
            triangle_color,
            text_color,
            row_style,
            row_hover_style,
            row_pressed_style,
            selected_style,
            internal_rows,
            internal_toggles,
            internal_icons,
            internal_labels,
            internal_lines,
        ) = snapshot;
        let spacing = ui_v_spacing_amount(v_spacing, row_height);
        for (visible_idx, row) in rows.iter().enumerate() {
            let Some(item) = items.get(row.index) else {
                continue;
            };
            let y = -((row_height + spacing) * visible_idx as f32);
            let x = indent * row.depth as f32;
            if let Some(node) = self.nodes.get_mut_untracked(internal_rows[visible_idx])
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = visible;
                button.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(row_height),
                );
                button.base.transform.position = UiVector2::pixels(0.0, y);
                button.base.layout.anchor = UiAnchor::Top;
                button.style = if selected_index == Some(row.index) {
                    selected_style.clone()
                } else {
                    row_style.clone()
                };
                button.hover_style = row_hover_style.clone();
                button.pressed_style = row_pressed_style.clone();
                button.disabled = !item.selectable;
            }
            if let Some(node) = self.nodes.get_mut_untracked(internal_toggles[visible_idx])
                && let SceneNodeData::UiShape(shape) = &mut node.data
            {
                shape.base.visible = visible && row.has_children;
                shape.base.layout.size = UiVector2::pixels(toggle_size, toggle_size);
                shape.base.transform.position = UiVector2::pixels(x + toggle_size * 0.5, 0.0);
                shape.fill = triangle_color;
                shape.stroke = Color::TRANSPARENT;
                shape.base.transform.rotation = if item.open {
                    std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                };
            }
            if let Some(node) = self.nodes.get_mut_untracked(internal_icons[visible_idx])
                && let SceneNodeData::UiImage(image) = &mut node.data
            {
                image.base.visible = visible && !item.icon.is_nil();
                image.base.layout.size = UiVector2::pixels(icon_size, icon_size);
                image.base.transform.position =
                    UiVector2::pixels(x + toggle_size + icon_size * 0.5 + 3.0, 0.0);
                image.texture = item.icon;
            }
            if let Some(node) = self.nodes.get_mut_untracked(internal_labels[visible_idx])
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                let icon_width = if item.icon.is_nil() {
                    0.0
                } else {
                    icon_size + 4.0
                };
                label.base.visible = visible;
                label.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(row_height),
                );
                label.base.transform.position =
                    UiVector2::pixels(x + toggle_size + icon_width + 8.0, 0.0);
                label.color = text_color;
                if label.text != item.label {
                    label.set_text(item.label.to_string());
                }
            }
            if let Some(pair) = internal_lines.get(visible_idx).copied() {
                self.sync_tree_list_line(
                    pair[0],
                    visible,
                    row.last_child,
                    x,
                    row_height,
                    line_width,
                    line_color,
                    true,
                );
                self.sync_tree_list_line(
                    pair[1],
                    visible,
                    row.last_child,
                    x,
                    row_height,
                    line_width,
                    line_color,
                    false,
                );
            }
        }
        for id in internal_rows.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_toggles.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_icons.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for id in internal_labels.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(id);
        }
        for pair in internal_lines.iter().copied().skip(rows.len()) {
            self.hide_tree_list_internal_node(pair[0]);
            self.hide_tree_list_internal_node(pair[1]);
        }
        self.mark_ui_dirty(
            tree_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        for id in internal_rows
            .into_iter()
            .chain(internal_toggles)
            .chain(internal_icons)
            .chain(internal_labels)
            .chain(internal_lines.into_iter().flat_map(|pair| pair.into_iter()))
        {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn sync_tree_list_line(
        &mut self,
        id: NodeID,
        visible: bool,
        last_child: bool,
        x: f32,
        row_height: f32,
        line_width: f32,
        line_color: Color,
        vertical: bool,
    ) {
        if let Some(node) = self.nodes.get_mut_untracked(id)
            && let SceneNodeData::UiPanel(panel) = &mut node.data
        {
            panel.base.visible = visible && line_width > 0.0;
            panel.style.fill = line_color;
            if vertical {
                panel.base.visible = panel.base.visible && !last_child;
                panel.base.layout.size = UiVector2::pixels(line_width, row_height);
                panel.base.transform.position = UiVector2::pixels(x + 5.0, 0.0);
            } else {
                panel.base.layout.size = UiVector2::pixels(8.0, line_width);
                panel.base.transform.position = UiVector2::pixels(x + 9.0, 0.0);
            }
        }
    }

    pub(super) fn update_dropdown_open_animations(&mut self) -> bool {
        let dt = self.time.delta.max(0.0);
        let ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                matches!(node.data, SceneNodeData::UiDropdown(_)).then_some(id)
            })
            .collect::<Vec<_>>();
        let mut changed = false;
        for id in ids {
            let mut node_changed = false;
            if let Some(node) = self.nodes.get_mut_untracked(id)
                && let SceneNodeData::UiDropdown(dropdown) = &mut node.data
            {
                if dropdown.open != dropdown.was_open {
                    dropdown.was_open = dropdown.open;
                    dropdown.open_animation_progress = if dropdown.open
                        && matches!(dropdown.open_animation, UiDropdownOpenAnimation::Pop)
                    {
                        1.0
                    } else {
                        0.0
                    };
                    node_changed = true;
                }
                if dropdown.open
                    && matches!(dropdown.open_animation, UiDropdownOpenAnimation::Extend)
                    && dropdown.open_animation_progress < 1.0
                {
                    let duration = dropdown.open_animation_duration;
                    dropdown.open_animation_progress = if duration <= 0.0 {
                        1.0
                    } else {
                        (dropdown.open_animation_progress + dt / duration).min(1.0)
                    };
                    node_changed = true;
                }
            }
            if node_changed {
                changed = true;
                self.mark_ui_dirty(id, Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS);
            }
        }
        changed
    }

    pub(super) fn ensure_dropdown_internal_nodes(&mut self) {
        // Same gating as tree lists: only re-sync dropdowns whose node is
        // dirty this frame. Engine-side open/select paths call
        // sync_dropdown_internal_nodes directly.
        let dropdown_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((id, dropdown.internal_label.is_nil())),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (dropdown_id, never_synced) in dropdown_ids {
            if never_synced || self.dirty.ui_flags_at(dropdown_id.index() as usize) != 0 {
                self.ensure_dropdown_internal_nodes_for(dropdown_id);
                self.sync_dropdown_internal_nodes(dropdown_id);
            }
        }
    }

    pub(super) fn ensure_dropdown_internal_nodes_for(&mut self, dropdown_id: NodeID) {
        let Some((label_id, popup_id, mut option_buttons, mut option_labels, option_count)) = self
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.internal_label,
                    dropdown.internal_popup_panel,
                    dropdown.internal_option_buttons.clone(),
                    dropdown.internal_option_labels.clone(),
                    dropdown.options.len(),
                )),
                _ => None,
            })
        else {
            return;
        };

        let mut label_id = label_id;
        let mut popup_id = popup_id;
        if !self.dropdown_internal_valid(label_id, dropdown_id, "label") {
            label_id = self.insert_dropdown_label(dropdown_id, "__perro_dropdown_label");
        }
        if !self.dropdown_internal_valid(popup_id, dropdown_id, "panel") {
            popup_id = self.insert_dropdown_popup_panel(dropdown_id);
        }
        for id in option_buttons.iter().copied().skip(option_count) {
            if let Some(node) = self.nodes.get_mut_untracked(id)
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = false;
            }
        }
        for id in option_labels.iter().copied().skip(option_count) {
            if let Some(node) = self.nodes.get_mut_untracked(id)
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                label.base.visible = false;
            }
        }
        // Keep the high-water allocation. Shrink only hides unused nodes, so
        // later growth reuses the same arena slots, schedules, names, and
        // parent-child links.
        if option_buttons.len() < option_count {
            option_buttons.resize(option_count, NodeID::nil());
        }
        if option_labels.len() < option_count {
            option_labels.resize(option_count, NodeID::nil());
        }
        for idx in 0..option_count {
            if !self.dropdown_internal_valid(option_buttons[idx], popup_id, "button") {
                option_buttons[idx] = self.insert_dropdown_option_button(popup_id, idx);
            }
            if !self.dropdown_internal_valid(option_labels[idx], option_buttons[idx], "label") {
                option_labels[idx] = self
                    .insert_dropdown_label(option_buttons[idx], "__perro_dropdown_option_label");
            }
        }

        if let Some(node) = self.nodes.get_mut_untracked(dropdown_id)
            && let SceneNodeData::UiDropdown(dropdown) = &mut node.data
        {
            dropdown.internal_label = label_id;
            dropdown.internal_popup_panel = popup_id;
            dropdown.internal_option_buttons = option_buttons;
            dropdown.internal_option_labels = option_labels;
        }
    }

    pub(super) fn dropdown_internal_valid(&self, id: NodeID, parent: NodeID, kind: &str) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && match kind {
                    "button" => matches!(node.data, SceneNodeData::UiButton(_)),
                    "label" => matches!(node.data, SceneNodeData::UiLabel(_)),
                    "panel" => matches!(node.data, SceneNodeData::UiPanel(_)),
                    _ => false,
                }
        })
    }

    pub(super) fn insert_dropdown_popup_panel(&mut self, dropdown_id: NodeID) -> NodeID {
        let mut panel = UiPanel::new();
        panel.base.layout.z_index = 100;
        panel.base.clip_children = true;
        self.insert_color_picker_internal_node(
            dropdown_id,
            "__perro_dropdown_popup",
            SceneNodeData::UiPanel(Box::new(panel)),
        )
    }

    pub(super) fn insert_dropdown_option_button(&mut self, popup_id: NodeID, idx: usize) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.z_index = 100;
        button.base.clip_children = false;
        self.insert_color_picker_internal_node(
            popup_id,
            format!("__perro_dropdown_option_{idx}"),
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    pub(super) fn insert_dropdown_label(
        &mut self,
        parent_id: NodeID,
        name: &'static str,
    ) -> NodeID {
        let mut label = perro_ui::UiLabel::new();
        label.base.layout.z_index = 101;
        label.base.input_enabled = false;
        label.base.mouse_filter = perro_ui::UiMouseFilter::Pass;
        label.base.layout.size = UiVector2::percent(100.0, 100.0);
        label.text_size_ratio = 0.55;
        label.base.layout.padding = perro_ui::UiRect::symmetric(6.0, 2.0);
        label.h_align = perro_ui::UiTextAlign::Start;
        self.insert_color_picker_internal_node(
            parent_id,
            name,
            SceneNodeData::UiLabel(Box::new(label)),
        )
    }

    pub(super) fn sync_dropdown_internal_nodes(&mut self, dropdown_id: NodeID) {
        let Some(snapshot) = self
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.selected_label().to_string(),
                    dropdown.open,
                    dropdown.button.base.visible,
                    dropdown.option_height,
                    dropdown.popup_size,
                    dropdown.popup_offset,
                    dropdown.popup_direction,
                    dropdown.open_animation,
                    dropdown.open_animation_progress,
                    dropdown.option_style.clone(),
                    dropdown.option_hover_style.clone(),
                    dropdown.option_pressed_style.clone(),
                    dropdown.popup_style.clone(),
                    dropdown
                        .options
                        .iter()
                        .map(|option| option.label.to_string())
                        .collect::<Vec<_>>(),
                    dropdown.internal_label,
                    dropdown.internal_popup_panel,
                    dropdown.internal_option_buttons.clone(),
                    dropdown.internal_option_labels.clone(),
                )),
                _ => None,
            })
        else {
            return;
        };
        let (
            selected,
            open,
            base_visible,
            option_height,
            popup_size,
            popup_offset,
            popup_direction,
            open_animation,
            open_animation_progress,
            option_style,
            option_hover_style,
            option_pressed_style,
            popup_style,
            labels,
            label_id,
            popup_id,
            option_buttons,
            option_labels,
        ) = snapshot;
        if let Some(node) = self.nodes.get_mut_untracked(label_id)
            && let SceneNodeData::UiLabel(label) = &mut node.data
        {
            label.base.visible = base_visible;
            label.set_text(selected);
        }
        let full_popup_height = if popup_size[1] > 0.0 {
            popup_size[1]
        } else {
            option_height * labels.len() as f32
        };
        let progress = if matches!(open_animation, UiDropdownOpenAnimation::Extend) {
            open_animation_progress
        } else {
            1.0
        };
        let mut popup_width = if popup_size[0] > 0.0 {
            UiUnit::Pixels(popup_size[0])
        } else {
            UiUnit::Percent(100.0)
        };
        if matches!(
            popup_direction,
            UiDropdownDirection::Left | UiDropdownDirection::Right
        ) && matches!(open_animation, UiDropdownOpenAnimation::Extend)
        {
            popup_width = match popup_width {
                UiUnit::Pixels(value) => UiUnit::Pixels(value * progress),
                UiUnit::Percent(value) => UiUnit::Percent(value * progress),
            };
        }
        let popup_height = if matches!(
            popup_direction,
            UiDropdownDirection::Left | UiDropdownDirection::Right
        ) {
            full_popup_height
        } else {
            full_popup_height * progress
        };
        if let Some(node) = self.nodes.get_mut_untracked(popup_id)
            && let SceneNodeData::UiPanel(panel) = &mut node.data
        {
            panel.base.visible = open && base_visible;
            panel.base.clip_children = true;
            panel.base.layout.size = UiVector2::new(popup_width, UiUnit::Pixels(popup_height));
            panel.base.layout.z_index = 100;
            panel.style = popup_style;
            panel.base.layout.anchor = UiAnchor::Center;
            panel.base.transform.position = UiVector2::pixels(popup_offset[0], popup_offset[1]);
            panel.base.transform.translation = Vector2::ZERO;
            panel.base.transform.self_translation = Vector2::ZERO;
            match popup_direction {
                UiDropdownDirection::Down => {
                    panel.base.transform.translation.y = 0.5;
                    panel.base.transform.self_translation.y = 0.5;
                }
                UiDropdownDirection::Up => {
                    panel.base.transform.translation.y = -0.5;
                    panel.base.transform.self_translation.y = -0.5;
                }
                UiDropdownDirection::Left => {
                    panel.base.transform.translation.x = -0.5;
                    panel.base.transform.self_translation.x = -0.5;
                }
                UiDropdownDirection::Right => {
                    panel.base.transform.translation.x = 0.5;
                    panel.base.transform.self_translation.x = 0.5;
                }
            }
        }
        for (idx, button_id) in option_buttons.iter().copied().enumerate() {
            let active = idx < labels.len();
            if let Some(node) = self.nodes.get_mut_untracked(button_id)
                && let SceneNodeData::UiButton(button) = &mut node.data
            {
                button.base.visible = active && open && base_visible;
                button.base.layout.size = UiVector2::new(
                    perro_ui::UiUnit::Percent(100.0),
                    perro_ui::UiUnit::Pixels(option_height),
                );
                match popup_direction {
                    UiDropdownDirection::Down => {
                        button.base.transform.position =
                            UiVector2::pixels(0.0, option_height * idx as f32);
                        button.base.layout.anchor = UiAnchor::Bottom;
                    }
                    UiDropdownDirection::Up => {
                        button.base.transform.position =
                            UiVector2::pixels(0.0, -option_height * idx as f32);
                        button.base.layout.anchor = UiAnchor::Top;
                    }
                    UiDropdownDirection::Left | UiDropdownDirection::Right => {
                        button.base.transform.position =
                            UiVector2::pixels(0.0, -option_height * idx as f32);
                        button.base.layout.anchor = UiAnchor::Top;
                    }
                }
                button.base.layout.z_index = 1 + idx as i32;
                button.style = option_style.clone();
                button.hover_style = option_hover_style.clone();
                button.pressed_style = option_pressed_style.clone();
            }
            if let Some(node) = self
                .nodes
                .get_mut_untracked(option_labels.get(idx).copied().unwrap_or_default())
                && let SceneNodeData::UiLabel(label) = &mut node.data
            {
                label.base.visible = active && open && base_visible;
                if let Some(text) = labels.get(idx) {
                    label.set_text(text.clone());
                }
            }
        }
        self.mark_ui_dirty(
            dropdown_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        let mut dirty_ids = vec![label_id];
        dirty_ids.extend(option_buttons);
        dirty_ids.extend(option_labels);
        for id in dirty_ids {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    pub(super) fn ensure_color_picker_internal_nodes(&mut self) {
        // Same gating as tree lists: only re-sync pickers whose node is
        // dirty this frame. Engine-side popup/edit paths call
        // sync_color_picker_internal_nodes directly.
        let picker_ids = self
            .nodes
            .iter()
            .filter_map(|(id, node)| match &node.data {
                SceneNodeData::UiColorPicker(picker) => {
                    Some((id, picker.internal_swatch_button.is_nil()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (picker_id, never_synced) in picker_ids {
            if never_synced || self.dirty.ui_flags_at(picker_id.index() as usize) != 0 {
                self.ensure_color_picker_internal_nodes_for(picker_id);
                self.sync_color_picker_internal_nodes(picker_id);
            }
        }
    }

    pub(super) fn ensure_color_picker_internal_nodes_for(&mut self, picker_id: NodeID) {
        let Some(snapshot) = self.nodes.get(picker_id).and_then(|node| match &node.data {
            SceneNodeData::UiColorPicker(picker) => Some((
                picker.internal_swatch_button,
                picker.internal_popup_panel,
                picker.internal_rgba_boxes,
                picker.internal_hsv_boxes,
                picker.internal_hex_box,
            )),
            _ => None,
        }) else {
            return;
        };

        let mut ids = snapshot;
        if !self.color_picker_internal_valid(ids.0, picker_id, false) {
            ids.0 = self.insert_color_picker_swatch(picker_id);
        }
        if !self.color_picker_internal_valid(ids.1, picker_id, false) {
            ids.1 = self.insert_color_picker_popup(picker_id);
        }
        for idx in 0..ids.2.len() {
            if !self.color_picker_internal_valid(ids.2[idx], ids.1, true) {
                ids.2[idx] = self.insert_color_picker_text_box(
                    ids.1,
                    [
                        "__perro_color_rgba_r",
                        "__perro_color_rgba_g",
                        "__perro_color_rgba_b",
                        "__perro_color_rgba_a",
                    ][idx],
                );
            }
        }
        for idx in 0..ids.3.len() {
            if !self.color_picker_internal_valid(ids.3[idx], ids.1, true) {
                ids.3[idx] = self.insert_color_picker_text_box(
                    ids.1,
                    [
                        "__perro_color_hsv_h",
                        "__perro_color_hsv_s",
                        "__perro_color_hsv_v",
                    ][idx],
                );
            }
        }
        if !self.color_picker_internal_valid(ids.4, ids.1, true) {
            ids.4 = self.insert_color_picker_text_box(ids.1, "__perro_color_hex");
        }

        if let Some(node) = self.nodes.get_mut_untracked(picker_id)
            && let SceneNodeData::UiColorPicker(picker) = &mut node.data
        {
            picker.internal_swatch_button = ids.0;
            picker.internal_popup_panel = ids.1;
            picker.internal_rgba_boxes = ids.2;
            picker.internal_hsv_boxes = ids.3;
            picker.internal_hex_box = ids.4;
        }
    }

    pub(super) fn color_picker_internal_valid(
        &self,
        id: NodeID,
        parent: NodeID,
        nested: bool,
    ) -> bool {
        if id.is_nil() {
            return false;
        }
        self.nodes.get(id).is_some_and(|node| {
            node.parent == parent
                && if nested {
                    matches!(node.data, SceneNodeData::UiTextBox(_))
                } else {
                    matches!(
                        node.data,
                        SceneNodeData::UiButton(_) | SceneNodeData::UiPanel(_)
                    )
                }
        })
    }

    pub(super) fn insert_color_picker_swatch(&mut self, picker_id: NodeID) -> NodeID {
        let mut button = UiButton::new();
        button.base.layout.size = UiVector2::percent(100.0, 100.0);
        button.base.layout.z_index = 1;
        button.base.clip_children = false;
        button.style.set_corner_radius(0.15);
        button.hover_style.set_corner_radius(0.15);
        button.pressed_style.set_corner_radius(0.15);
        self.insert_color_picker_internal_node(
            picker_id,
            "__perro_color_picker_swatch",
            SceneNodeData::UiButton(Box::new(button)),
        )
    }

    pub(super) fn insert_color_picker_popup(&mut self, picker_id: NodeID) -> NodeID {
        let mut panel = UiPanel::new();
        panel.base.layout.anchor = UiAnchor::Bottom;
        panel.base.layout.z_index = 100;
        panel.base.clip_children = false;
        self.insert_color_picker_internal_node(
            picker_id,
            "__perro_color_picker_popup",
            SceneNodeData::UiPanel(Box::new(panel)),
        )
    }

    pub(super) fn insert_color_picker_text_box(
        &mut self,
        popup_id: NodeID,
        name: &'static str,
    ) -> NodeID {
        let mut text_box = UiTextBox::new();
        text_box.inner.base.layout.z_index = 102;
        text_box.inner.font_size = 14.0;
        text_box.inner.text_size_ratio = 0.55;
        text_box.inner.padding = perro_ui::UiRect::symmetric(6.0, 3.0);
        self.insert_color_picker_internal_node(
            popup_id,
            name,
            SceneNodeData::UiTextBox(Box::new(text_box)),
        )
    }

    pub(super) fn insert_color_picker_internal_node(
        &mut self,
        parent_id: NodeID,
        name: impl Into<std::borrow::Cow<'static, str>>,
        data: SceneNodeData,
    ) -> NodeID {
        let mut node = SceneNode::new(data);
        node.set_name(name);
        node.parent = parent_id;
        let id = self.nodes.insert(node);
        if self
            .nodes
            .children(parent_id)
            .is_some_and(|children| !children.contains(&id))
        {
            self.nodes.push_child(parent_id, id);
        }
        if let Some(node) = self.nodes.get(id) {
            self.register_internal_node_schedules(id, node.node_type());
        }
        self.mark_needs_rerender(id);
        self.mark_ui_dirty(
            id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_LAYOUT_PARENT | Self::UI_DIRTY_COMMANDS,
        );
        self.mark_ui_dirty(
            parent_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_LAYOUT_PARENT | Self::UI_DIRTY_COMMANDS,
        );
        id
    }

    pub(super) fn sync_color_picker_internal_nodes(&mut self, picker_id: NodeID) {
        let Some((color, popup_open, popup_style, popup_size, wheel_radius, visibility, ids)) =
            self.nodes.get(picker_id).and_then(|node| match &node.data {
                SceneNodeData::UiColorPicker(picker) => Some((
                    picker.color,
                    picker.popup_open,
                    picker.popup_style.clone(),
                    picker.popup_size,
                    picker.wheel_radius,
                    (
                        picker.show_selector,
                        picker.show_rgba,
                        picker.show_hsl,
                        picker.show_hex,
                    ),
                    (
                        picker.internal_swatch_button,
                        picker.internal_popup_panel,
                        picker.internal_rgba_box,
                        picker.internal_hsv_box,
                        picker.internal_rgba_boxes,
                        picker.internal_hsv_boxes,
                        picker.internal_hex_box,
                    ),
                )),
                _ => None,
            })
        else {
            return;
        };
        let layout = color_picker_layout(
            popup_size,
            wheel_radius,
            visibility.0,
            visibility.1,
            visibility.2,
            visibility.3,
        );
        let popup_size = layout.popup_size;

        if let Some(node) = self.nodes.get_mut_untracked(ids.0)
            && let SceneNodeData::UiButton(button) = &mut node.data
        {
            button.base.visible = true;
            button.style.fill = color;
            button.hover_style.fill = color;
            button.pressed_style.fill = Color::new(
                (color.r() * 0.8).clamp(0.0, 1.0),
                (color.g() * 0.8).clamp(0.0, 1.0),
                (color.b() * 0.8).clamp(0.0, 1.0),
                color.a(),
            );
        }
        if let Some(node) = self.nodes.get_mut_untracked(ids.1)
            && let SceneNodeData::UiPanel(panel) = &mut node.data
        {
            panel.base.visible = popup_open;
            panel.base.layout.size = UiVector2::pixels(popup_size[0], popup_size[1]);
            panel.base.transform.position = UiVector2::pixels(0.0, -popup_size[1] - 8.0);
            panel.style = popup_style;
        }
        let rgba = color_to_rgba_components(color);
        let hsl = color_to_hsl_components(color);
        let hex = color_to_hex_text(color);
        self.sync_color_picker_legacy_text_box(ids.2, false);
        self.sync_color_picker_legacy_text_box(ids.3, false);
        for (idx, text) in rgba.iter().enumerate() {
            self.sync_color_picker_component_box(
                ids.4[idx],
                popup_open && visibility.1,
                ColorPickerComponentLayout::new(popup_size, layout.rgba_y, idx, rgba.len()),
                text,
            );
        }
        for (idx, text) in hsl.iter().enumerate() {
            self.sync_color_picker_component_box(
                ids.5[idx],
                popup_open && visibility.2,
                ColorPickerComponentLayout::new(popup_size, layout.hsl_y, idx, hsl.len()),
                text,
            );
        }
        self.sync_color_picker_text_box(
            ids.6,
            popup_open && visibility.3,
            popup_size,
            layout.hex_y,
            &hex,
        );

        self.mark_ui_dirty(
            picker_id,
            Self::UI_DIRTY_LAYOUT_SELF | Self::UI_DIRTY_COMMANDS,
        );
        let mut dirty_ids = vec![ids.0, ids.1, ids.2, ids.3, ids.6];
        dirty_ids.extend(ids.4);
        dirty_ids.extend(ids.5);
        for id in dirty_ids {
            if !id.is_nil() {
                self.mark_ui_dirty(
                    id,
                    Self::UI_DIRTY_LAYOUT_SELF
                        | Self::UI_DIRTY_LAYOUT_PARENT
                        | Self::UI_DIRTY_COMMANDS,
                );
            }
        }
    }

    pub(super) fn sync_color_picker_legacy_text_box(&mut self, node_id: NodeID, visible: bool) {
        if let Some(node) = self.nodes.get_mut_untracked(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            text_box.inner.base.visible = visible;
        }
    }

    pub(super) fn sync_color_picker_component_box(
        &mut self,
        node_id: NodeID,
        visible: bool,
        layout: ColorPickerComponentLayout,
        text: &str,
    ) {
        if let Some(node) = self.nodes.get_mut_untracked(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            let gap = 6.0;
            let total_gap = gap * layout.cols.saturating_sub(1) as f32;
            let width =
                ((layout.popup_size[0] - 24.0 - total_gap) / layout.cols.max(1) as f32).max(36.0);
            let left = -layout.popup_size[0] * 0.5 + 12.0 + width * 0.5;
            let x = left + layout.col as f32 * (width + gap);
            text_box.inner.base.visible = visible;
            text_box.inner.base.layout.size = UiVector2::pixels(width, 30.0);
            text_box.inner.base.transform.position =
                UiVector2::pixels(x, layout.popup_size[1] * 0.5 - layout.y_from_top);
            if self.render_ui.focused_text_edit != Some(node_id) {
                text_box.inner.set_text(text.to_string());
            }
        }
    }

    pub(super) fn sync_color_picker_text_box(
        &mut self,
        node_id: NodeID,
        visible: bool,
        popup_size: [f32; 2],
        y_from_top: f32,
        text: &str,
    ) {
        if let Some(node) = self.nodes.get_mut_untracked(node_id)
            && let SceneNodeData::UiTextBox(text_box) = &mut node.data
        {
            text_box.inner.base.visible = visible;
            text_box.inner.base.layout.size =
                UiVector2::pixels((popup_size[0] - 24.0).max(48.0), 30.0);
            text_box.inner.base.transform.position =
                UiVector2::pixels(0.0, popup_size[1] * 0.5 - y_from_top);
            if self.render_ui.focused_text_edit != Some(node_id) {
                text_box.inner.set_text(text.to_string());
            }
        }
    }

    pub(super) fn emit_color_picker_wheel_commands(
        &mut self,
        computed: &AHashMap<NodeID, ComputedUiRect>,
        viewport: Vector2,
    ) {
        let pickers = self
            .nodes
            .iter()
            .filter_map(|(id, node)| {
                let SceneNodeData::UiColorPicker(picker) = &node.data else {
                    return None;
                };
                Some((
                    id,
                    picker.popup_open,
                    picker.wheel_radius,
                    picker.picker_mode,
                    picker.color,
                    picker.show_selector,
                    picker.popup_size,
                    picker.show_rgba,
                    picker.show_hsl,
                    picker.show_hex,
                    picker.internal_popup_panel,
                ))
            })
            .collect::<Vec<_>>();
        for (
            picker_id,
            popup_open,
            wheel_radius,
            picker_mode,
            color,
            show_selector,
            popup_size,
            show_rgba,
            show_hsl,
            show_hex,
            popup_id,
        ) in pickers
        {
            let wheel_node = color_picker_wheel_render_node(picker_id);
            if !popup_open || !show_selector {
                self.queue_render_command(RenderCommand::Ui(UiCommand::RemoveNode {
                    node: wheel_node,
                }));
                continue;
            }
            let Some(popup_rect) = computed.get(&popup_id).copied().or_else(|| {
                self.render_ui
                    .retained_rects
                    .get(&popup_id)
                    .copied()
                    .map(|rect| computed_rect_from_state(&rect))
            }) else {
                continue;
            };
            let layout = color_picker_layout(
                popup_size,
                wheel_radius,
                show_selector,
                show_rgba,
                show_hsl,
                show_hex,
            );
            let rect = color_picker_wheel_rect(popup_rect, wheel_radius, layout.selector_y);
            let rect_state = UiRectState {
                center: [rect.center.x, rect.center.y],
                size: [rect.size.x, rect.size.y],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: self.ui_effective_z(popup_id).saturating_add(1),
            };
            self.queue_render_command(RenderCommand::Ui(UiCommand::UpsertColorWheel {
                node: wheel_node,
                rect: rect_state,
                clip_rect: self.ui_effective_clip_rect_screen(popup_id, computed, viewport),
                mode: picker_mode,
                selected: color.to_rgba(),
            }));
        }
    }

    pub(super) fn color_picker_parent_for_swatch(&self, swatch_id: NodeID) -> Option<NodeID> {
        self.nodes.iter().find_map(|(id, node)| match &node.data {
            SceneNodeData::UiColorPicker(picker) if picker.internal_swatch_button == swatch_id => {
                Some(id)
            }
            _ => None,
        })
    }

    pub(super) fn process_color_picker_text_edit(
        &mut self,
        text_node: NodeID,
        text: &str,
        command_ids: &mut Vec<NodeID>,
        command_seen: &mut ahash::AHashSet<NodeID>,
    ) {
        let Some((picker_id, field, current)) = self.nodes.iter().find_map(|(id, node)| {
            let SceneNodeData::UiColorPicker(picker) = &node.data else {
                return None;
            };
            if let Some(idx) = picker
                .internal_rgba_boxes
                .iter()
                .position(|box_id| *box_id == text_node)
            {
                Some((id, ColorPickerTextField::Rgba(idx), picker.color))
            } else if let Some(idx) = picker
                .internal_hsv_boxes
                .iter()
                .position(|box_id| *box_id == text_node)
            {
                Some((id, ColorPickerTextField::Hsv(idx), picker.color))
            } else if picker.internal_hex_box == text_node {
                Some((id, ColorPickerTextField::Hex, picker.color))
            } else {
                None
            }
        }) else {
            return;
        };
        let Some(color) = parse_color_picker_text(field, text, current) else {
            return;
        };
        let Some(scene_node) = self.nodes.get_mut_untracked(picker_id) else {
            return;
        };
        let SceneNodeData::UiColorPicker(picker) = &mut scene_node.data else {
            return;
        };
        if picker.color == color {
            return;
        }
        let signals = picker.color_changed_signals.clone();
        picker.color = color;
        self.sync_color_picker_internal_nodes(picker_id);
        if command_seen.insert(picker_id) {
            command_ids.push(picker_id);
        }
        let params = [
            Variant::from(picker_id),
            Variant::from(color.r()),
            Variant::from(color.g()),
            Variant::from(color.b()),
            Variant::from(color.a()),
        ];
        for signal in signals {
            self.queue_ui_signal(signal, &params);
        }
    }
}
