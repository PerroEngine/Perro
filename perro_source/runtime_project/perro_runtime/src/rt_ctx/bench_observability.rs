//! Bench fixtures only; no script-facing API change.
use crate::{Runtime, RuntimeScriptApi};
use perro_ids::{NodeID, ScriptMemberID};
use perro_nodes::Node2D;
use perro_scripting::{ScriptBehavior, ScriptContext, ScriptFlags, ScriptLifecycle};
use perro_variant::Variant;
use std::any::Any;
use std::rc::Rc;

struct FrameScript {
    moving_stride: usize,
}

impl ScriptLifecycle<RuntimeScriptApi> for FrameScript {
    fn on_update(&self, ctx: &mut ScriptContext<'_, RuntimeScriptApi>) {
        if self.moving_stride != 0 && (ctx.id.index() as usize).is_multiple_of(self.moving_stride) {
            let _ = ctx
                .run
                .Nodes()
                .with_base_node_mut::<Node2D, _, _>(ctx.id, |node| {
                    node.transform.position.x = if node.transform.position.x == 0.0 {
                        1.0
                    } else {
                        0.0
                    };
                });
        }
    }
}

impl ScriptBehavior<RuntimeScriptApi> for FrameScript {
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(ScriptFlags::HAS_UPDATE | ScriptFlags::HAS_FIXED_UPDATE)
    }
    fn create_state(&self) -> Box<dyn Any> {
        Box::new([0u8; 32])
    }
    fn get_var(&self, _: &dyn Any, _: ScriptMemberID) -> Variant {
        Variant::Null
    }
    fn set_var(&self, _: &mut dyn Any, _: ScriptMemberID, _: Variant) {}
    fn call_method(
        &self,
        _: ScriptMemberID,
        _: &mut ScriptContext<'_, RuntimeScriptApi>,
        params: &[Variant],
    ) -> Variant {
        std::hint::black_box(params);
        Variant::Null
    }
}

/// Share one behavior across nodes; retain one 32-byte state allocation per node.
pub fn attach_shared_frame_scripts(runtime: &mut Runtime, ids: &[NodeID], moving_stride: usize) {
    let behavior: Rc<dyn ScriptBehavior<RuntimeScriptApi>> = Rc::new(FrameScript { moving_stride });
    for &id in ids {
        runtime
            .scripts
            .insert(id, behavior.clone(), behavior.create_state());
    }
}
