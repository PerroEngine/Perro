pub mod api;
pub mod sub_apis;

pub use perro_ids;
pub use perro_nodes;
pub use perro_variant;

pub use api::RuntimeContext;

pub mod prelude {
    pub use crate::api::{RuntimeAPI, RuntimeContext};
    pub use crate::sub_apis::{
        Attribute, IntoNodeTags, IntoScriptMemberID, Member, NodeAPI, NodeModule, QueryExpr,
        QueryScope, ScriptAPI, ScriptModule, SignalAPI, SignalModule, TagQuery, TimeAPI,
        TimeModule,
    };
    pub use crate::{
        attribute, attributes_of, call_method, create_node, delta_time, elapsed_time,
        fixed_delta_time, get_node_children_ids, get_node_name, get_node_parent_id, get_node_tags,
        get_node_type, get_var, has_attribute, member, members_with, query, query_first,
        remove_node, reparent, reparent_multi, script_attach, script_detach, set_node_name,
        set_var, signal_connect, signal_disconnect, signal_emit, tag_add, tag_remove, tag_set,
        with_base_node, with_base_node_mut, with_node, with_node_mut, with_state, with_state_mut,
    };
    pub use perro_ids::{func, method, sid, signal, smid, tag, tags, var};
    pub use perro_variant::{params, variant};
}

#[cfg(test)]
#[path = "../tests/unit/lib_tests.rs"]
mod tests;
