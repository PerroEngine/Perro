pub mod api;
pub mod sub_apis;

pub use api::RuntimeContext;

#[macro_export]
macro_rules! with_state {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state::<$state_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_state_mut {
    ($ctx:expr, $state_ty:ty, $id:expr, $f:expr) => {
        $ctx.Scripts().with_state_mut::<$state_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_node_mut {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_mut::<$node_ty, _, _>($id, $f)
    };
}

#[macro_export]
macro_rules! with_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node::<$node_ty, _>($id, $f)
    };
}

#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
}

#[macro_export]
macro_rules! get_node_name {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_name($id)
    };
}

#[macro_export]
macro_rules! set_node_name {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().set_node_name($id, $name)
    };
}

#[macro_export]
macro_rules! get_node_parent_id {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_parent_id($id)
    };
}

#[macro_export]
macro_rules! get_node_children_ids {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_children_ids($id)
    };
}

#[macro_export]
macro_rules! reparent {
    ($ctx:expr, $parent:expr, $child:expr) => {
        $ctx.Nodes().reparent($parent, $child)
    };
}

#[macro_export]
macro_rules! reparent_multi {
    ($ctx:expr, $parent:expr, $child_ids:expr) => {
        $ctx.Nodes().reparent_multi($parent, $child_ids)
    };
}

#[macro_export]
macro_rules! attach_script {
    ($ctx:expr, $id:expr, $path:expr) => {
        $ctx.Scripts().attach($id, $path)
    };
}

#[macro_export]
macro_rules! detach_script {
    ($ctx:expr, $id:expr) => {
        $ctx.Scripts().detach($id)
    };
}

#[macro_export]
macro_rules! get_var {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().get_var($id, $member)
    };
}

#[macro_export]
macro_rules! set_var {
    ($ctx:expr, $id:expr, $member:expr, $value:expr) => {
        $ctx.Scripts().set_var($id, $member, $value)
    };
}

#[macro_export]
macro_rules! call_method {
    ($ctx:expr, $id:expr, $method:expr, $params:expr) => {
        $ctx.Scripts().call_method($id, $method, $params)
    };
}

#[macro_export]
macro_rules! attributes_of {
    ($ctx:expr, $id:expr, $member:expr) => {
        $ctx.Scripts().attributes_of($id, $member)
    };
}

#[macro_export]
macro_rules! members_with {
    ($ctx:expr, $id:expr, $attribute:expr) => {
        $ctx.Scripts().members_with($id, $attribute)
    };
}

#[macro_export]
macro_rules! has_attribute {
    ($ctx:expr, $id:expr, $member:expr, $attribute:expr) => {
        $ctx.Scripts().has_attribute($id, $member, $attribute)
    };
}

#[macro_export]
macro_rules! connect_signal {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .connect($script, $signal, $function)
    };
}

#[macro_export]
macro_rules! disconnect_signal {
    ($ctx:expr, $script:expr, $signal:expr, $function:expr) => {
        $ctx.Signals()
            .disconnect($script, $signal, $function)
    };
}

#[macro_export]
macro_rules! emit_signal {
    ($ctx:expr, $signal:expr, $params:expr) => {
        $ctx.Signals().emit($signal, $params)
    };
    ($ctx:expr, $signal:expr) => {
        $ctx.Signals().emit($signal, &[])
    };
}

#[macro_export]
macro_rules! smid {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! sid {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! var {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! func {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! method {
    ($name:expr) => {
        ::perro_ids::ScriptMemberID::from_string($name)
    };
}

#[macro_export]
macro_rules! signal {
    ($name:expr) => {
        ::perro_ids::SignalID::from_string($name)
    };
}

#[macro_export]
macro_rules! member {
    ($name:expr) => {
        ::perro_runtime_context::sub_apis::Member::new($name)
    };
}

#[macro_export]
macro_rules! attribute {
    ($value:expr) => {
        ::perro_runtime_context::sub_apis::Attribute::new($value)
    };
}

#[macro_export]
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$(::perro_variant::Variant::from($value)),*]
    };
}

#[macro_export]
macro_rules! variant {
    ($value:expr) => {
        ::perro_variant::Variant::from($value)
    };
}

#[macro_export]
macro_rules! delta_time {
    ($ctx:expr) => {
        $ctx.Time().get_delta()
    };
}

#[macro_export]
macro_rules! fixed_delta_time {
    ($ctx:expr) => {
        $ctx.Time().get_fixed_delta()
    };
}

#[macro_export]
macro_rules! elapsed_time {
    ($ctx:expr) => {
        $ctx.Time().get_elapsed()
    };
}

pub mod prelude {
    pub use crate::api::{RuntimeAPI, RuntimeContext};
    pub use crate::sub_apis::{
        Attribute, IntoScriptMemberID, Member, NodeAPI, NodeModule, ScriptAPI, ScriptModule,
        SignalAPI, SignalModule, TimeAPI, TimeModule,
    };
    pub use crate::{
        attach_script, attributes_of, call_method, connect_signal, create_node, delta_time,
        detach_script, disconnect_signal, elapsed_time, emit_signal, fixed_delta_time, func,
        get_node_children_ids, get_node_name, get_node_parent_id, get_var, has_attribute, method,
        member, members_with, params, reparent, reparent_multi, set_node_name, set_var, sid, signal,
        smid, var, variant, with_node, with_node_mut, with_state, with_state_mut, attribute,
    };
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    use perro_core::prelude::Node2D;
    use perro_ids::NodeID;
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
            T: Default + Into<perro_core::SceneNodeData>,
        {
            NodeID::nil()
        }

        fn with_node_mut<T, V, F>(&mut self, _id: NodeID, _f: F) -> Option<V>
        where
            T: perro_core::NodeTypeDispatch,
            F: FnOnce(&mut T) -> V,
        {
            None
        }

        fn with_node<T, V: Clone + Default>(
            &mut self,
            _node: NodeID,
            _f: impl FnOnce(&T) -> V,
        ) -> V
        where
            T: perro_core::NodeTypeDispatch,
        {
            V::default()
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

        fn reparent(&mut self, _parent: NodeID, _child: NodeID) -> bool {
            false
        }

        fn reparent_multi<I>(&mut self, _parent: NodeID, _child_ids: I) -> usize
        where
            I: IntoIterator<Item = NodeID>,
        {
            0
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

        fn attach_script(&mut self, _node: NodeID, _script_path: &str) -> bool {
            false
        }

        fn detach_script(&mut self, _node: NodeID) -> bool {
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

        fn attributes_of(
            &mut self,
            _script: NodeID,
            _member: &str,
        ) -> &'static [Attribute] {
            &[]
        }

        fn members_with(
            &mut self,
            _script: NodeID,
            _attribute: &str,
        ) -> &'static [Member] {
            &[]
        }

        fn has_attribute(
            &mut self,
            _script: NodeID,
            _member: &str,
            _attribute: &str,
        ) -> bool {
            false
        }
    }

    impl SignalAPI for DummyRuntime {
        fn connect_signal(
            &mut self,
            _script: NodeID,
            _signal: perro_ids::SignalID,
            _function: perro_ids::ScriptMemberID,
        ) -> bool {
            true
        }

        fn disconnect_signal(
            &mut self,
            _script: NodeID,
            _signal: perro_ids::SignalID,
            _function: perro_ids::ScriptMemberID,
        ) -> bool {
            true
        }

        fn emit_signal(
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
        assert_eq!(get_node_name!(&mut ctx, id), None);
        assert!(!set_node_name!(&mut ctx, id, "player"));
        assert_eq!(get_node_parent_id!(&mut ctx, id), None);
        assert_eq!(get_node_children_ids!(&mut ctx, id), None);
        assert!(!reparent!(&mut ctx, NodeID::new(1), id));
        assert_eq!(reparent_multi!(&mut ctx, NodeID::new(1), [id]), 0);
        assert!(!attach_script!(&mut ctx, id, "res://scripts/a.rs"));
        assert!(!detach_script!(&mut ctx, id));
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
        assert_eq!(
            signal_member,
            perro_ids::SignalID::from_string("on_test")
        );
        let _value = get_var!(&mut ctx, id, member);
        set_var!(&mut ctx, id, member, variant!(perro_variant::Variant::Null));
        set_var!(&mut ctx, id, member, variant!(77_i32));
        let _result = call_method!(&mut ctx, id, method_member, &[]);
        let _result2 = call_method!(&mut ctx, id, member, params![1_i32, "abc"]);
        let _attrs = attributes_of!(&mut ctx, id, "speed");
        let _members = members_with!(&mut ctx, id, "export");
        let _has = has_attribute!(&mut ctx, id, "speed", "export");
        assert!(connect_signal!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert!(disconnect_signal!(
            &mut ctx,
            id,
            signal!("on_test"),
            method!("handle")
        ));
        assert_eq!(emit_signal!(&mut ctx, signal!("on_test"), params![1_i32]), 1);
        assert_eq!(emit_signal!(&mut ctx, signal!("on_test")), 1);

        let dt = delta_time!(&mut ctx);
        let fdt = fixed_delta_time!(&mut ctx);
        let elapsed = elapsed_time!(&mut ctx);
        assert_eq!(dt, 0.016);
        assert_eq!(fdt, 0.016);
        assert_eq!(elapsed, 1.0);
    }
}


