//! UI element registry — single source of truth for Panel, Button, Text.
//!
//! Like the node registry: one macro defines the enum, types, and BaseElement impls.
//! To add a new element: add one line in `define_ui_elements!`, add the struct in ui_elements/,
//! then FUR + rendering.

use crate::ids::UIElementID;
use crate::nodes::ui::fur_ast::FurAnchor;
use crate::nodes::ui::ui_element::BaseElement;
use crate::structs2d::{Transform2D, Vector2};
use std::collections::HashMap;
use std::fmt;

/// Implements BaseElement + Deref for a UI type (has .base: BaseUIElement).
/// Called by define_ui_elements! for each type — no separate impl in each file.
#[macro_export]
macro_rules! impl_ui_element {
    ($ty:ty) => {
        impl crate::nodes::ui::ui_element::BaseElement for $ty {
            fn get_id(&self) -> crate::ids::UIElementID {
                self.base.id
            }
            fn set_id(&mut self, id: crate::ids::UIElementID) {
                self.base.id = id;
            }
            fn get_name(&self) -> &str {
                &self.base.name
            }
            fn set_name(&mut self, name: &str) {
                self.base.name = name.to_string();
            }
            fn get_visible(&self) -> bool {
                self.base.visible
            }
            fn set_visible(&mut self, visible: bool) {
                self.base.visible = visible;
            }
            fn get_parent(&self) -> crate::ids::UIElementID {
                self.base.parent_id
            }
            fn set_parent(&mut self, parent: Option<crate::ids::UIElementID>) {
                self.base.parent_id = parent.unwrap_or(crate::ids::UIElementID::nil());
            }
            fn get_children(&self) -> &[crate::ids::UIElementID] {
                &self.base.children
            }
            fn set_children(&mut self, children: Vec<crate::ids::UIElementID>) {
                self.base.children = children;
            }
            fn get_transform(&self) -> &crate::structs2d::Transform2D {
                &self.base.transform
            }
            fn get_transform_mut(&mut self) -> &mut crate::structs2d::Transform2D {
                &mut self.base.transform
            }
            fn get_global_transform(&self) -> &crate::structs2d::Transform2D {
                &self.base.global_transform
            }
            fn set_global_transform(&mut self, transform: crate::structs2d::Transform2D) {
                self.base.global_transform = transform;
            }
            fn get_size(&self) -> &crate::structs2d::Vector2 {
                &self.base.size
            }
            fn set_size(&mut self, size: crate::structs2d::Vector2) {
                self.base.size = size;
            }
            fn get_pivot(&self) -> &crate::structs2d::Vector2 {
                &self.base.pivot
            }
            fn set_pivot(&mut self, pivot: crate::structs2d::Vector2) {
                self.base.pivot = pivot;
            }
            fn get_anchor(&self) -> &crate::fur_ast::FurAnchor {
                &self.base.anchor
            }
            fn set_anchor(&mut self, anchor: crate::fur_ast::FurAnchor) {
                self.base.anchor = anchor;
            }
            fn get_modulate(&self) -> Option<&crate::structs::Color> {
                self.base.modulate.as_ref()
            }
            fn set_modulate(&mut self, color: Option<crate::structs::Color>) {
                self.base.modulate = color;
            }
            fn get_z_index(&self) -> i32 {
                self.base.z_index
            }
            fn set_z_index(&mut self, z_index: i32) {
                self.base.z_index = z_index;
            }
            fn get_style_map(&self) -> &std::collections::HashMap<String, f32> {
                &self.base.style_map
            }
            fn get_style_map_mut(&mut self) -> &mut std::collections::HashMap<String, f32> {
                &mut self.base.style_map
            }
        }
        impl std::ops::Deref for $ty {
            type Target = crate::nodes::ui::ui_element::BaseUIElement;
            fn deref(&self) -> &Self::Target {
                &self.base
            }
        }
        impl std::ops::DerefMut for $ty {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.base
            }
        }
    };
}

/// Single macro: defines UIElementType, UIElement enum, BaseElement impls, dispatch, element_type.
/// Like define_nodes! — one place to add new element types.
macro_rules! define_ui_elements {
    ( $( $variant:ident => $ty:path ),+ $(,)? ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum UIElementType {
            $( $variant, )+
        }

        impl UIElementType {
            pub fn type_name(&self) -> &'static str {
                match self {
                    $( UIElementType::$variant => concat!("UI", stringify!($variant)), )+
                }
            }
        }

        impl fmt::Display for UIElementType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.type_name())
            }
        }

        impl std::str::FromStr for UIElementType {
            type Err = String;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $( _ if s.eq_ignore_ascii_case(stringify!($variant)) || s.eq_ignore_ascii_case(concat!("UI", stringify!($variant))) => Ok(UIElementType::$variant), )+
                    _ => Err(format!("Unknown UI element type: {}", s)),
                }
            }
        }

        #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
        #[serde(tag = "type")]
        pub enum UIElement {
            $( $variant($ty), )+
        }

        impl BaseElement for UIElement {
            fn get_id(&self) -> UIElementID {
                match self { $( UIElement::$variant(e) => e.get_id(), )+ }
            }
            fn set_id(&mut self, id: UIElementID) {
                match self { $( UIElement::$variant(e) => e.set_id(id), )+ }
            }
            fn get_name(&self) -> &str {
                match self { $( UIElement::$variant(e) => e.get_name(), )+ }
            }
            fn set_name(&mut self, name: &str) {
                match self { $( UIElement::$variant(e) => e.set_name(name), )+ }
            }
            fn get_visible(&self) -> bool {
                match self { $( UIElement::$variant(e) => e.get_visible(), )+ }
            }
            fn set_visible(&mut self, visible: bool) {
                match self { $( UIElement::$variant(e) => e.set_visible(visible), )+ }
            }
            fn get_parent(&self) -> UIElementID {
                match self { $( UIElement::$variant(e) => e.get_parent(), )+ }
            }
            fn set_parent(&mut self, parent: Option<UIElementID>) {
                match self { $( UIElement::$variant(e) => e.set_parent(parent), )+ }
            }
            fn get_children(&self) -> &[UIElementID] {
                match self { $( UIElement::$variant(e) => e.get_children(), )+ }
            }
            fn set_children(&mut self, children: Vec<UIElementID>) {
                match self { $( UIElement::$variant(e) => e.set_children(children), )+ }
            }
            fn get_transform(&self) -> &Transform2D {
                match self { $( UIElement::$variant(e) => e.get_transform(), )+ }
            }
            fn get_transform_mut(&mut self) -> &mut Transform2D {
                match self { $( UIElement::$variant(e) => e.get_transform_mut(), )+ }
            }
            fn get_global_transform(&self) -> &Transform2D {
                match self { $( UIElement::$variant(e) => e.get_global_transform(), )+ }
            }
            fn set_global_transform(&mut self, transform: Transform2D) {
                match self { $( UIElement::$variant(e) => e.set_global_transform(transform), )+ }
            }
            fn get_size(&self) -> &Vector2 {
                match self { $( UIElement::$variant(e) => e.get_size(), )+ }
            }
            fn set_size(&mut self, size: Vector2) {
                match self { $( UIElement::$variant(e) => e.set_size(size), )+ }
            }
            fn get_pivot(&self) -> &Vector2 {
                match self { $( UIElement::$variant(e) => e.get_pivot(), )+ }
            }
            fn set_pivot(&mut self, pivot: Vector2) {
                match self { $( UIElement::$variant(e) => e.set_pivot(pivot), )+ }
            }
            fn get_anchor(&self) -> &FurAnchor {
                match self { $( UIElement::$variant(e) => e.get_anchor(), )+ }
            }
            fn set_anchor(&mut self, anchor: FurAnchor) {
                match self { $( UIElement::$variant(e) => e.set_anchor(anchor), )+ }
            }
            fn get_modulate(&self) -> Option<&crate::structs::Color> {
                match self { $( UIElement::$variant(e) => e.get_modulate(), )+ }
            }
            fn set_modulate(&mut self, color: Option<crate::structs::Color>) {
                match self { $( UIElement::$variant(e) => e.set_modulate(color), )+ }
            }
            fn get_z_index(&self) -> i32 {
                match self { $( UIElement::$variant(e) => e.get_z_index(), )+ }
            }
            fn set_z_index(&mut self, z_index: i32) {
                match self { $( UIElement::$variant(e) => e.set_z_index(z_index), )+ }
            }
            fn get_style_map(&self) -> &HashMap<String, f32> {
                match self { $( UIElement::$variant(e) => e.get_style_map(), )+ }
            }
            fn get_style_map_mut(&mut self) -> &mut HashMap<String, f32> {
                match self { $( UIElement::$variant(e) => e.get_style_map_mut(), )+ }
            }
        }

        impl UIElement {
            /// Typed read (mirrors SceneNode::with_typed_ref).
            #[inline(always)]
            pub fn with_typed_ref<T: UIElementDispatch, R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
                T::extract_ref(self).map(f)
            }
            /// Typed mutate (mirrors SceneNode::with_typed_mut).
            #[inline(always)]
            pub fn with_typed_mut<T: UIElementDispatch, R>(&mut self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
                T::extract_mut(self).map(f)
            }
            /// Read using base fields only (mirrors read_scene_node for base Node).
            #[inline]
            pub fn with_base_ref<R>(&self, f: impl FnOnce(&dyn BaseElement) -> R) -> R {
                f(self)
            }
        }

        /// Trait for extracting concrete UI element types from UIElement (like NodeTypeDispatch for nodes).
        pub trait UIElementDispatch: 'static {
            fn extract_ref(element: &UIElement) -> Option<&Self>;
            fn extract_mut(element: &mut UIElement) -> Option<&mut Self>;
        }

        $(
            impl crate::nodes::ui::ui_element::IntoUIInner<$ty> for UIElement {
                fn into_ui_inner(self) -> $ty {
                    match self {
                        UIElement::$variant(inner) => inner,
                        _ => panic!("Cannot extract {} from UIElement variant", stringify!($variant)),
                    }
                }
            }
            impl UIElementDispatch for $ty {
                fn extract_ref(element: &UIElement) -> Option<&Self> {
                    match element {
                        UIElement::$variant(inner) => Some(inner),
                        _ => None,
                    }
                }
                fn extract_mut(element: &mut UIElement) -> Option<&mut Self> {
                    match element {
                        UIElement::$variant(inner) => Some(inner),
                        _ => None,
                    }
                }
            }
            impl_ui_element!($ty);
        )+

        pub fn element_type(element: &UIElement) -> UIElementType {
            match element {
                $( UIElement::$variant(_) => UIElementType::$variant, )+
            }
        }
    };
}

define_ui_elements! {
    Panel => crate::nodes::ui::ui_elements::ui_container::UIPanel,
    Button => crate::nodes::ui::ui_elements::ui_button::UIButton,
    Text => crate::nodes::ui::ui_elements::ui_text::UIText,
}
