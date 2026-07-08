//! Locale text bindings for retained UI nodes.

use super::Runtime;
use crate::runtime::state::{LocaleTextBinding, LocaleTextField};
use perro_ids::{NodeID, string_to_u64};
use perro_nodes::SceneNodeData;
use std::borrow::Cow;

impl Runtime {
    pub(crate) fn add_locale_text_binding(
        &mut self,
        node: NodeID,
        field: LocaleTextField,
        key: String,
        key_hash: u64,
    ) {
        let binding = LocaleTextBinding {
            node,
            field,
            key,
            key_hash,
        };
        let _ = self.insert_locale_text_binding(binding);
    }

    pub(crate) fn bind_locale_text(&mut self, node: NodeID, key: &str) -> bool {
        let Some(field) = self
            .nodes
            .get(node)
            .and_then(|scene_node| default_locale_text_field(&scene_node.data))
        else {
            return false;
        };
        self.bind_locale_text_field(node, field, key)
    }

    pub(crate) fn bind_locale_placeholder(&mut self, node: NodeID, key: &str) -> bool {
        self.bind_locale_text_field(node, LocaleTextField::TextEditPlaceholder, key)
    }

    fn bind_locale_text_field(&mut self, node: NodeID, field: LocaleTextField, key: &str) -> bool {
        if !self
            .nodes
            .get(node)
            .is_some_and(|scene_node| locale_text_field_supported(&scene_node.data, field))
        {
            return false;
        }
        let key = key.trim();
        if key.is_empty() {
            return false;
        }
        let binding = LocaleTextBinding {
            node,
            field,
            key: key.to_string(),
            key_hash: string_to_u64(key),
        };
        self.insert_locale_text_binding(binding)
    }

    fn insert_locale_text_binding(&mut self, binding: LocaleTextBinding) -> bool {
        self.locale_text
            .bindings
            .retain(|existing| existing.node != binding.node || existing.field != binding.field);
        let changed = self.apply_locale_text_binding(&binding);
        let node = binding.node;
        self.locale_text.bindings.push(binding);
        self.locale_text.last_epoch = self.resource_api.localization_epoch();
        if changed {
            self.mark_ui_dirty(
                node,
                Runtime::UI_DIRTY_TEXT
                    | Runtime::UI_DIRTY_LAYOUT_SELF
                    | Runtime::UI_DIRTY_LAYOUT_PARENT
                    | Runtime::UI_DIRTY_COMMANDS,
            );
        }
        true
    }

    pub(super) fn refresh_locale_text_bindings(&mut self) {
        let epoch = self.resource_api.localization_epoch();
        if epoch == self.locale_text.last_epoch {
            return;
        }
        self.locale_text.last_epoch = epoch;
        let bindings = self.locale_text.bindings.clone();
        let mut live = Vec::with_capacity(bindings.len());
        for binding in bindings {
            if self.nodes.get(binding.node).is_none() {
                continue;
            }
            let changed = self.apply_locale_text_binding(&binding);
            if changed {
                self.mark_ui_dirty(
                    binding.node,
                    Runtime::UI_DIRTY_TEXT
                        | Runtime::UI_DIRTY_LAYOUT_SELF
                        | Runtime::UI_DIRTY_LAYOUT_PARENT
                        | Runtime::UI_DIRTY_COMMANDS,
                );
            }
            live.push(binding);
        }
        self.locale_text.bindings = live;
    }

    fn apply_locale_text_binding(&mut self, binding: &LocaleTextBinding) -> bool {
        let text = self
            .resource_api
            .localized_or_key_by_hash(&binding.key, binding.key_hash);
        let Some(scene_node) = self.nodes.get_mut(binding.node) else {
            return false;
        };
        match (&mut scene_node.data, binding.field) {
            (SceneNodeData::UiLabel(label), LocaleTextField::LabelText) => {
                if label.text.as_ref() == text {
                    return false;
                }
                label.text = Cow::Borrowed(text);
                true
            }
            (SceneNodeData::Label2D(label), LocaleTextField::LabelText) => {
                if label.text.as_ref() == text {
                    return false;
                }
                label.text = Cow::Borrowed(text);
                true
            }
            (SceneNodeData::Label3D(label), LocaleTextField::LabelText) => {
                if label.text.as_ref() == text {
                    return false;
                }
                label.text = Cow::Borrowed(text);
                true
            }
            (SceneNodeData::TextDecal3D(label), LocaleTextField::LabelText) => {
                if label.text.as_ref() == text {
                    return false;
                }
                label.text = Cow::Borrowed(text);
                true
            }
            (SceneNodeData::UiTextBox(text_box), LocaleTextField::TextEditText) => {
                if text_box.inner.text.as_ref() == text {
                    return false;
                }
                text_box.inner.text = Cow::Borrowed(text);
                text_box.inner.caret = text_box.inner.text.len();
                text_box.inner.anchor = text_box.inner.caret;
                true
            }
            (SceneNodeData::UiTextBlock(text_block), LocaleTextField::TextEditText) => {
                if text_block.inner.text.as_ref() == text {
                    return false;
                }
                text_block.inner.text = Cow::Borrowed(text);
                text_block.inner.caret = text_block.inner.text.len();
                text_block.inner.anchor = text_block.inner.caret;
                true
            }
            (SceneNodeData::UiTextBox(text_box), LocaleTextField::TextEditPlaceholder) => {
                if text_box.inner.placeholder.as_ref() == text {
                    return false;
                }
                text_box.inner.placeholder = Cow::Borrowed(text);
                true
            }
            (SceneNodeData::UiTextBlock(text_block), LocaleTextField::TextEditPlaceholder) => {
                if text_block.inner.placeholder.as_ref() == text {
                    return false;
                }
                text_block.inner.placeholder = Cow::Borrowed(text);
                true
            }
            _ => false,
        }
    }
}

fn default_locale_text_field(data: &SceneNodeData) -> Option<LocaleTextField> {
    match data {
        SceneNodeData::UiLabel(_)
        | SceneNodeData::Label2D(_)
        | SceneNodeData::Label3D(_)
        | SceneNodeData::TextDecal3D(_) => Some(LocaleTextField::LabelText),
        SceneNodeData::UiTextBox(_) | SceneNodeData::UiTextBlock(_) => {
            Some(LocaleTextField::TextEditText)
        }
        _ => None,
    }
}

fn locale_text_field_supported(data: &SceneNodeData, field: LocaleTextField) -> bool {
    matches!(
        (data, field),
        (SceneNodeData::UiLabel(_), LocaleTextField::LabelText)
            | (SceneNodeData::Label2D(_), LocaleTextField::LabelText)
            | (SceneNodeData::Label3D(_), LocaleTextField::LabelText)
            | (SceneNodeData::TextDecal3D(_), LocaleTextField::LabelText)
            | (SceneNodeData::UiTextBox(_), LocaleTextField::TextEditText)
            | (SceneNodeData::UiTextBlock(_), LocaleTextField::TextEditText)
            | (
                SceneNodeData::UiTextBox(_),
                LocaleTextField::TextEditPlaceholder
            )
            | (
                SceneNodeData::UiTextBlock(_),
                LocaleTextField::TextEditPlaceholder
            )
    )
}
