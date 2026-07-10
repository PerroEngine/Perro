//! Public runtime scripting API.
//!
//! This crate exposes the runtime-side script surface: time, window requests,
//! node access, node queries, scripts, signals, physics, animation, scene
//! loading, and runtime audio. Scripts normally import [`prelude`] and receive a
//! [`RuntimeApiSurface`] from the script context.

pub mod api;
pub mod sub_apis;

// ---- Core engine re-exports ----

pub use perro_ids;
#[doc(hidden)]
pub use perro_ids::string_to_u64 as __perro_string_to_u64;
pub use perro_nodes;
pub use perro_structs;
pub use perro_variant;

// ---- Window facade ----

pub use api::{RuntimeApiSurface, RuntimeWindow};

/// Common imports for scripts that use runtime APIs.
pub mod prelude {
    // Facade traits and module accessors.
    pub use crate::api::{RuntimeAPI, RuntimeApiSurface, RuntimeWindow};

    // Runtime domain APIs.
    pub use crate::sub_apis::{
        AnimPlayerAPI, AnimPlayerModule, AttachedMidiTarget, CameraRay3D, CursorIcon, FrameRateCap,
        IntoImpulseDirection, IntoNodeCollection, IntoNodeCreateBatch, IntoNodeTag, IntoNodeTags,
        IntoPreloadedSceneID, IntoPreloadedSceneTarget, IntoSceneLoadSource, IntoScenePath,
        IntoScriptMemberID, MeshDataSurfaceHit3D, MeshDataSurfaceRegion3D, MeshMaterialRegion3D,
        MeshQueryModule, MeshSurfaceHit3D, MeshSurfaceRay3D, MidiChannel, MidiNoteHandle,
        MidiNoteOptions, MidiProgram, MidiSong, MidiSound, NavMeshAPI, NavMeshModule,
        NavMeshPath3D, NavMeshPathOptions, NavMeshPathStatus, NodeAPI, NodeCollection,
        NodeCollectionEntry, NodeCreateBatch, NodeModule, NodeQuery, NodeQueryModule,
        NodeQueryView, NodeSceneSpec, NodeScriptSpec, NodeScriptVar, NodeSpec, Note, PhysicsAPI,
        PhysicsBodyPrediction2D, PhysicsBodyPrediction3D, PhysicsLaunchSolution2D,
        PhysicsLaunchSolution3D, PhysicsModule, PhysicsMoveResult2D, PhysicsMoveResult3D,
        PhysicsQueryFilter, PhysicsRayHit2D, PhysicsRayHit3D, PhysicsShapeHit2D, PhysicsShapeHit3D,
        PhysicsSlideResult2D, PhysicsSlideResult3D, PreloadedSceneTarget, ProfilingSnapshot,
        QueryBounds, QueryExpr, QueryScope, RuntimeMidiModule, SceneAPI, SceneLoadSource,
        SceneModule, ScriptAPI, ScriptModule, SignalAPI, SignalModule, SpatialAudioOptions,
        TimeAPI, TimeModule, WindowAPI, WindowMode, WindowModule, WindowRequest, program,
    };

    // Convenience macros.
    #[allow(deprecated)]
    pub use crate::{
        anim_player_bind, anim_player_clear_bindings, anim_player_pause, anim_player_play,
        anim_player_seek_frame, anim_player_set_clip, anim_player_set_speed, apply_force,
        apply_impulse, audio_play_attached, bind_locale_placeholder, bind_locale_text,
        broadcast_var, call_method, close_app, create_node, create_nodes, delta_time,
        delta_time_capped, delta_time_clamped, descendants, elapsed_time, find_node,
        fixed_delta_time, force_rerender, fps, frame_time, get_child, get_children,
        get_global_pos_2d, get_global_pos_3d, get_global_rot_2d, get_global_rot_3d,
        get_global_scale_2d, get_global_scale_3d, get_global_transform_2d, get_global_transform_3d,
        get_local_pos_2d, get_local_pos_3d, get_local_rot_2d, get_local_rot_3d, get_local_scale_2d,
        get_local_scale_3d, get_local_transform_2d, get_local_transform_3d, get_node_children_ids,
        get_node_name, get_node_parent_id, get_node_tags, get_node_type, get_node_var, get_var,
        graphics_time, is_mesh_instance_ready, look_at_3d, mesh_data_surface_at_local_point_3d,
        mesh_data_surface_on_local_ray_3d, mesh_data_surface_regions_3d,
        mesh_instance_material_regions_3d, mesh_instance_surface_at_global_point_3d,
        mesh_instance_surface_global_point_3d, mesh_instance_surface_on_global_ray_3d,
        mesh_instance_surfaces_on_global_rays_3d, midi_play_attached, midi_release_attached,
        midi_start_attached, midi_stop_attached, navmesh_find_path_3d, node_collection,
        physics_apply_gravity_2d, physics_apply_gravity_3d, physics_get_body_gravity_scale,
        physics_get_coefficient, physics_get_gravity, physics_is_paused, physics_move_and_slide_2d,
        physics_move_and_slide_3d, physics_move_body_2d, physics_move_body_3d, physics_pause,
        physics_predict_body_2d, physics_predict_body_3d, physics_raycast_3d,
        physics_raycast_3d_with_areas, physics_raycast_3d_without_areas,
        physics_set_body_gravity_scale, physics_set_coefficient, physics_set_gravity,
        physics_solve_launch_velocity_2d, physics_solve_launch_velocity_3d,
        physics_solve_velocity_to_target_2d, physics_solve_velocity_to_target_3d, profiling, query,
        query_builder, query_each, query_expr, query_first, query_iter, query_map, remove_node,
        reparent, reparent_multi, scene_drop_preloaded, scene_free_preloaded, scene_load,
        scene_preload, script_attach, script_detach, script_set_fixed_update_enabled,
        script_set_update_enabled, set_global_pos_2d, set_global_pos_3d, set_global_rot_2d,
        set_global_rot_3d, set_global_scale_2d, set_global_scale_3d, set_global_transform_2d,
        set_global_transform_3d, set_local_pos_2d, set_local_pos_3d, set_local_rot_2d,
        set_local_rot_3d, set_local_scale_2d, set_local_scale_3d, set_local_transform_2d,
        set_local_transform_3d, set_node_name, set_tree_visible, set_ui_rotation, set_var,
        signal_connect, signal_connect_many, signal_connect_pairs, signal_disconnect,
        signal_disconnect_many, signal_emit, simulation_time, spawn, tag_add, tag_remove, tag_set,
        to_global_point_2d, to_global_point_3d, to_global_transform_2d, to_global_transform_3d,
        to_local_point_2d, to_local_point_3d, to_local_transform_2d, to_local_transform_3d,
        window_get_active_refresh_rate, window_set_cursor_icon, window_set_frame_rate_cap,
        window_set_frame_rate_limit, window_set_mode, window_set_size, window_set_title,
        with_base_node, with_base_node_mut, with_node, with_node_mut, with_state, with_state_mut,
    };

    // Common id and variant helpers.
    pub use perro_ids::prelude::{
        AnimationID, AudioBusID, LightID, MaterialID, MeshID, NavMeshID, NodeID, PreloadedSceneID,
        ScriptMemberID, SignalID, TagID, TextureID,
    };
    pub use perro_ids::{func, method, sid, signal, smid, tag, tags, var};
    pub use perro_nodes::prelude::*;
    pub use perro_variant::{VariantKind, params, variant};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
