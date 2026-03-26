pub mod api;
pub mod sub_apis;

pub use perro_ids;
pub use perro_nodes;
pub use perro_structs;
pub use perro_variant;

pub use api::RuntimeContext;

pub mod prelude {
    pub use crate::api::{RuntimeAPI, RuntimeContext};
    pub use crate::sub_apis::{
        AnimPlayerAPI, AnimPlayerModule, Attribute, IntoImpulseDirection, IntoNodeTags,
        IntoScenePath, IntoScriptMemberID, Member, NodeAPI, NodeModule, PhysicsAPI, PhysicsModule,
        QueryExpr, QueryScope, SceneAPI, SceneModule, ScriptAPI, ScriptModule, SignalAPI,
        SignalModule, TagQuery, TimeAPI, TimeModule,
    };
    pub use crate::{
        anim_player_bind, anim_player_clear_bindings, anim_player_pause, anim_player_play,
        anim_player_seek_frame, anim_player_set_clip, anim_player_set_speed, apply_force,
        apply_impulse, attribute, attributes_of, call_method, create_node, delta_time,
        delta_time_capped, delta_time_clamped, elapsed_time, fixed_delta_time, get_child,
        get_children, get_global_transform_2d, get_global_transform_3d, get_node_children_ids,
        get_node_name, get_node_parent_id, get_node_tags, get_node_type, get_var, has_attribute,
        member, members_with, query, query_first, remove_node, reparent, reparent_multi,
        scene_load, script_attach, script_detach, set_global_transform_2d, set_global_transform_3d,
        set_node_name, set_var, signal_connect, signal_disconnect, signal_emit, tag_add,
        tag_remove, tag_set, to_global_point_2d, to_global_point_3d, to_global_transform_2d,
        to_global_transform_3d, to_local_point_2d, to_local_point_3d, to_local_transform_2d,
        to_local_transform_3d, with_base_node, with_base_node_mut, with_node, with_node_mut,
        with_state, with_state_mut,
    };
    pub use perro_ids::{func, method, sid, signal, smid, tag, tags, var};
    pub use perro_variant::{params, variant};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
