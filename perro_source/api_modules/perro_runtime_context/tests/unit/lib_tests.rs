use crate::{
    prelude::*,
    sub_apis::{AnimPlayerAPI, SceneAPI},
};
use perro_ids::{AnimationID, IntoTagID, NodeID, TagID};
use perro_nodes::prelude::{Node2D, UiRect, UiSizeMode};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
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

    fn force_rerender(&mut self, _root_id: NodeID) -> bool {
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

    fn tag_set<T>(&mut self, _node_id: NodeID, _tags: Option<T>) -> bool
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

    fn get_global_transform_2d(&mut self, _node_id: NodeID) -> Option<perro_structs::Transform2D> {
        None
    }

    fn get_global_transform_3d(&mut self, _node_id: NodeID) -> Option<perro_structs::Transform3D> {
        None
    }

    fn set_global_transform_2d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Transform2D,
    ) -> bool {
        false
    }

    fn set_global_transform_3d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Transform3D,
    ) -> bool {
        false
    }

    fn to_global_point_2d(
        &mut self,
        _node_id: NodeID,
        _local: perro_structs::Vector2,
    ) -> Option<perro_structs::Vector2> {
        None
    }

    fn to_local_point_2d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Vector2,
    ) -> Option<perro_structs::Vector2> {
        None
    }

    fn to_global_point_3d(
        &mut self,
        _node_id: NodeID,
        _local: perro_structs::Vector3,
    ) -> Option<perro_structs::Vector3> {
        None
    }

    fn to_local_point_3d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Vector3,
    ) -> Option<perro_structs::Vector3> {
        None
    }

    fn to_global_transform_2d(
        &mut self,
        _node_id: NodeID,
        _local: perro_structs::Transform2D,
    ) -> Option<perro_structs::Transform2D> {
        None
    }

    fn to_local_transform_2d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Transform2D,
    ) -> Option<perro_structs::Transform2D> {
        None
    }

    fn to_global_transform_3d(
        &mut self,
        _node_id: NodeID,
        _local: perro_structs::Transform3D,
    ) -> Option<perro_structs::Transform3D> {
        None
    }

    fn to_local_transform_3d(
        &mut self,
        _node_id: NodeID,
        _global: perro_structs::Transform3D,
    ) -> Option<perro_structs::Transform3D> {
        None
    }

    fn mesh_instance_surface_at_world_point(
        &mut self,
        _node_id: NodeID,
        _world_point: perro_structs::Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_instance_surface_on_world_ray(
        &mut self,
        _node_id: NodeID,
        _ray_origin: perro_structs::Vector3,
        _ray_direction: perro_structs::Vector3,
        _max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_instance_material_regions(
        &mut self,
        _node_id: NodeID,
        _material: perro_ids::MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        Vec::new()
    }

    fn mesh_data_surface_at_world_point(
        &mut self,
        _node_id: NodeID,
        _world_point: perro_structs::Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_data_surface_on_world_ray(
        &mut self,
        _node_id: NodeID,
        _ray_origin: perro_structs::Vector3,
        _ray_direction: perro_structs::Vector3,
        _max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_data_surface_regions(
        &mut self,
        _node_id: NodeID,
        _surface_index: u32,
    ) -> Vec<MeshMaterialRegion3D> {
        Vec::new()
    }
}

impl ScriptAPI for DummyRuntime {
    fn with_state<T: 'static, V: Default, F>(&mut self, _script: NodeID, f: F) -> V
    where
        F: FnOnce(&T) -> V,
    {
        self.state.downcast_ref::<T>().map(f).unwrap_or_default()
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

    fn script_attach_hashed(&mut self, _node: NodeID, _script_path_hash: u64) -> bool {
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
        _params: &[perro_variant::Variant],
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

impl PhysicsAPI for DummyRuntime {
    fn apply_force_2d(&mut self, _body_id: NodeID, _force: Vector2) -> bool {
        true
    }

    fn apply_force_3d(&mut self, _body_id: NodeID, _force: Vector3) -> bool {
        true
    }

    fn apply_impulse_2d(&mut self, _body_id: NodeID, _impulse: Vector2) -> bool {
        true
    }

    fn apply_impulse_3d(&mut self, _body_id: NodeID, _impulse: Vector3) -> bool {
        true
    }
}

impl AnimPlayerAPI for DummyRuntime {
    fn animation_set_clip(&mut self, _player: NodeID, _animation: AnimationID) -> bool {
        true
    }

    fn animation_play(&mut self, _player: NodeID) -> bool {
        true
    }

    fn animation_pause(&mut self, _player: NodeID, _paused: bool) -> bool {
        true
    }

    fn animation_seek_frame(&mut self, _player: NodeID, _frame: u32) -> bool {
        true
    }

    fn animation_set_speed(&mut self, _player: NodeID, _speed: f32) -> bool {
        true
    }

    fn animation_bind(&mut self, _player: NodeID, _track: &str, _node: NodeID) -> bool {
        true
    }

    fn animation_clear_bindings(&mut self, _player: NodeID) -> bool {
        true
    }
}

impl SceneAPI for DummyRuntime {
    fn scene_load(&mut self, _path: &str) -> Result<NodeID, String> {
        Ok(NodeID::new(7))
    }

    fn scene_preload(&mut self, _path: &str) -> Result<PreloadedSceneID, String> {
        Ok(PreloadedSceneID::from_u64(11))
    }

    fn scene_load_preloaded(&mut self, id: PreloadedSceneID) -> Result<NodeID, String> {
        if id == PreloadedSceneID::from_u64(11) {
            Ok(NodeID::new(8))
        } else {
            Err("bad preloaded scene id".to_string())
        }
    }

    fn scene_free_preloaded(&mut self, id: PreloadedSceneID) -> bool {
        id == PreloadedSceneID::from_u64(11)
    }

    fn scene_free_preloaded_by_path(&mut self, path: &str) -> bool {
        path == "res://scenes/preloaded.scene"
    }
}

#[test]
fn script_macros_typecheck_and_forward() {
    let mut rt = DummyRuntime {
        state: Box::new(5_i32),
    };
    let mut ctx = RuntimeWindow::new(&mut rt);
    let id = NodeID::new(42);

    let initial = with_state!(&mut ctx, i32, id, |state| *state);
    assert_eq!(initial, 5);

    let _ = with_state_mut!(&mut ctx, i32, id, |state| {
        *state += 7;
    });
    let updated = with_state!(&mut ctx, i32, id, |state| *state);
    assert_eq!(updated, 12);

    let _new_node = create_node!(&mut ctx, Node2D);
    with_node_mut!(&mut ctx, Node2D, id, |_node| {});
    let value = with_node!(&mut ctx, Node2D, id, |_node| 99_i32);
    assert_eq!(value, 0_i32);
    let _ = with_base_node!(&mut ctx, Node2D, id, |_node| 1_i32);
    let _ = with_base_node_mut!(&mut ctx, Node2D, id, |_node| 2_i32);
    assert_eq!(get_node_name!(&mut ctx, id), None);
    assert!(!set_node_name!(&mut ctx, id, "player"));
    assert!(!set_ui_min_size!(&mut ctx, id, Vector2::new(100.0, 50.0)));
    assert!(!set_ui_max_size!(&mut ctx, id, Vector2::new(1200.0, 96.0)));
    assert!(!set_ui_scale!(&mut ctx, id, Vector2::new(2.0, 0.5)));
    assert!(!set_ui_rotation!(&mut ctx, id, 0.5));
    assert!(!set_ui_padding!(&mut ctx, id, UiRect::all(8.0)));
    assert!(!set_ui_margin!(&mut ctx, id, UiRect::symmetric(4.0, 2.0)));
    assert!(!set_ui_h_size!(&mut ctx, id, UiSizeMode::Fill));
    assert!(!set_ui_v_size!(&mut ctx, id, UiSizeMode::FitChildren));
    assert!(!set_ui_min_w!(&mut ctx, id, 100.0));
    assert!(!set_ui_min_h!(&mut ctx, id, 50.0));
    assert!(!set_ui_max_w!(&mut ctx, id, 1200.0));
    assert!(!set_ui_max_h!(&mut ctx, id, 96.0));
    assert_eq!(get_node_parent_id!(&mut ctx, id), None);
    assert_eq!(get_node_children_ids!(&mut ctx, id), None);
    assert_eq!(get_node_type!(&mut ctx, id), None);
    assert_eq!(get_node_tags!(&mut ctx, id), None);
    assert!(!tag_set!(&mut ctx, id, tags!["player", "enemy"]));
    assert!(!tag_set!(&mut ctx, id));
    assert!(!tag_add!(&mut ctx, id, "player"));
    assert!(!tag_remove!(&mut ctx, id, "player"));
    assert!(query!(&mut ctx, all(tags["player"], not(tags["enemy"]))).is_empty());
    assert!(query!(&mut ctx, all(is[Node2D], base[Node3D])).is_empty());
    assert!(!reparent!(&mut ctx, NodeID::new(1), id));
    assert_eq!(reparent_multi!(&mut ctx, NodeID::new(1), [id]), 0);
    assert!(!remove_node!(&mut ctx, id));
    assert_eq!(get_global_transform_2d!(&mut ctx, id), None);
    assert_eq!(get_global_transform_3d!(&mut ctx, id), None);
    assert!(!set_global_transform_2d!(
        &mut ctx,
        id,
        Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
    ));
    assert!(!set_global_transform_3d!(
        &mut ctx,
        id,
        Transform3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        )
    ));
    assert_eq!(
        to_global_point_2d!(&mut ctx, id, Vector2::new(1.0, 0.0)),
        None
    );
    assert_eq!(
        to_local_point_2d!(&mut ctx, id, Vector2::new(1.0, 0.0)),
        None
    );
    assert_eq!(
        to_global_point_3d!(&mut ctx, id, Vector3::new(1.0, 0.0, 0.0)),
        None
    );
    assert_eq!(
        to_local_point_3d!(&mut ctx, id, Vector3::new(1.0, 0.0, 0.0)),
        None
    );
    assert_eq!(
        to_global_transform_2d!(
            &mut ctx,
            id,
            Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
        ),
        None
    );
    assert_eq!(
        to_local_transform_2d!(
            &mut ctx,
            id,
            Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
        ),
        None
    );
    assert_eq!(
        to_global_transform_3d!(
            &mut ctx,
            id,
            Transform3D::new(
                Vector3::new(1.0, 2.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            )
        ),
        None
    );
    assert_eq!(
        to_local_transform_3d!(
            &mut ctx,
            id,
            Transform3D::new(
                Vector3::new(1.0, 2.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE
            )
        ),
        None
    );
    assert_eq!(
        mesh_surface_at_world_point_3d!(&mut ctx, id, Vector3::new(0.0, 0.0, 0.0)),
        None
    );
    assert_eq!(
        mesh_surface_on_world_ray_3d!(
            &mut ctx,
            id,
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0
        ),
        None
    );
    assert!(mesh_material_regions_3d!(&mut ctx, id, perro_ids::MaterialID::new(1)).is_empty());
    assert!(apply_force!(&mut ctx, id, Vector2::new(8.0, 0.0)));
    assert!(apply_force!(&mut ctx, id, Vector3::new(0.0, 3.5, 0.0)));
    assert!(apply_impulse!(&mut ctx, id, Vector2::new(0.0, 1.25)));
    assert!(apply_impulse!(&mut ctx, id, Vector3::new(2.75, 0.0, 0.0)));
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
    assert!(signal_connect!(
        &mut ctx,
        id,
        signal!("on_test_with_params"),
        method!("handle"),
        params!["button_a"]
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
    assert_eq!(
        scene_load!(&mut ctx, "res://scenes/a.scene"),
        Ok(NodeID::new(7))
    );
    assert_eq!(
        scene_load!(&mut ctx, String::from("res://scenes/b.scene")),
        Ok(NodeID::new(7))
    );
    let cow_path = std::borrow::Cow::Borrowed("res://scenes/c.scene");
    assert_eq!(scene_load!(&mut ctx, cow_path), Ok(NodeID::new(7)));
    let preloaded = scene_preload!(&mut ctx, "res://scenes/preloaded.scene")
        .expect("preload should return deterministic id");
    assert_eq!(preloaded, PreloadedSceneID::from_u64(11));
    assert_eq!(scene_load!(&mut ctx, preloaded), Ok(NodeID::new(8)));
    assert!(scene_free_preloaded!(&mut ctx, preloaded));
    assert!(scene_free_preloaded!(
        &mut ctx,
        "res://scenes/preloaded.scene"
    ));

    let dt = delta_time!(&mut ctx);
    let dt_capped = delta_time_capped!(&mut ctx, 0.010);
    let dt_clamped = delta_time_clamped!(&mut ctx, 0.020, 0.030);
    let fdt = fixed_delta_time!(&mut ctx);
    let elapsed = elapsed_time!(&mut ctx);
    assert_eq!(dt, 0.016);
    assert_eq!(dt_capped, 0.010);
    assert_eq!(dt_clamped, 0.020);
    assert_eq!(fdt, 0.016);
    assert_eq!(elapsed, 1.0);
}

