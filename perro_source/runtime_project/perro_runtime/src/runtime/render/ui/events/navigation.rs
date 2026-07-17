use super::*;

impl Runtime {
    pub(super) fn ui_gamepad_dpad_direction(&self) -> Option<UiDirectionalNav> {
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            let source = UiInputSource::Gamepad(index);
            if gamepad.is_button_pressed(GamepadButton::DpadUp) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [0, 1],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadDown) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [0, -1],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadLeft) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [-1, 0],
                });
            }
            if gamepad.is_button_pressed(GamepadButton::DpadRight) {
                return Some(UiDirectionalNav {
                    source,
                    dir: [1, 0],
                });
            }
        }
        None
    }

    pub(super) fn ui_repeating_stick_nav_direction(&mut self) -> Option<UiDirectionalNav> {
        let nav = self.ui_stick_nav_direction();
        let Some(nav) = nav else {
            if !self.ui_any_nav_stick_held() {
                self.render_ui.ui_nav_repeat_dir = None;
                self.render_ui.ui_nav_repeat_timer = 0.0;
            }
            return None;
        };
        if self.render_ui.ui_nav_repeat_dir != Some(nav.dir) {
            self.render_ui.ui_nav_repeat_dir = Some(nav.dir);
            self.render_ui.ui_nav_repeat_timer = UI_NAV_REPEAT_DELAY;
            return Some(nav);
        }
        self.render_ui.ui_nav_repeat_timer -= self.time.delta.max(0.0);
        if self.render_ui.ui_nav_repeat_timer > 0.0 {
            return None;
        }
        while self.render_ui.ui_nav_repeat_timer <= 0.0 {
            self.render_ui.ui_nav_repeat_timer += UI_NAV_REPEAT_RATE;
        }
        Some(nav)
    }

    pub(super) fn ui_stick_nav_direction(&self) -> Option<UiDirectionalNav> {
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if let Some(dir) = stick_nav_direction(gamepad.left_stick(), UI_NAV_STICK_ON) {
                return Some(UiDirectionalNav {
                    source: UiInputSource::Gamepad(index),
                    dir,
                });
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if let Some(dir) = stick_nav_direction(joycon.stick(), UI_NAV_STICK_ON) {
                return Some(UiDirectionalNav {
                    source: UiInputSource::JoyCon(index),
                    dir,
                });
            }
        }
        None
    }

    pub(super) fn ui_any_nav_stick_held(&self) -> bool {
        self.input
            .gamepads()
            .iter()
            .any(|gamepad| stick_nav_direction(gamepad.left_stick(), UI_NAV_STICK_OFF).is_some())
            || self
                .input
                .joycons()
                .iter()
                .any(|joycon| stick_nav_direction(joycon.stick(), UI_NAV_STICK_OFF).is_some())
    }

    pub(super) fn ui_action_pressed(&self) -> Option<UiInputSource> {
        if self.input.is_key_pressed(KeyCode::Enter) || self.input.is_key_pressed(KeyCode::Space) {
            return Some(UiInputSource::Kbm);
        }
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if gamepad.is_button_pressed(GamepadButton::Bottom) {
                return Some(UiInputSource::Gamepad(index));
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if joycon.is_button_pressed(JoyConButton::Right) {
                return Some(UiInputSource::JoyCon(index));
            }
        }
        None
    }

    pub(super) fn ui_action_released(&self) -> Option<UiInputSource> {
        if self.input.is_key_released(KeyCode::Enter) || self.input.is_key_released(KeyCode::Space)
        {
            return Some(UiInputSource::Kbm);
        }
        for (index, gamepad) in self.input.gamepads().iter().enumerate() {
            if gamepad.is_button_released(GamepadButton::Bottom) {
                return Some(UiInputSource::Gamepad(index));
            }
        }
        for (index, joycon) in self.input.joycons().iter().enumerate() {
            if joycon.is_button_released(JoyConButton::Right) {
                return Some(UiInputSource::JoyCon(index));
            }
        }
        None
    }

    pub(super) fn process_focused_button_action(&mut self) {
        let focused_button = self.render_ui.focused_ui_node.and_then(|node| {
            self.nodes
                .get(node)
                .and_then(|scene_node| match &scene_node.data {
                    SceneNodeData::UiButton(button) if !button_inactive(button) => Some(node),
                    SceneNodeData::UiCheckbox(checkbox) if !checkbox_inactive(checkbox) => {
                        Some(node)
                    }
                    SceneNodeData::UiImageButton(button) if !image_button_inactive(button) => {
                        Some(node)
                    }
                    SceneNodeData::UiNineSliceButton(button)
                        if !nine_slice_button_inactive(button) =>
                    {
                        Some(node)
                    }
                    _ => None,
                })
        });
        if let Some(source) = self.ui_action_pressed()
            && let Some(node) = focused_button
            && self.ui_node_accepts_input_source(node, source)
        {
            self.render_ui.nav_pressed_button = Some(node);
        }
        if let Some(source) = self.ui_action_released()
            && let Some(node) = self.render_ui.nav_pressed_button
            && self.ui_node_accepts_input_source(node, source)
        {
            self.render_ui.nav_pressed_button = None;
        }
    }

    pub(super) fn ui_node_accepts_input_source(&self, node: NodeID, source: UiInputSource) -> bool {
        let Some(scene_node) = self.nodes.get(node) else {
            return false;
        };
        match &scene_node.data {
            SceneNodeData::UiButton(button) => {
                !button_inactive(button) && self.ui_input_mask_accepts(&button.input_mask, source)
            }
            SceneNodeData::UiCheckbox(checkbox) => {
                !checkbox_inactive(checkbox)
                    && self.ui_input_mask_accepts(&checkbox.input_mask, source)
            }
            SceneNodeData::UiImageButton(button) => {
                !image_button_inactive(button)
                    && self.ui_input_mask_accepts(&button.input_mask, source)
            }
            SceneNodeData::UiNineSliceButton(button) => {
                !nine_slice_button_inactive(button)
                    && self.ui_input_mask_accepts(&button.input_mask, source)
            }
            data => text_edit_ref(data)
                .is_some_and(|edit| self.ui_input_mask_accepts(&edit.input_mask, source)),
        }
    }

    pub(super) fn ui_input_mask_accepts(
        &self,
        mask: &perro_ui::UiInputMask,
        source: UiInputSource,
    ) -> bool {
        if self.ui_input_mask_matches_kbm(mask.deny_kbm, source)
            || self.ui_input_mask_matches_ids(&mask.deny_gamepads, source, UiInputSource::Gamepad)
            || self.ui_input_mask_matches_ids(&mask.deny_joycons, source, UiInputSource::JoyCon)
            || mask
                .deny_players
                .iter()
                .any(|&player| self.ui_input_source_matches_player(player, source))
        {
            return false;
        }
        if !mask.has_allow_filter() {
            return true;
        }
        self.ui_input_mask_matches_kbm(mask.allow_kbm, source)
            || self.ui_input_mask_matches_ids(&mask.allow_gamepads, source, UiInputSource::Gamepad)
            || self.ui_input_mask_matches_ids(&mask.allow_joycons, source, UiInputSource::JoyCon)
            || mask
                .allow_players
                .iter()
                .any(|&player| self.ui_input_source_matches_player(player, source))
    }

    pub(super) fn ui_input_mask_matches_kbm(&self, enabled: bool, source: UiInputSource) -> bool {
        enabled && source == UiInputSource::Kbm
    }

    pub(super) fn ui_input_mask_matches_ids(
        &self,
        ids: &[usize],
        source: UiInputSource,
        make_source: fn(usize) -> UiInputSource,
    ) -> bool {
        ids.iter().any(|&id| make_source(id) == source)
    }

    pub(super) fn ui_input_source_matches_player(
        &self,
        player: usize,
        source: UiInputSource,
    ) -> bool {
        let Some(player) = self.input.players().get(player) else {
            return false;
        };
        match (player.get_binding(), source) {
            (PlayerBinding::Kbm, UiInputSource::Kbm) => true,
            (PlayerBinding::Gamepad { index }, UiInputSource::Gamepad(source_index)) => {
                index == source_index
            }
            (PlayerBinding::JoyConSingle { index }, UiInputSource::JoyCon(source_index)) => {
                index == source_index
            }
            (PlayerBinding::JoyConPair { left, right }, UiInputSource::JoyCon(source_index)) => {
                left == source_index || right == source_index
            }
            _ => false,
        }
    }
}
