use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

type SelfNodeType = Node2D;

#[State]
pub struct EmptyState {}

lifecycle!({
    fn on_init(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {
    }

    fn on_all_init(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});

methods!({
    fn default_method(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});
