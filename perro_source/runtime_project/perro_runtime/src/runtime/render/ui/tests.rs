use super::*;
use crate::RuntimeScriptApi;
use perro_ids::ScriptMemberID;
use perro_nodes::{
    AmbientLight2D, AmbientLight3D, CameraStream3D, MeshInstance3D, Node3D, SceneNode,
    SceneNodeData, Sky3D, SubView2D, SubView3D, UiCameraStream, UiSubView, Webcam,
    camera_3d::Camera3D, particle_emitter_2d::ParticleEmitter2D,
};
use perro_render_bridge::{
    CameraStreamCommand, CameraStreamSourceState, Command2D, Command3D, Light2DState, RenderEvent,
    UiCommand,
};
use perro_resource_api::sub_apis::{TextureAPI, WebcamAPI, WebcamFrame};
use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec, SignalAPI};
use perro_scripting::{ScriptBehavior, ScriptContext, ScriptFlags, ScriptLifecycle};
use perro_structs::{Color, Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_ui::{
    UiAnchor, UiAnimatedImage, UiAnimatedImageFrameSet, UiGrid, UiHLayout, UiLayoutSpacingMode,
    UiPanel, UiScrollContainer, UiShape, UiShapeKind, UiVLayout, UiVector2,
};
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    Color::new(r, g, b, a).to_float_slice()
}

fn collect_resource_texture_request(
    runtime: &mut Runtime,
    texture: TextureID,
) -> perro_render_bridge::RenderRequestID {
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    commands
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, id, .. })
                if id == texture =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected texture create request")
}

fn has_external_texture_create(commands: &[RenderCommand]) -> bool {
    commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::Resource(ResourceCommand::CreateExternalTexture { .. })
        )
    })
}

fn insert_panel(runtime: &mut Runtime, size: [f32; 2], fill: Color) -> NodeID {
    let mut panel = UiPanel::new();
    panel.layout.size = UiVector2::pixels(size[0], size[1]);
    panel.style.fill = fill;
    insert_ui_node(runtime, SceneNodeData::UiPanel(Box::new(panel)))
}

fn insert_button(runtime: &mut Runtime, size: [f32; 2]) -> NodeID {
    let mut button = perro_ui::UiButton::new();
    button.layout.size = UiVector2::pixels(size[0], size[1]);
    button.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
    button.hover_style.fill = Color::new(0.2, 0.3, 0.4, 1.0);
    button.pressed_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
    insert_ui_node(runtime, SceneNodeData::UiButton(Box::new(button)))
}

fn insert_button_at(runtime: &mut Runtime, size: [f32; 2], x: f32, y: f32) -> NodeID {
    let mut button = perro_ui::UiButton::new();
    button.layout.size = UiVector2::pixels(size[0], size[1]);
    button.transform.position = UiVector2::pixels(x, y);
    button.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
    button.hover_style.fill = Color::new(0.2, 0.3, 0.4, 1.0);
    button.pressed_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
    insert_ui_node(runtime, SceneNodeData::UiButton(Box::new(button)))
}

fn insert_text_box_at(runtime: &mut Runtime, x: f32, y: f32) -> NodeID {
    let mut text_box = perro_ui::UiTextBox::new();
    text_box.inner.base.layout.size = UiVector2::pixels(140.0, 40.0);
    text_box.inner.base.transform.position = UiVector2::pixels(x, y);
    insert_ui_node(runtime, SceneNodeData::UiTextBox(Box::new(text_box)))
}

fn insert_text_block_at(runtime: &mut Runtime, x: f32, y: f32) -> NodeID {
    let mut text_block = perro_ui::UiTextBlock::new();
    text_block.inner.base.layout.size = UiVector2::pixels(140.0, 80.0);
    text_block.inner.base.transform.position = UiVector2::pixels(x, y);
    insert_ui_node(runtime, SceneNodeData::UiTextBlock(Box::new(text_block)))
}

fn tap_key_and_extract(runtime: &mut Runtime, key: KeyCode) {
    runtime.begin_input_frame();
    runtime.set_key_state(key, true);
    runtime.extract_render_ui_commands();
    runtime.set_key_state(key, false);
}

fn click_mouse_and_extract(runtime: &mut Runtime, x: f32, y: f32) {
    runtime.begin_input_frame();
    runtime.set_mouse_position(x, y);
    runtime.set_mouse_button_state(MouseButton::Left, true);
    runtime.extract_render_ui_commands();
    runtime.set_mouse_button_state(MouseButton::Left, false);
}

fn set_panel_visible(runtime: &mut Runtime, node: NodeID, visible: bool) {
    if let Some(mut scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.visible = visible;
    }
}

fn insert_ui_node(runtime: &mut Runtime, data: SceneNodeData) -> NodeID {
    let node = runtime.nodes.insert(SceneNode::new(data));
    runtime.mark_needs_rerender(node);
    node
}

fn attach_child(runtime: &mut Runtime, parent: NodeID, child: NodeID) {
    runtime
        .nodes
        .get_mut(parent)
        .expect("parent exists")
        .add_child(child);
    assert!(runtime.nodes.set_parent(child, parent), "child exists");
    runtime.mark_needs_rerender(parent);
    runtime.mark_needs_rerender(child);
}

struct HideClickedButtonScript {
    calls: Arc<AtomicUsize>,
}

impl ScriptLifecycle<RuntimeScriptApi> for HideClickedButtonScript {}

include!("tests/streams.rs");
include!("tests/controls.rs");
include!("tests/layout.rs");
include!("tests/styling.rs");

impl ScriptBehavior<RuntimeScriptApi> for HideClickedButtonScript {
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(ScriptFlags::NONE)
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }

    fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
        Variant::Null
    }

    fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: Variant) {}

    fn call_method(
        &self,
        _method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
        params: &[Variant],
    ) -> Variant {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(button_id) = params.first().and_then(Variant::as_node) {
            let _ =
                ctx.run
                    .Nodes()
                    .with_node_mut::<perro_ui::UiButton, _, _>(button_id, |button| {
                        button.visible = false;
                    });
        }
        Variant::Null
    }
}
