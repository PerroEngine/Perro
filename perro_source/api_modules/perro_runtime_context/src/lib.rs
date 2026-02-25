pub mod api;
pub mod sub_apis;

pub use api::RuntimeContext;

pub mod prelude {
    pub use crate::api::{RuntimeAPI, RuntimeContext};
    pub use crate::sub_apis::{
        Attribute, IntoNodeTags, IntoScriptMemberID, Member, NodeAPI, NodeModule, ScriptAPI,
        ScriptModule, SignalAPI, SignalModule, QueryExpr, QueryScope, TagQuery, TimeAPI,
        TimeModule,
    };
    pub use crate::{
        attribute, attributes_of, call_method, create_node, delta_time, elapsed_time,
        fixed_delta_time, get_node_children_ids, get_node_name, get_node_parent_id, get_node_tags,
        get_node_type, get_var, has_attribute, member, members_with, query, remove_node, reparent,
        reparent_multi, script_attach, script_detach, set_node_name, set_node_tags, set_var,
        signal_connect, signal_disconnect, signal_emit, tag_add, tag_remove, with_base_node,
        with_base_node_mut, with_node, with_node_mut, with_state, with_state_mut,
    };
    pub use perro_ids::{func, method, sid, signal, smid, tag, tags, var};
    pub use perro_variant::{params, variant};
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use perro_ids::{IntoTagID, NodeID, TagID};
    use perro_nodes::prelude::Node2D;
    use std::any::Any;

    struct DummyRuntime {
        state: Box<dyn Any>,
    }

    impl TimeAPI for DummyRuntime {
        fn get_delta(&self) -> f32 {
            0.016
        }
        fn get_fixed_delta(&self) -> f32 {
            0.016
        }
        fn get_elapsed(&self) -> f32 {
            1.0
        }
    }

    impl NodeAPI for DummyRuntime {
        fn create<T>(&mut self) -> NodeID
        where
            T: Default + Into<perro_nodes::SceneNodeData>,
        {
            NodeID::nil()
        }

        fn with_node_mut<T, V, F>(&mut self, _id: NodeID, _f: F) -> Option<V>
        where
            T: perro_nodes::NodeTypeDispatch,
            F: FnOnce(&mut T) -> V,
        {
            None
        }

        fn with_node<T, V: Clone + Default>(&mut self, _node: NodeID, _f: impl FnOnce(&T) -> V) -> V
        where
            T: perro_nodes::NodeTypeDispatch,
        {
            V::default()
        }

        fn with_base_node<T, V, F>(&mut self, _id: NodeID, _f: F) -> Option<V>
        where
            T: perro_nodes::NodeBaseDispatch,
            F: FnOnce(&T) -> V,
        {
            None
        }

        fn with_base_node_mut<T, V, F>(&mut self, _id: NodeID, _f: F) -> Option<V>
        where
            T: perro_nodes::NodeBaseDispatch,
            F: FnOnce(&mut T) -> V,
        {
            None
        }

        fn get_node_name(&mut self, _node: NodeID) -> Option<std::borrow::Cow<'static, str>> {
            None
        }

        fn set_node_name<S>(&mut self, _node: NodeID, _name: S) -> bool
        where
            S: Into<std::borrow::Cow<'static, str>>,
        {
            false
        }

        fn get_node_parent_id(&mut self, _node: NodeID) -> Option<NodeID> {
            None
        }

        fn get_node_children_ids(&mut self, _node: NodeID) -> Option<Vec<NodeID>> {
            None
        }

        fn get_node_type(&mut self, _node: NodeID) -> Option<perro_nodes::NodeType> {
            None
        }

        fn reparent(&mut self, _parent: NodeID, _child: NodeID) -> bool {
            false
        }

        fn reparent_multi<I>(&mut self, _parent: NodeID, _child_ids: I) -> usize
        where
            I: IntoIterator<Item = NodeID>,
        {
            0
        }

        fn remove_node(&mut self, _node_id: NodeID) -> bool {
            false
        }

        fn get_node_tags(&mut self, _node_id: NodeID) -> Option<Vec<TagID>> {
            None
        }

        fn set_node_tags<T>(&mut self, _node_id: NodeID, _tags: Option<T>) -> bool
        where
            T: Into<std::borrow::Cow<'static, [TagID]>>,
        {
            false
        }

        fn add_node_tag<T>(&mut self, _node_id: NodeID, _tag: T) -> bool
        where
            T: IntoTagID,
        {
            false
        }

        fn remove_node_tag<T>(&mut self, _node_id: NodeID, _tag: T) -> bool
        where
            T: IntoTagID,
        {
            false
        }

        fn query_nodes(&mut self, _query: TagQuery) -> Vec<NodeID> {
            Vec::new()
        }
    }

    impl ScriptAPI for DummyRuntime {
        fn with_state<T: 'static, V, F>(&mut self, _script: NodeID, f: F) -> Option<V>
        where
            F: FnOnce(&T) -> V,
        {
            self.state.downcast_ref::<T>().map(f)
        }

        fn with_state_mut<T: 'static, V, F>(&mut self, _script: NodeID, f: F) -> Option<V>
        where
            F: FnOnce(&mut T) -> V,
        {
            self.state.downcast_mut::<T>().map(f)
        }

        fn script_attach(&mut self, _node: NodeID, _script_path: &str) -> bool {
            false
        }

        fn script_detach(&mut self, _node: NodeID) -> bool {
            false
        }

        fn remove_script(&mut self, _script: NodeID) -> bool {
            false
        }

        fn get_var(
            &mut self,
            _script: NodeID,
            _member: perro_ids::ScriptMemberID,
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
        }

        fn set_var(
            &mut self,
            _script: NodeID,
            _member: perro_ids::ScriptMemberID,
            _value: perro_variant::Variant,
        ) {
        }

        fn call_method(
            &mut self,
            _script: NodeID,
            _method: perro_ids::ScriptMemberID,
            _params: &[perro_variant::Variant],
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
        }

        fn attributes_of(&mut self, _script: NodeID, _member: &str) -> &'static [Attribute] {
            &[]
        }

        fn members_with(&mut self, _script: NodeID, _attribute: &str) -> &'static [Member] {
            &[]
        }

        fn has_attribute(&mut self, _script: NodeID, _member: &str, _attribute: &str) -> bool {
            false
        }
    }

    impl SignalAPI for DummyRuntime {
        fn signal_connect(
            &mut self,
            _script: NodeID,
            _signal: perro_ids::SignalID,
            _function: perro_ids::ScriptMemberID,
        ) -> bool {
            true
        }

        fn signal_disconnect(
            &mut self,
            _script: NodeID,
            _signal: perro_ids::SignalID,
            _function: perro_ids::ScriptMemberID,
        ) -> bool {
            true
        }

        fn signal_emit(
            &mut self,
            _signal: perro_ids::SignalID,
            _params: &[perro_variant::Variant],
        ) -> usize {
            1
        }
    }

    #[test]
    fn script_macros_typecheck_and_forward() {
        let mut rt = DummyRuntime {
            state: Box::new(5_i32),
        };
        let mut ctx = RuntimeContext::new(&mut rt);
        let id = NodeID::new(42);

        let initial = with_state!(&mut ctx, i32, id, |state| *state);
        assert_eq!(initial, Some(5));

        let _ = with_state_mut!(&mut ctx, i32, id, |state| {
            *state += 7;
        });
        let updated = with_state!(&mut ctx, i32, id, |state| *state);
        assert_eq!(updated, Some(12));

        let _new_node = create_node!(&mut ctx, Node2D);
        with_node_mut!(&mut ctx, Node2D, id, |_node| {});
        let value = with_node!(&mut ctx, Node2D, id, |_node| 99_i32);
        assert_eq!(value, 0_i32);
        let _ = with_base_node!(&mut ctx, Node2D, id, |_node| 1_i32);
        let _ = with_base_node_mut!(&mut ctx, Node2D, id, |_node| 2_i32);
        assert_eq!(get_node_name!(&mut ctx, id), None);
        assert!(!set_node_name!(&mut ctx, id, "player"));
        assert_eq!(get_node_parent_id!(&mut ctx, id), None);
        assert_eq!(get_node_children_ids!(&mut ctx, id), None);
        assert_eq!(get_node_type!(&mut ctx, id), None);
        assert_eq!(get_node_tags!(&mut ctx, id), None);
        assert!(!set_node_tags!(&mut ctx, id, tags!["player", "enemy"]));
        assert!(!set_node_tags!(&mut ctx, id));
        assert!(!tag_add!(&mut ctx, id, "player"));
        assert!(!tag_remove!(&mut ctx, id, "player"));
        assert!(query!(&mut ctx, all(tags["player"], not(tags["enemy"]))).is_empty());
        assert!(query!(&mut ctx, all(is[Node2D], base[Node3D])).is_empty());
        assert!(!reparent!(&mut ctx, NodeID::new(1), id));
        assert_eq!(reparent_multi!(&mut ctx, NodeID::new(1), [id]), 0);
        assert!(!remove_node!(&mut ctx, id));
        assert!(!script_attach!(&mut ctx, id, "res://scripts/a.rs"));
        assert!(!script_detach!(&mut ctx, id));
        let member = var!("x");
        let member_alias = sid!("x");
        let var_member = var!("x");
        let method_member = method!("x");
        let func_member = func!("x");
        let signal_member = signal!("on_test");
        assert_eq!(member, member_alias);
        assert_eq!(member, var_member);
        assert_eq!(member, method_member);
        assert_eq!(member, func_member);
        assert_eq!(signal_member, perro_ids::SignalID::from_string("on_test"));
        let _value = get_var!(&mut ctx, id, member);
        set_var!(&mut ctx, id, member, variant!(perro_variant::Variant::Null));
        set_var!(&mut ctx, id, member, variant!(77_i32));
        let _result = call_method!(&mut ctx, id, method_member, &[]);
        let _result2 = call_method!(&mut ctx, id, member, params![1_i32, "abc"]);
        let _attrs = attributes_of!(&mut ctx, id, "speed");
        let _members = members_with!(&mut ctx, id, "export");
        let _has = has_attribute!(&mut ctx, id, "speed", "export");
        assert!(signal_connect!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert!(signal_disconnect!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert_eq!(
            signal_emit!(&mut ctx, signal!("on_test"), params![1_i32]),
            1
        );
        assert_eq!(signal_emit!(&mut ctx, signal!("on_test")), 1);

        let dt = delta_time!(&mut ctx);
        let fdt = fixed_delta_time!(&mut ctx);
        let elapsed = elapsed_time!(&mut ctx);
        assert_eq!(dt, 0.016);
        assert_eq!(fdt, 0.016);
        assert_eq!(elapsed, 1.0);
    }
}
