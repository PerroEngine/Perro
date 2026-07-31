use super::*;

impl Runtime {
    pub(super) fn button_2d_input_changed(&self) -> bool {
        let pointer = (
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        );
        self.render_2d.last_button_pointer != Some(pointer)
            || self.input.is_mouse_pressed(MouseButton::Left)
            || self.input.is_mouse_released(MouseButton::Left)
    }

    pub(super) fn refresh_button_2d_visual_states(&mut self, hovered: Option<NodeID>) {
        let mouse_down = self.input.is_mouse_down(MouseButton::Left);
        let mut next_states = std::mem::take(&mut self.render_ui.button_states);
        next_states.retain(|node, _| self.nodes.get(*node).is_some());
        let mut events = Vec::new();
        let button_count = self.internal_updates.button_nodes_2d.len();
        for i in 0..button_count {
            let node = self.internal_updates.button_nodes_2d[i];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let Some(inactive) = button_2d_inactive_from_data(&scene_node.data) else {
                continue;
            };
            let next = if inactive
                || !self.is_effectively_visible(node)
                || self.is_under_sub_view(node)
                || Some(node) != hovered
            {
                UiButtonVisualState::Neutral
            } else if mouse_down {
                UiButtonVisualState::Pressed
            } else {
                UiButtonVisualState::Hover
            };
            let prev = next_states.insert(node, next).unwrap_or_default();
            if !inactive {
                collect_button_2d_events(node, prev, next, &mut events);
            }
        }
        self.render_ui.button_states = next_states;
        let cursor_icon = hovered
            .and_then(|node| self.nodes.get(node))
            .and_then(|scene_node| button_2d_cursor_icon(&scene_node.data))
            .unwrap_or(perro_ui::CursorIcon::Default);
        self.set_render_cursor_icon_2d(cursor_icon);
        self.render_2d.last_button_pointer = Some((
            self.input.mouse_position(),
            self.input.is_mouse_down(MouseButton::Left),
        ));
        self.emit_button_2d_events(&events);
    }

    pub(super) fn hovered_button_2d(
        &mut self,
        camera: Option<&Camera2DState>,
        camera_render_mask: BitMask,
    ) -> Option<NodeID> {
        let world = self.pointer_world_2d(camera);
        let mut best: Option<(NodeID, i32)> = None;
        let button_count = self.internal_updates.button_nodes_2d.len();
        for i in 0..button_count {
            let node = self.internal_updates.button_nodes_2d[i];
            let Some(scene_node) = self.nodes.get(node) else {
                continue;
            };
            let Some(hit) = button_2d_hit_data(&scene_node.data) else {
                continue;
            };
            let Button2DHitData {
                visible,
                size,
                z_index,
                render_layers,
                input_enabled,
                mouse_filter,
                input_mask,
            } = hit;
            let input_accepted = self.ui_input_mask_accepts_kbm_2d(input_mask);
            if !visible
                || !input_enabled
                || !input_accepted
                || !self.is_effectively_visible(node)
                || self.is_under_sub_view(node)
                || !render_mask_matches(camera_render_mask, render_layers)
                || !matches!(
                    mouse_filter,
                    perro_ui::UiMouseFilter::Stop | perro_ui::UiMouseFilter::Pass
                )
            {
                continue;
            }
            let Some(local) = self.button_2d_local_point(node, world) else {
                continue;
            };
            let half = size * 0.5;
            if local.x.abs() > half.x || local.y.abs() > half.y {
                continue;
            }
            match best {
                Some((best_node, best_z))
                    if best_z > z_index
                        || (best_z == z_index && best_node.as_u64() > node.as_u64()) => {}
                _ => best = Some((node, z_index)),
            }
        }
        best.map(|(node, _)| node)
    }

    pub(super) fn pointer_world_2d(&self, camera: Option<&Camera2DState>) -> Vector2 {
        let mouse = self.input.mouse_position();
        let viewport = self.input.viewport_size();
        // The renderer maps world -> screen with the aspect-fit virtual-canvas
        // scale min(w/vw, h/vh) times zoom; picking must invert the same rule
        // or hitboxes drift as soon as the window is not the design size.
        let virtual_scale = self.ui_virtual_font_scale(viewport);
        let screen = Vector2::new(
            (mouse.x - 0.5) * viewport.x / virtual_scale,
            (mouse.y - 0.5) * viewport.y / virtual_scale,
        );
        let Some(camera) = camera else {
            return screen;
        };
        // Match the renderer's zoom coercion: non-finite / non-positive -> 1.
        let zoom = if camera.zoom.is_finite() && camera.zoom > 0.0 {
            camera.zoom
        } else {
            1.0
        };
        let x = screen.x / zoom;
        let y = screen.y / zoom;
        let sin = camera.rotation_radians.sin();
        let cos = camera.rotation_radians.cos();
        Vector2::new(
            camera.position[0] + x * cos - y * sin,
            camera.position[1] + x * sin + y * cos,
        )
    }

    pub(super) fn button_2d_local_point(
        &mut self,
        node: NodeID,
        world: Vector2,
    ) -> Option<Vector2> {
        let transform = self.get_render_global_transform_2d(node)?;
        let local = transform.to_mat3().inverse() * glam::Vec3::new(world.x, world.y, 1.0);
        Some(Vector2::new(local.x, local.y))
    }

    pub(super) fn ui_input_mask_accepts_kbm_2d(&self, mask: &perro_ui::UiInputMask) -> bool {
        if mask.deny_kbm {
            return false;
        }
        !mask.has_allow_filter() || mask.allow_kbm
    }

    pub(super) fn emit_button_2d_events(&mut self, events: &[(NodeID, &'static str)]) {
        for &(node, event) in events {
            if event == "click" {
                self.process_button_2d_web_action(node);
            }
            let signals = self.button_2d_event_signals(node, event);
            if signals.is_empty() {
                continue;
            }
            let params = [Variant::from(node)];
            for signal in signals {
                self.queue_ui_signal(signal, &params);
            }
        }
    }

    pub(super) fn process_button_2d_web_action(&mut self, node: NodeID) {
        let Some(scene_node) = self.nodes.get(node) else {
            return;
        };
        let web = match &scene_node.data {
            SceneNodeData::Button2D(button) => button.web.as_ref(),
            SceneNodeData::ImageButton2D(button) => button.web.as_ref(),
            SceneNodeData::NineSliceButton2D(button) => button.web.as_ref(),
            _ => None,
        };
        if let Some(web) = web {
            let _ = perro_web::push_route(web.href.as_ref());
        }
    }

    pub(super) fn button_2d_event_signals(&mut self, node: NodeID, event: &str) -> Vec<SignalID> {
        let Some(scene_node) = self.nodes.get(node) else {
            return Vec::new();
        };
        let Some(custom) = button_2d_custom_event_signals(&scene_node.data, event) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(1 + custom.len());
        let name = scene_node.name.as_ref();
        if !name.is_empty() {
            self.render_ui.event_signal_name_scratch.clear();
            self.render_ui.event_signal_name_scratch.push_str(name);
            self.render_ui.event_signal_name_scratch.push('_');
            self.render_ui
                .event_signal_name_scratch
                .push_str(button_2d_named_event(event));
            out.push(SignalID::from_string(
                &self.render_ui.event_signal_name_scratch,
            ));
        }
        out.extend(custom.iter().copied());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_world_2d_inverts_renderer_virtual_scale() {
        let mut runtime = Runtime::new();
        runtime.project = Some(std::rc::Rc::new(
            crate::runtime_project::RuntimeProject::new("Test", "."),
        ));
        runtime.set_viewport_size(3440, 1440);
        // Window px; normalized mouse = (0.75, 0.75) (y flipped upward).
        runtime.set_mouse_position(2580.0, 360.0);

        let camera = Camera2DState {
            position: [100.0, -50.0],
            rotation_radians: 0.0,
            zoom: 2.0,
            ..Camera2DState::default()
        };
        let world = runtime.pointer_world_2d(Some(&camera));

        // Renderer world->screen scale = min(3440/1920, 1440/1080) * zoom.
        let virtual_scale = (3440.0_f32 / 1920.0).min(1440.0 / 1080.0);
        let screen = Vector2::new((0.75 - 0.5) * 3440.0, (0.75 - 0.5) * 1440.0);
        let expected = Vector2::new(
            camera.position[0] + screen.x / (virtual_scale * camera.zoom),
            camera.position[1] + screen.y / (virtual_scale * camera.zoom),
        );
        assert!(
            (world.x - expected.x).abs() < 1e-3,
            "{world:?} vs {expected:?}"
        );
        assert!(
            (world.y - expected.y).abs() < 1e-3,
            "{world:?} vs {expected:?}"
        );

        // Roundtrip world -> screen with the renderer rule lands on the cursor.
        let back = Vector2::new(
            (world.x - camera.position[0]) * virtual_scale * camera.zoom,
            (world.y - camera.position[1]) * virtual_scale * camera.zoom,
        );
        assert!((back.x - screen.x).abs() < 1e-3);
        assert!((back.y - screen.y).abs() < 1e-3);
    }

    #[test]
    fn pointer_world_2d_is_identity_at_design_resolution() {
        let mut runtime = Runtime::new();
        runtime.project = Some(std::rc::Rc::new(
            crate::runtime_project::RuntimeProject::new("Test", "."),
        ));
        runtime.set_viewport_size(1920, 1080);
        runtime.set_mouse_position(1920.0, 540.0);
        let camera = Camera2DState::default();
        // zoom <= 0 coerces to 1.0 like the renderer, so the design-res
        // mapping is exactly screen px from center.
        let world = runtime.pointer_world_2d(Some(&camera));
        assert!((world.x - 960.0).abs() < 1e-3, "{world:?}");
        assert!(world.y.abs() < 1e-3, "{world:?}");
    }
}
