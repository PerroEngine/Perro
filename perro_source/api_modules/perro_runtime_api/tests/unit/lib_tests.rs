use crate::{
    prelude::*,
    sub_apis::{
        AnimPlayerAPI, AnimTreeAPI, RuntimeAudio, RuntimeAudioAPI, SceneAPI, SpatialAudioOptions,
    },
};
use perro_ids::{AnimationID, AudioBusID, IntoTagID, MeshID, NodeID};
use perro_nodes::prelude::{Node2D, NodeTypeDispatch, SceneNodeData, UiLabel};
use perro_resource_api::{LoadError, res_path};
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

impl TimerAPI for DummyRuntime {
    fn timer_start(
        &mut self,
        _duration: std::time::Duration,
        _timer: perro_ids::TimerID,
        _started: perro_ids::SignalID,
        _finished: perro_ids::SignalID,
    ) {
    }

    fn timer_cancel(&mut self, _timer: perro_ids::TimerID) -> bool {
        true
    }

    fn timer_is_active(&self, _timer: perro_ids::TimerID) -> bool {
        false
    }

    fn timer_remaining(&self, _timer: perro_ids::TimerID) -> Option<std::time::Duration> {
        None
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

    fn mesh_instance_surface_global_point(
        &mut self,
        _node_id: NodeID,
        _triangle_index: u32,
        _barycentric: perro_structs::Vector3,
    ) -> Option<perro_structs::Vector3> {
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

    fn scene_drop_preloaded(&mut self, id: PreloadedSceneID) -> bool {
        id == PreloadedSceneID::from_u64(11)
    }

    fn scene_drop_preloaded_by_path(&mut self, path: &str) -> bool {
        path == "res://scenes/preloaded.scene"
    }
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

include!("lib_tests/core.rs");
include!("lib_tests/nodes.rs");
include!("lib_tests/scene.rs");
include!("lib_tests/audio.rs");
