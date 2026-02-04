use std::{
    borrow::Cow,
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use crate::ids::{SignalID, UIElementID};
use serde::{Deserialize, Serialize};

use crate::{
    Node,
    nodes::node_registry::NodeType,
    prelude::string_to_u64,
    scripting::api::ScriptApi,
    ui_element::{BaseElement, IntoUIInner, UIElement},
};

fn default_visible() -> bool {
    true
}
fn is_default_visible(v: &bool) -> bool {
    *v == default_visible()
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct UINode {
    #[serde(rename = "type")]
    pub ty: NodeType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fur_path: Option<Cow<'static, str>>,

    #[serde(skip)]
    pub loaded_fur_path: Option<Cow<'static, str>>,

    #[serde(skip)]
    pub elements: Option<HashMap<UIElementID, UIElement>>,
    #[serde(skip)]
    pub root_ids: Option<Vec<UIElementID>>,

    #[serde(
        default = "default_visible",
        skip_serializing_if = "is_default_visible"
    )]
    pub visible: bool,

    pub base: Node,
}

impl UINode {
    pub fn new() -> Self {
        let mut base = Node::new();
        base.name = Cow::Borrowed("UINode");
        Self {
            ty: NodeType::UINode,
            visible: default_visible(),
            base,
            fur_path: None,
            loaded_fur_path: None,
            elements: None,
            root_ids: None,
        }
    }

    pub fn get_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Get an element by ID (for read_ui_element / mutate_ui_element)
    pub fn get_element_by_id(&self, id: UIElementID) -> Option<&UIElement> {
        self.elements.as_ref().and_then(|m| m.get(&id))
    }

    /// Get a mutable element by ID (for mutate_ui_element)
    pub fn get_element_by_id_mut(&mut self, id: UIElementID) -> Option<&mut UIElement> {
        self.elements.as_mut().and_then(|m| m.get_mut(&id))
    }

    /// Find an element by name (ID) in the UI tree
    pub fn find_element_by_name(&self, name: &str) -> Option<&UIElement> {
        if let Some(elements) = &self.elements {
            elements.values().find(|el| el.get_name() == name)
        } else {
            None
        }
    }

    /// Find a mutable element by name (ID) in the UI tree
    pub fn find_element_by_name_mut(&mut self, name: &str) -> Option<&mut UIElement> {
        if let Some(elements) = &mut self.elements {
            elements.values_mut().find(|el| el.get_name() == name)
        } else {
            None
        }
    }

    /// Get an element by name (ID) - returns a reference
    /// This is useful for checking if an element exists or reading its properties
    pub fn get_element(&self, name: &str) -> Option<&UIElement> {
        self.find_element_by_name(name)
    }

    /// Get an element by name (ID) and clone it as a specific type
    /// Similar to `get_node_clone` for SceneNode
    ///
    /// This clones the element. After modifying it, use `set_element` to put it back.
    ///
    /// # Example
    /// ```ignore
    /// let mut text: UIText = ui_node.get_element_clone("bob").unwrap();
    /// text.set_content("Hello");
    /// ui_node.set_element("bob", UIElement::Text(text));
    /// ```
    pub fn get_element_clone<T: Clone>(&self, name: &str) -> Option<T>
    where
        UIElement: IntoUIInner<T>,
    {
        if let Some(element) = self.find_element_by_name(name) {
            // Clone the element and convert it
            let cloned = element.clone();
            Some(cloned.into_ui_inner())
        } else {
            None
        }
    }

    pub fn set_element(&mut self, name: &str, element: UIElement) -> bool {
        if let Some(elements) = &mut self.elements {
            // Find the element by name and get its ID
            if let Some((id, _)) = elements.iter().find(|(_, el)| el.get_name() == name) {
                let id = *id;
                elements.insert(id, element);
                return true;
            }
        }
        false
    }


    /// Get a Text element by name (ID) - returns a reference to UIText if the element is a Text element
    /// Returns None if the element doesn't exist or isn't a Text element
    pub fn get_text_element(&self, name: &str) -> Option<&crate::ui_elements::ui_text::UIText> {
        if let Some(element) = self.find_element_by_name(name) {
            if let UIElement::Text(text) = element {
                return Some(text);
            }
        }
        None
    }

    /// Get a mutable Text element by name (ID) - returns a mutable reference to UIText if the element is a Text element
    /// Returns None if the element doesn't exist or isn't a Text element
    pub fn get_text_element_mut(
        &mut self,
        name: &str,
    ) -> Option<&mut crate::ui_elements::ui_text::UIText> {
        if let Some(element) = self.find_element_by_name_mut(name) {
            if let UIElement::Text(text) = element {
                return Some(text);
            }
        }
        None
    }

}

impl Deref for UINode {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for UINode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl UINode {
    pub fn internal_update(&mut self, api: &mut ScriptApi<'_>) {
        if let Some(gfx) = api.gfx.as_mut() {
            let events = gfx.egui_integration.drain_events();
            for event in events {
                let (element_id, suffix) = match event {
                    crate::nodes::ui::egui_integration::ElementEvent::ButtonHovered(id) => {
                        (id, "Hovered")
                    }
                    crate::nodes::ui::egui_integration::ElementEvent::ButtonUnhovered(id) => {
                        (id, "Unhovered")
                    }
                    crate::nodes::ui::egui_integration::ElementEvent::ButtonPressed(id) => {
                        (id, "Pressed")
                    }
                    crate::nodes::ui::egui_integration::ElementEvent::ButtonReleased(id) => {
                        (id, "Released")
                    }
                };

                let Some(element) = self.get_element_by_id(element_id) else {
                    continue;
                };
                let signal_name = format!("{}_{}", element.get_name(), suffix);
                let signal_id = SignalID::from_u64(string_to_u64(&signal_name));
                api.emit_signal_id(signal_id, &[]);
            }
        }

        api.scene.mark_needs_rerender(self.base.id);
    }
}

impl crate::nodes::node_registry::NodeWithInternalUpdate for UINode {
    fn internal_update(&mut self, api: &mut crate::scripting::api::ScriptApi) {
        UINode::internal_update(self, api);
    }
}
