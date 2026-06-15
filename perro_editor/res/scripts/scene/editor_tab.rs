use perro_api::prelude::*;

#[derive(Variant, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EditorTab {
    #[default]
    Scene,
    Script,
    AnimTree,
}

impl EditorTab {
    pub fn signal_name(self) -> &'static str {
        match self {
            Self::Scene => "editor_tab_scene",
            Self::Script => "editor_tab_script",
            Self::AnimTree => "editor_tab_anim",
        }
    }

    pub fn root_name(self) -> &'static str {
        match self {
            Self::Scene => "scene_editor_root",
            Self::Script => "script_editor_root",
            Self::AnimTree => "anim_editor_root",
        }
    }

    pub fn button_name(self) -> &'static str {
        match self {
            Self::Scene => "tab_scene_button",
            Self::Script => "tab_script_button",
            Self::AnimTree => "tab_anim_button",
        }
    }

    pub fn label_name(self) -> &'static str {
        match self {
            Self::Scene => "tab_scene_label",
            Self::Script => "tab_script_label",
            Self::AnimTree => "tab_anim_label",
        }
    }

    pub fn accent_fill(self) -> &'static str {
        match self {
            Self::Scene => "#2A2F36",
            Self::Script => "#2A2F36",
            Self::AnimTree => "#2A2F36",
        }
    }

    pub fn accent_text(self) -> &'static str {
        match self {
            Self::Scene => "#5EA868",
            Self::Script => "#5A91DD",
            Self::AnimTree => "#A7AFB9",
        }
    }

    pub fn all() -> [EditorTab; 3] {
        [Self::Scene, Self::Script, Self::AnimTree]
    }
}
