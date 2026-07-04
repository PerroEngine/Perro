use crate::{
    prelude::*,
    sub_apis::{
        AnimPlayerAPI, AnimTreeAPI, RuntimeAudio, RuntimeAudioAPI, SceneAPI, SpatialAudioOptions,
    },
};
use perro_ids::{AnimationID, AudioBusID, IntoTagID, MeshID, NodeID};
use perro_nodes::prelude::{Node2D, NodeTypeDispatch, SceneNodeData, UiLabel};
use perro_resource_api::res_path;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use perro_variant::Variant;
use std::{any::Any, borrow::Cow, time::Duration};

struct DummyRuntime {
    state: Box<dyn Any>,
    gravity: f32,
    coefficient: f32,
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
    fn get_simulation_time(&self) -> Duration {
        Duration::from_micros(1_000)
    }
    fn get_graphics_time(&self) -> Duration {
        Duration::from_micros(2_000)
    }
    fn get_frame_time(&self) -> Duration {
        Duration::from_micros(16_000)
    }
    fn get_fps(&self) -> f32 {
        60.0
    }
}

impl WindowAPI for DummyRuntime {
    fn set_window_title(&mut self, title: impl Into<String>) {
        self.state = Box::new(title.into());
    }

    fn set_window_size(&mut self, width: u32, height: u32) {
        self.state = Box::new((width, height));
    }

    fn set_window_mode(&mut self, mode: WindowMode) {
        self.state = Box::new(mode);
    }

    fn set_frame_rate_cap(&mut self, cap: FrameRateCap) {
        self.state = Box::new(cap);
    }

    fn set_cursor_icon(&mut self, icon: CursorIcon) {
        self.state = Box::new(icon);
    }

    fn close_app(&mut self) {
        self.state = Box::new(WindowRequest::CloseApp);
    }

    fn get_active_refresh_rate(&mut self) -> Option<f32> {
        Some(60.0)
    }
}

impl NodeAPI for DummyRuntime {
    fn create<T>(&mut self) -> NodeID
    where
        T: Default + Into<perro_nodes::SceneNodeData>,
    {
        NodeID::nil()
    }

    fn create_nodes<'a, B>(&mut self, requests: B, _parent_id: NodeID) -> Vec<NodeID>
    where
        B: IntoNodeCreateBatch<'a>,
    {
        let count = match requests.into_node_create_batch() {
            NodeCreateBatch::Specs(specs) => specs.len(),
            NodeCreateBatch::Collection(collection) => collection.specs.len(),
            NodeCreateBatch::OwnedSpecs(specs) => specs.len(),
            NodeCreateBatch::OwnedCollection(collection) => collection.specs.len(),
        };
        (0..count).map(|_| NodeID::nil()).collect()
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

    fn find_node_by_name<S>(&mut self, _root: NodeID, _name: S) -> Option<NodeID>
    where
        S: AsRef<str>,
    {
        None
    }

    fn bind_locale_text<S>(&mut self, _node: NodeID, _key: S) -> bool
    where
        S: AsRef<str>,
    {
        false
    }

    fn bind_locale_placeholder<S>(&mut self, _node: NodeID, _key: S) -> bool
    where
        S: AsRef<str>,
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

    fn mark_needs_rerender(&mut self, _node_id: NodeID) -> bool {
        false
    }

    fn is_mesh_instance_ready(&mut self, _node_id: NodeID) -> bool {
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

    fn get_node_tags(&mut self, _node_id: NodeID) -> Option<Vec<Cow<'static, str>>> {
        None
    }

    fn tag_set<T>(&mut self, _node_id: NodeID, _tags: Option<T>) -> bool
    where
        T: IntoNodeTags,
    {
        false
    }

    fn add_node_tag<T>(&mut self, _node_id: NodeID, _tag: T) -> bool
    where
        T: IntoNodeTag,
    {
        false
    }

    fn remove_node_tag<T>(&mut self, _node_id: NodeID, _tag: T) -> bool
    where
        T: IntoTagID,
    {
        false
    }

    fn query_nodes(&mut self, _query: NodeQueryView<'_>) -> Vec<NodeID> {
        self.state
            .downcast_ref::<Vec<NodeID>>()
            .cloned()
            .unwrap_or_default()
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

    fn mesh_instance_surface_at_global_point(
        &mut self,
        _node_id: NodeID,
        _global_point: perro_structs::Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_instance_surface_on_global_ray(
        &mut self,
        _node_id: NodeID,
        _ray_origin: perro_structs::Vector3,
        _ray_direction: perro_structs::Vector3,
        _max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        None
    }

    fn mesh_instance_surfaces_on_global_rays(
        &mut self,
        _node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        _resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        vec![None; rays.len()]
    }

    fn mesh_instance_material_regions(
        &mut self,
        _node_id: NodeID,
        _material: perro_ids::MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        Vec::new()
    }

    fn mesh_data_surface_at_local_point(
        &mut self,
        _mesh_id: MeshID,
        _local_point: perro_structs::Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        None
    }

    fn mesh_data_surface_on_local_ray(
        &mut self,
        _mesh_id: MeshID,
        _ray_origin_local: perro_structs::Vector3,
        _ray_direction_local: perro_structs::Vector3,
        _max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        None
    }

    fn mesh_data_surface_regions(
        &mut self,
        _mesh_id: MeshID,
        _surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
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

    fn script_set_update_enabled(&mut self, _script: NodeID, enabled: bool) -> bool {
        self.state = Box::new(enabled);
        true
    }

    fn script_set_fixed_update_enabled(&mut self, _script: NodeID, enabled: bool) -> bool {
        self.state = Box::new(enabled);
        true
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
    fn get_gravity(&mut self) -> f32 {
        self.gravity
    }

    fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    fn get_coefficient(&mut self) -> f32 {
        self.coefficient
    }

    fn set_coefficient(&mut self, coefficient: f32) {
        self.coefficient = coefficient;
    }

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

    fn raycast_3d(
        &mut self,
        _origin: Vector3,
        _direction: Vector3,
        _max_distance: f32,
        _include_areas: bool,
    ) -> Option<crate::sub_apis::PhysicsRayHit3D> {
        None
    }

    fn raycast_2d(
        &mut self,
        _origin: Vector2,
        _direction: Vector2,
        _max_distance: f32,
        _filter: crate::sub_apis::PhysicsQueryFilter,
    ) -> Option<crate::sub_apis::PhysicsRayHit2D> {
        None
    }

    fn shape_cast_2d(
        &mut self,
        _shape: perro_nodes::Shape2D,
        _origin: Vector2,
        _direction: Vector2,
        _max_distance: f32,
        _filter: crate::sub_apis::PhysicsQueryFilter,
    ) -> Option<crate::sub_apis::PhysicsShapeHit2D> {
        None
    }

    fn shape_cast_3d(
        &mut self,
        _shape: perro_nodes::Shape3D,
        _origin: Vector3,
        _direction: Vector3,
        _max_distance: f32,
        _filter: crate::sub_apis::PhysicsQueryFilter,
    ) -> Option<crate::sub_apis::PhysicsShapeHit3D> {
        None
    }

    fn contacts_2d(&mut self, _body_id: NodeID) -> Vec<crate::sub_apis::PhysicsContact2D> {
        Vec::new()
    }

    fn contacts_3d(&mut self, _body_id: NodeID) -> Vec<crate::sub_apis::PhysicsContact3D> {
        Vec::new()
    }

    fn physics_pause(&mut self, _paused: bool) {}

    fn physics_is_paused(&mut self) -> bool {
        false
    }
}

impl RuntimeAudioAPI for DummyRuntime {
    fn set_audio_debug_rays(&mut self, _enabled: bool) {}

    fn audio_debug_rays_enabled(&mut self) -> bool {
        false
    }

    fn play_runtime_audio_attached(
        &mut self,
        _bus_id: Option<AudioBusID>,
        _audio: RuntimeAudio<'_>,
        _node_id: NodeID,
        _options: SpatialAudioOptions,
    ) -> bool {
        true
    }

    fn stop_runtime_audio_attached(&mut self, _node_id: NodeID, _source: &str) -> bool {
        true
    }

    fn play_midi_note_attached(
        &mut self,
        _note: Note,
        _node: NodeID,
        _options: MidiNoteOptions,
        _spatial: SpatialAudioOptions,
    ) -> bool {
        true
    }

    fn start_midi_note_attached(
        &mut self,
        _note: Note,
        _node: NodeID,
        _options: MidiNoteOptions,
        _spatial: SpatialAudioOptions,
    ) -> Option<MidiNoteHandle> {
        Some(MidiNoteHandle(1))
    }

    fn play_midi_file_attached(
        &mut self,
        _song: MidiSong,
        _node: NodeID,
        _spatial: SpatialAudioOptions,
    ) -> bool {
        true
    }

    fn release_midi_note(&mut self, _handle: MidiNoteHandle) -> bool {
        true
    }

    fn stop_midi_attached(&mut self, _node: NodeID, _target: AttachedMidiTarget<'_>) -> bool {
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

impl AnimTreeAPI for DummyRuntime {
    fn animation_tree_set_clip_by_name(
        &mut self,
        _tree: NodeID,
        _slot: &str,
        _animation: AnimationID,
    ) -> bool {
        true
    }

    fn animation_tree_set_clip_by_index(
        &mut self,
        _tree: NodeID,
        _slot: usize,
        _animation: AnimationID,
    ) -> bool {
        true
    }

    fn animation_tree_play_slot(&mut self, _tree: NodeID, _slot: &str) -> bool {
        true
    }

    fn animation_tree_pause_slot(&mut self, _tree: NodeID, _slot: &str, _paused: bool) -> bool {
        true
    }

    fn animation_tree_seek_slot_frame(&mut self, _tree: NodeID, _slot: &str, _frame: u32) -> bool {
        true
    }

    fn animation_tree_set_slot_speed(&mut self, _tree: NodeID, _slot: &str, _speed: f32) -> bool {
        true
    }

    fn animation_tree_set_slot_playback(
        &mut self,
        _tree: NodeID,
        _slot: &str,
        _playback_type: perro_nodes::animation_player::AnimationPlaybackType,
    ) -> bool {
        true
    }

    fn animation_tree_seek_node_time(&mut self, _tree: NodeID, _node: &str, _seconds: f32) -> bool {
        true
    }

    fn animation_tree_set_weight(
        &mut self,
        _tree: NodeID,
        _node: &str,
        _input: &str,
        _weight: f32,
    ) -> bool {
        true
    }

    fn animation_tree_pause(&mut self, _tree: NodeID, _paused: bool) -> bool {
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
fn node_collection_typed_nodes_fill_defaults_and_keep_meta() {
    let collection = node_collection![{
        name = "title",
        tags = tags!["ui"],
        node = UiLabel {
            text: {"Paused".into()},
            font_size: 32.0,
        },
        script = res_path!("res://scripts/title.rs"),
    }];

    assert_eq!(collection.specs.len(), 1);
    let spec = &collection.specs[0];
    assert_eq!(spec.name.as_deref(), Some("title"));
    assert_eq!(spec.tags.len(), 1);
    assert_eq!(
        spec.script.as_ref().map(|script| script.path.as_ref()),
        Some("res://scripts/title.rs")
    );
    match &spec.data {
        SceneNodeData::UiLabel(label) => {
            assert_eq!(label.text.as_ref(), "Paused");
            assert_eq!(label.font_size, 32.0);
        }
        other => panic!("expected UiLabel, got {other:?}"),
    }
}

#[test]
fn node_collection_scene_patch_and_script_are_stored() {
    let collection = node_collection![{
        scene = {
            path = res_path!("res://scenes/player.scn"),
            patch = Node2D {
                transform: Transform2D {
                    position: Vector2::new(10.0, 3.0),
                },
            },
        },
        script = res_path!("res://scripts/player.rs"),
    }];

    assert_eq!(collection.scenes.len(), 1);
    let scene = &collection.scenes[0];
    assert_eq!(scene.path.as_ref(), "res://scenes/player.scn");
    assert_eq!(
        scene.script.as_ref().map(|script| script.path.as_ref()),
        Some("res://scripts/player.rs")
    );
    assert_eq!(
        scene.patches.first().map(|patch| patch.node_type()),
        Some(Node2D::NODE_TYPE)
    );
}

#[test]
fn node_collection_script_vars_are_stored() {
    let collection = node_collection![{
        node = Node2D,
        script = {
            path = res_path!("res://scripts/player.rs"),
            vars = {
                hp: 100_i32,
                "title": {"Player".to_string()},
            },
        },
    }];

    let script = collection.specs[0].script.as_ref().expect("script");
    assert_eq!(script.path.as_ref(), "res://scripts/player.rs");
    assert_eq!(script.vars.len(), 2);
    assert_eq!(
        script.vars[0].0,
        perro_ids::ScriptMemberID::from_string("hp")
    );
    assert_eq!(
        script.vars[0].1,
        NodeScriptVar::Value(Variant::from(100_i32))
    );
    assert_eq!(
        script.vars[1].0,
        perro_ids::ScriptMemberID::from_string("title")
    );
    assert_eq!(
        script.vars[1].1,
        NodeScriptVar::Value(Variant::from("Player".to_string()))
    );
}

#[test]
fn node_collection_key_vars_root_and_patch_list_are_stored() {
    let collection = node_collection![
        root: { node = Node2D },
        follower: {
            parent = @root,
            node = Node2D,
            script = {
                path = res_path!("res://scripts/follower.rs"),
                vars = {
                    target: @root,
                },
            },
        },
        {
            scene = {
                path = res_path!("res://scenes/player.scn"),
                patch = [
                    Node2D {
                        transform: Transform2D {
                            position: Vector2::new(1.0, 2.0),
                        },
                    },
                ],
            },
        },
        root = @follower,
    ];

    assert_eq!(collection.root, Some(1));
    let script = collection.specs[1].script.as_ref().expect("script");
    assert_eq!(
        script.vars[0],
        (
            perro_ids::ScriptMemberID::from_string("target"),
            NodeScriptVar::NodeRef(0),
        )
    );
    assert_eq!(collection.scenes[0].patches.len(), 1);
}

#[test]
fn node_collection_key_defaults_name_and_parent_refs() {
    let collection = node_collection![
        root: { node = Node2D },
        sprite: {
            parent = @root,
            node = Node2D,
        },
        {
            parent = @root,
            node = Node2D,
        }
    ];

    assert_eq!(collection.specs.len(), 3);
    assert_eq!(collection.specs[0].name.as_deref(), Some("root"));
    assert_eq!(collection.specs[1].name.as_deref(), Some("sprite"));
    assert_eq!(collection.specs[2].name, None);
    assert_eq!(collection.specs[0].parent, None);
    assert_eq!(collection.specs[1].parent, Some(0));
    assert_eq!(collection.specs[2].parent, Some(0));
}

#[test]
fn node_collection_key_name_can_override_default() {
    let collection = node_collection![
        player: {
            name = "PlayerRoot",
            node = Node2D,
            children = [
                sprite: { node = Node2D },
                { node = Node2D }
            ],
        }
    ];

    assert_eq!(collection.specs.len(), 3);
    assert_eq!(collection.specs[0].name.as_deref(), Some("PlayerRoot"));
    assert_eq!(collection.specs[1].name.as_deref(), Some("sprite"));
    assert_eq!(collection.specs[1].parent, Some(0));
    assert_eq!(collection.specs[2].parent, Some(0));
}

#[test]
fn script_macros_typecheck_and_forward() {
    let mut rt = DummyRuntime {
        state: Box::new(5_i32),
        gravity: -9.81,
        coefficient: 1.0,
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
    let _root_nodes = create_nodes!(
        &mut ctx,
        node_collection![
            { node = Node2D::new() },
            { name = "root", node = Node2D::new() },
        ]
    );
    let _new_nodes = create_nodes!(
        &mut ctx,
        node_collection![{
            name = "child",
            tags = tags!["spawned"],
            node = Node2D::new()
        }],
        id
    );
    with_node_mut!(&mut ctx, Node2D, id, |_node| {});
    let value = with_node!(&mut ctx, Node2D, id, |_node| 99_i32);
    assert_eq!(value, 0_i32);
    let _ = with_base_node!(&mut ctx, Node2D, id, |_node| 1_i32);
    let _ = with_base_node_mut!(&mut ctx, Node2D, id, |_node| 2_i32);
    assert_eq!(get_node_name!(&mut ctx, id), None);
    assert!(!set_node_name!(&mut ctx, id, "player"));
    assert!(!set_ui_rotation!(&mut ctx, id, 0.5));
    assert_eq!(get_node_parent_id!(&mut ctx, id), None);
    assert_eq!(get_node_children_ids!(&mut ctx, id), None);
    assert_eq!(get_node_type!(&mut ctx, id), None);
    assert_eq!(get_node_tags!(&mut ctx, id), None);
    assert!(!tag_set!(&mut ctx, id, tags!["player", "enemy"]));
    assert!(!tag_set!(&mut ctx, id));
    assert!(!tag_add!(&mut ctx, id, "player"));
    assert!(!tag_remove!(&mut ctx, id, "player"));
    assert!(query!(&mut ctx, all(tags["player"], not(tags["enemy"]))).is_empty());
    let player_tag = "player".to_string();
    assert!(query!(&mut ctx, all(tags[player_tag.as_str()])).is_empty());
    assert!(query!(&mut ctx, all(node_type[Node2D], base_type[Node3D])).is_empty());
    assert!(query!(&mut ctx, all(layers[1], mask[2, 3])).is_empty());
    let layer = 4usize;
    assert!(query!(&mut ctx, all(layers[layer], mask[layer])).is_empty());
    let expr = query_expr!(all(tags["player"], not(tags["enemy"])));
    assert!(matches!(expr, QueryExpr::All(_)));
    let reusable_query = query_builder!(all(tags["player"]), in_subtree(id));
    assert_eq!(reusable_query.scope, QueryScope::Subtree(id));
    assert!(query!(&mut ctx, &reusable_query).is_empty());
    let original_scope = reusable_query.scope;
    assert!(query!(&mut ctx, &reusable_query, in_subtree(NodeID::new(7))).is_empty());
    assert_eq!(reusable_query.scope, original_scope);
    assert!(query!(&mut ctx, query_builder!(all(tags["player"]))).is_empty());
    assert!(query_first!(&mut ctx, &reusable_query).is_none());
    let direct_query = NodeQuery::new().where_expr(QueryExpr::Name(vec!["Player".to_string()]));
    assert!(ctx.NodeQuery().query(&direct_query).is_empty());
    assert!(!reparent!(&mut ctx, NodeID::new(1), id));
    assert_eq!(reparent_multi!(&mut ctx, NodeID::new(1), [id]), 0);
    assert!(!remove_node!(&mut ctx, id));
    assert_eq!(get_global_transform_2d!(&mut ctx, id), None);
    assert_eq!(get_global_transform_3d!(&mut ctx, id), None);
    assert_eq!(get_local_transform_2d!(&mut ctx, id), None);
    assert_eq!(get_local_transform_3d!(&mut ctx, id), None);
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
    assert!(!set_local_transform_2d!(
        &mut ctx,
        id,
        Transform2D::new(Vector2::new(1.0, 2.0), 0.5, Vector2::ONE)
    ));
    assert!(!set_local_transform_3d!(
        &mut ctx,
        id,
        Transform3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Quaternion::IDENTITY,
            Vector3::ONE
        )
    ));
    assert_eq!(get_local_pos_2d!(&mut ctx, id), None);
    assert_eq!(get_local_pos_3d!(&mut ctx, id), None);
    assert!(!set_local_pos_2d!(&mut ctx, id, Vector2::new(1.0, 2.0)));
    assert!(!set_local_pos_3d!(
        &mut ctx,
        id,
        Vector3::new(1.0, 2.0, 3.0)
    ));
    assert_eq!(get_global_pos_2d!(&mut ctx, id), None);
    assert_eq!(get_global_pos_3d!(&mut ctx, id), None);
    assert!(!set_global_pos_2d!(&mut ctx, id, Vector2::new(1.0, 2.0)));
    assert!(!set_global_pos_3d!(
        &mut ctx,
        id,
        Vector3::new(1.0, 2.0, 3.0)
    ));
    assert_eq!(get_local_rot_2d!(&mut ctx, id), None);
    assert_eq!(get_local_rot_3d!(&mut ctx, id), None);
    assert!(!set_local_rot_2d!(&mut ctx, id, 0.5));
    assert!(!set_local_rot_3d!(&mut ctx, id, Quaternion::IDENTITY));
    assert_eq!(get_global_rot_2d!(&mut ctx, id), None);
    assert_eq!(get_global_rot_3d!(&mut ctx, id), None);
    assert!(!set_global_rot_2d!(&mut ctx, id, 0.5));
    assert!(!set_global_rot_3d!(&mut ctx, id, Quaternion::IDENTITY));
    assert_eq!(get_local_scale_2d!(&mut ctx, id), None);
    assert_eq!(get_local_scale_3d!(&mut ctx, id), None);
    assert!(!set_local_scale_2d!(&mut ctx, id, Vector2::ONE));
    assert!(!set_local_scale_3d!(&mut ctx, id, Vector3::ONE));
    assert_eq!(get_global_scale_2d!(&mut ctx, id), None);
    assert_eq!(get_global_scale_3d!(&mut ctx, id), None);
    assert!(!set_global_scale_2d!(&mut ctx, id, Vector2::ONE));
    assert!(!set_global_scale_3d!(&mut ctx, id, Vector3::ONE));
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
        mesh_instance_surface_at_global_point_3d!(&mut ctx, id, Vector3::new(0.0, 0.0, 0.0)),
        None
    );
    assert_eq!(
        mesh_instance_surface_on_global_ray_3d!(
            &mut ctx,
            id,
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0
        ),
        None
    );
    assert_eq!(
        mesh_instance_surfaces_on_global_rays_3d!(
            &mut ctx,
            id,
            &[MeshSurfaceRay3D {
                origin: Vector3::new(0.0, 1.0, 0.0),
                direction: Vector3::new(0.0, -1.0, 0.0),
                max_distance: 100.0,
            }],
            false
        ),
        vec![None]
    );
    let direct_hits = ctx.MeshQuery().instance_surfaces_on_global_rays(
        id,
        &[MeshSurfaceRay3D {
            origin: Vector3::new(0.0, 1.0, 0.0),
            direction: Vector3::new(0.0, -1.0, 0.0),
            max_distance: 100.0,
        }],
        false,
    );
    assert_eq!(direct_hits, vec![None]);
    assert!(
        mesh_instance_material_regions_3d!(&mut ctx, id, perro_ids::MaterialID::new(1)).is_empty()
    );
    assert!(apply_force!(&mut ctx, id, Vector2::new(8.0, 0.0)));
    assert!(apply_force!(&mut ctx, id, Vector3::new(0.0, 3.5, 0.0)));
    assert!(apply_impulse!(&mut ctx, id, Vector2::new(0.0, 1.25)));
    assert!(apply_impulse!(&mut ctx, id, Vector3::new(2.75, 0.0, 0.0)));
    assert_eq!(physics_predict_body_2d!(&mut ctx, id, 1.0), None);
    assert_eq!(
        physics_predict_body_3d!(&mut ctx, id, 1.0, Vector3::new(0.5, 0.0, 0.0)),
        None
    );
    assert_eq!(
        physics_raycast_3d!(
            &mut ctx,
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0
        ),
        None
    );
    assert_eq!(
        physics_raycast_3d_with_areas!(
            &mut ctx,
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0
        ),
        None
    );
    assert_eq!(
        physics_raycast_3d_without_areas!(
            &mut ctx,
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(0.0, -1.0, 0.0),
            100.0
        ),
        None
    );
    physics_pause!(&mut ctx, true);
    assert!(!physics_is_paused!(&mut ctx));
    assert!(!script_attach!(&mut ctx, id, "res://scripts/a.rs"));
    assert!(!script_detach!(&mut ctx, id));
    assert!(script_set_update_enabled!(&mut ctx, id, false));
    assert!(script_set_fixed_update_enabled!(&mut ctx, id, true));
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
    assert_eq!(
        signal_connect_many!(
            &mut ctx,
            id,
            &[signal!("on_a"), signal!("on_b")],
            [func!("handle_many")]
        ),
        2
    );
    assert_eq!(
        signal_connect_many!(
            &mut ctx,
            id,
            [signal!("on_c")],
            &[func!("handle_c"), func!("handle_c_extra")],
            params!["button_b"]
        ),
        2
    );
    assert_eq!(
        ctx.Signals().signal_connect_many(
            id,
            vec![signal!("on_d"), signal!("on_e")],
            vec![func!("handle_d"), func!("handle_e")],
            &[]
        ),
        4
    );
    assert!(signal_disconnect!(
        &mut ctx,
        id,
        signal!("on_test"),
        method!("handle")
    ));
    assert_eq!(
        signal_disconnect_many!(
            &mut ctx,
            id,
            &[signal!("on_a"), signal!("on_b")],
            [func!("handle_many")]
        ),
        2
    );
    assert_eq!(
        signal_disconnect_many!(
            &mut ctx,
            id,
            [signal!("on_c")],
            &[func!("handle_c"), func!("handle_c_extra")]
        ),
        2
    );
    assert_eq!(
        ctx.Signals().signal_disconnect_many(
            id,
            vec![signal!("on_d"), signal!("on_e")],
            vec![func!("handle_d"), func!("handle_e")]
        ),
        4
    );
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
    let sim = simulation_time!(&mut ctx);
    let gfx = graphics_time!(&mut ctx);
    let frame = frame_time!(&mut ctx);
    let fps_value = fps!(&mut ctx);
    let profile = profiling!(&mut ctx);
    assert_eq!(dt, 0.016);
    assert_eq!(dt_capped, 0.010);
    assert_eq!(dt_clamped, 0.020);
    assert_eq!(fdt, 0.016);
    assert_eq!(elapsed, 1.0);
    assert_eq!(sim, Duration::from_micros(1_000));
    assert_eq!(gfx, Duration::from_micros(2_000));
    assert_eq!(frame, Duration::from_micros(16_000));
    assert_eq!(fps_value, 60.0);
    assert_eq!(
        profile,
        ProfilingSnapshot {
            simulation_time: Duration::from_micros(1_000),
            graphics_time: Duration::from_micros(2_000),
            frame_time: Duration::from_micros(16_000),
            fps: 60.0,
            draw_gpu_prepare_3d: Duration::ZERO,
            draw_gpu_prepare_3d_frustum: Duration::ZERO,
            draw_gpu_prepare_3d_hiz: Duration::ZERO,
            draw_gpu_prepare_3d_indirect: Duration::ZERO,
            draw_gpu_prepare_3d_cull_inputs: Duration::ZERO,
            draw_calls_2d: 0,
            draw_calls_3d: 0,
            draw_calls_total: 0,
            draw_instances_3d: 0,
            draw_material_refs_3d: 0,
            skip_prepare_3d: 0,
            skip_prepare_3d_frustum: 0,
            skip_prepare_3d_hiz: 0,
            skip_prepare_3d_indirect: 0,
            skip_prepare_3d_cull_inputs: 0,
        }
    );
}

#[test]
fn node_query_iterator_macros_typecheck_and_forward() {
    let ids = vec![NodeID::new(1), NodeID::new(2), NodeID::new(3)];
    let mut rt = DummyRuntime {
        state: Box::new(ids.clone()),
        gravity: -9.81,
        coefficient: 1.0,
    };
    let mut ctx = RuntimeWindow::new(&mut rt);
    let parent = NodeID::new(99);

    let iter_hits = query_iter!(&mut ctx, all(tags["enemy"])).collect::<Vec<_>>();
    assert_eq!(iter_hits, ids);

    let subtree_hits =
        query_iter!(&mut ctx, all(tags["enemy"]), in_subtree(parent)).collect::<Vec<_>>();
    assert_eq!(subtree_hits, ids);

    let reusable_query = query_builder!(all(tags["enemy"]));
    let reusable_hits = query_iter!(&mut ctx, &reusable_query).collect::<Vec<_>>();
    assert_eq!(reusable_hits, ids);

    let reusable_subtree_hits =
        query_iter!(&mut ctx, &reusable_query, in_subtree(parent)).collect::<Vec<_>>();
    assert_eq!(reusable_subtree_hits, ids);

    let module_hits = ctx
        .NodeQuery()
        .query_iter(&reusable_query)
        .collect::<Vec<_>>();
    assert_eq!(module_hits, ids);

    let mut each_count = 0;
    query_each!(&mut ctx, all(tags["enemy"]), |id| {
        let _ = get_node_name!(&mut ctx, id);
        each_count += 1;
    });
    assert_eq!(each_count, ids.len());

    let mapped = query_map!(&mut ctx, all(tags["enemy"]), |id| id.index());
    assert_eq!(mapped, vec![1, 2, 3]);
}

fn dummy_runtime() -> DummyRuntime {
    DummyRuntime {
        state: Box::new(0_i32),
        gravity: -9.81,
        coefficient: 1.0,
    }
}

fn assert_vec2_close(actual: Vector2, expected: Vector2, epsilon: f32) {
    assert!(
        (actual - expected).length() <= epsilon,
        "actual={actual:?} expected={expected:?}"
    );
}

fn assert_vec3_close(actual: Vector3, expected: Vector3, epsilon: f32) {
    assert!(
        (actual - expected).length() <= epsilon,
        "actual={actual:?} expected={expected:?}"
    );
}

fn simulate_2d(
    origin: Vector2,
    velocity: Vector2,
    drift: Vector2,
    gravity: f32,
    time: f32,
) -> Vector2 {
    origin + (velocity + drift) * time + Vector2::new(0.0, gravity) * (0.5 * time * time)
}

fn simulate_3d(
    origin: Vector3,
    velocity: Vector3,
    drift: Vector3,
    gravity: f32,
    time: f32,
) -> Vector3 {
    origin + (velocity + drift) * time + Vector3::new(0.0, gravity, 0.0) * (0.5 * time * time)
}

#[test]
fn close_app_macro_queues_window_close_request() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);

    close_app!(&mut ctx);

    assert_eq!(
        rt.state.downcast_ref::<WindowRequest>(),
        Some(&WindowRequest::CloseApp)
    );
}

#[test]
fn physics_solve_velocity_to_target_2d_hits_target() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);
    let origin = Vector2::new(0.0, 0.0);
    let target = Vector2::new(12.0, 3.0);
    let time = 1.5;

    let velocity = physics_solve_velocity_to_target_2d!(&mut ctx, origin, target, time).unwrap();
    let hit = simulate_2d(origin, velocity, Vector2::ZERO, -9.81, time);

    assert_vec2_close(hit, target, 1.0e-4);
}

#[test]
fn physics_solve_velocity_to_target_3d_hits_target_with_drift() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);
    let origin = Vector3::new(0.0, 1.0, 0.0);
    let target = Vector3::new(8.0, 2.0, -4.0);
    let drift = Vector3::new(1.0, 0.0, -0.5);
    let time = 1.25;

    let velocity =
        physics_solve_velocity_to_target_3d!(&mut ctx, origin, target, time, drift).unwrap();
    let hit = simulate_3d(origin, velocity, drift, -9.81, time);

    assert_vec3_close(hit, target, 1.0e-4);
}

#[test]
fn physics_solve_launch_velocity_2d_returns_low_and_high_arcs() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);
    let origin = Vector2::new(0.0, 0.0);
    let target = Vector2::new(10.0, 0.0);
    let speed = 12.0;

    let solution = physics_solve_launch_velocity_2d!(&mut ctx, origin, target, speed, 5.0).unwrap();
    let low_time = 10.0 / solution.low.x;
    let high_time = 10.0 / solution.high.x;
    let low_hit = simulate_2d(origin, solution.low, Vector2::ZERO, -9.81, low_time);
    let high_hit = simulate_2d(origin, solution.high, Vector2::ZERO, -9.81, high_time);

    assert!(
        low_time < high_time,
        "low_time={low_time} high_time={high_time}"
    );
    assert_vec2_close(low_hit, target, 2.0e-3);
    assert_vec2_close(high_hit, target, 2.0e-3);
    assert!((solution.low.length() - speed).abs() < 2.0e-3);
    assert!((solution.high.length() - speed).abs() < 2.0e-3);
}

#[test]
fn physics_solve_launch_velocity_3d_returns_none_when_unreachable() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);

    let solution = physics_solve_launch_velocity_3d!(
        &mut ctx,
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(100.0, 0.0, 0.0),
        1.0,
        4.0
    );

    assert_eq!(solution, None);
}

#[test]
fn physics_trajectory_solver_rejects_invalid_inputs() {
    let mut rt = dummy_runtime();
    let mut ctx = RuntimeWindow::new(&mut rt);

    assert_eq!(
        physics_solve_velocity_to_target_2d!(&mut ctx, Vector2::ZERO, Vector2::new(1.0, 0.0), 0.0),
        None
    );
    assert_eq!(
        physics_solve_velocity_to_target_3d!(&mut ctx, Vector3::ZERO, Vector3::ZERO, 1.0),
        None
    );
    assert_eq!(
        physics_solve_launch_velocity_2d!(
            &mut ctx,
            Vector2::ZERO,
            Vector2::new(1.0, 0.0),
            0.0,
            1.0
        ),
        None
    );
    assert_eq!(
        physics_solve_launch_velocity_3d!(
            &mut ctx,
            Vector3::ZERO,
            Vector3::new(1.0, 0.0, 0.0),
            1.0,
            0.0
        ),
        None
    );
}

#[test]
fn physics_trajectory_solver_uses_gravity_coefficient() {
    let mut rt = dummy_runtime();
    rt.gravity = -5.0;
    rt.coefficient = 2.0;
    let mut ctx = RuntimeWindow::new(&mut rt);

    let velocity =
        physics_solve_velocity_to_target_2d!(&mut ctx, Vector2::ZERO, Vector2::new(10.0, 0.0), 1.0)
            .unwrap();

    assert_vec2_close(velocity, Vector2::new(10.0, 5.0), 1.0e-6);
}
