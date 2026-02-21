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
macro_rules! mutate_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().mutate::<$node_ty, _>($id, $f)
    };
}

#[macro_export]
macro_rules! read_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().read::<$node_ty, _>($id, $f)
    };
}

#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
}

#[macro_export]
macro_rules! mutate_meta {
    ($ctx:expr, $id:expr, $f:expr) => {
        $ctx.Nodes().mutate_meta($id, $f)
    };
}

#[macro_export]
macro_rules! read_meta {
    ($ctx:expr, $id:expr, $f:expr) => {
        $ctx.Nodes().read_meta($id, $f)
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
macro_rules! params {
    ($($value:expr),* $(,)?) => {
        &[$(::perro_variant::Variant::from($value)),*]
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
    pub use crate::sub_apis::{NodeAPI, NodeModule, ScriptAPI, ScriptModule, TimeAPI, TimeModule};
    pub use crate::{
        attach_script, call_method, create_node, delta_time, detach_script, elapsed_time,
        fixed_delta_time, get_var, mutate_meta, mutate_node, read_meta, read_node, set_var,
        with_state, with_state_mut, params,
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

        fn mutate<T, F>(&mut self, _id: NodeID, _f: F)
        where
            T: perro_core::NodeTypeDispatch,
            F: FnOnce(&mut T),
        {
        }

        fn read<T, V: Clone + Default>(&mut self, _node_id: NodeID, _f: impl FnOnce(&T) -> V) -> V
        where
            T: perro_core::NodeTypeDispatch,
        {
            V::default()
        }

        fn mutate_meta<F>(&mut self, _id: NodeID, _f: F)
        where
            F: FnOnce(&mut perro_core::SceneNode),
        {
        }

        fn read_meta<V: Clone + Default>(
            &mut self,
            _node_id: NodeID,
            _f: impl FnOnce(&perro_core::SceneNode) -> V,
        ) -> V {
            V::default()
        }
    }

    impl ScriptAPI for DummyRuntime {
        fn with_state<T: 'static, V, F>(&mut self, _script_id: NodeID, f: F) -> Option<V>
        where
            F: FnOnce(&T) -> V,
        {
            self.state.downcast_ref::<T>().map(f)
        }

        fn with_state_mut<T: 'static, V, F>(&mut self, _script_id: NodeID, f: F) -> Option<V>
        where
            F: FnOnce(&mut T) -> V,
        {
            self.state.downcast_mut::<T>().map(f)
        }

        fn attach_script(&mut self, _node_id: NodeID, _script_path: &str) -> bool {
            false
        }

        fn detach_script(&mut self, _node_id: NodeID) -> bool {
            false
        }

        fn remove_script(&mut self, _script_id: NodeID) -> bool {
            false
        }

        fn get_var(
            &mut self,
            _script_id: NodeID,
            _member: perro_ids::ScriptMemberID,
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
        }

        fn set_var(
            &mut self,
            _script_id: NodeID,
            _member: perro_ids::ScriptMemberID,
            _value: perro_variant::Variant,
        ) {
        }

        fn call_method(
            &mut self,
            _script_id: NodeID,
            _method_id: perro_ids::ScriptMemberID,
            _params: &[perro_variant::Variant],
        ) -> perro_variant::Variant {
            perro_variant::Variant::Null
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
        mutate_node!(&mut ctx, Node2D, id, |_node| {});
        let value = read_node!(&mut ctx, Node2D, id, |_node| 99_i32);
        assert_eq!(value, 0_i32);
        mutate_meta!(&mut ctx, id, |_node| {});
        let top = read_meta!(&mut ctx, id, |_node| 7_i32);
        assert_eq!(top, 0_i32);
        assert!(!attach_script!(&mut ctx, id, "res://scripts/a.rs"));
        assert!(!detach_script!(&mut ctx, id));
        let member = perro_ids::ScriptMemberID::from_string("x");
        let _value = get_var!(&mut ctx, id, member);
        set_var!(&mut ctx, id, member, perro_variant::Variant::Null);
        let _result = call_method!(&mut ctx, id, member, &[]);
        let _result2 = call_method!(&mut ctx, id, member, params![1_i32, "abc"]);

        let dt = delta_time!(&mut ctx);
        let fdt = fixed_delta_time!(&mut ctx);
        let elapsed = elapsed_time!(&mut ctx);
        assert_eq!(dt, 0.016);
        assert_eq!(fdt, 0.016);
        assert_eq!(elapsed, 1.0);
    }
}
