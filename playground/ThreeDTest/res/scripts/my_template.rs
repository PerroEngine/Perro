use perro_context::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

// Script is authored against a node type. This default template uses Node2D.
type SelfNodeType = Node2D;

// State is data-only. Keeping state separate from behavior makes cross-calls memory safe
// and helps the runtime handle recursion/re-entrancy without borrowing issues.

// Define state struct with #[State] and use #[default = _] for default values on initialization.
#[State]
pub struct ExampleState {
    #[default = 5]
    count: i32,
}

const SPEED: f32 = 5.0;

lifecycle!({
    // Lifecycle methods are engine entry points. They are called by the runtime.
    // `ctx` is the main interface into the engine core to access runtime data/scripts and nodes.
    // `self_id` is the NodeID handle of the node this script is attached to.

    // init is called when the script instance is created. This can be used for one-time setup. State is initialized
    fn on_init(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        // with_state! gives read-only state access and returns data from the closure.
        // with_state_mut! gives mutable state access; it can mutate and optionally return data.
        let count = with_state!(ctx, ExampleState, self_id, |state| {
            state.count
        }).unwrap_or_default();
        log_info!(count);
    }

    // on_all_init is called after all scripts have had on_init called. This can be used for setup that requires other scripts to be initialized.
    fn on_all_init(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    // on_update is called every frame. This is where most behavior logic goes.
    fn on_update(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        let dt = delta_time!(ctx);
        // Regular Rust method calls are for internal methods.
        self.bump_count(ctx, self_id);

        // with_node! gives read-only typed node access and returns data from the closure.
        // with_node_mut! gives mutable typed node access; it can mutate and optionally return data.
        // Here we mutate the attached node via `self_id`.
        with_node_mut!(ctx, SelfNodeType, self_id, |node| {
            node.position.x += dt * SPEED;
        });

        // You can also pass another NodeID with another node type if that id maps
        // to that type at runtime.
        // Example:
        // with_node_mut!(ctx, MeshInstance3D, enemy_id, |mesh| { mesh.scale.x += 1.0; });
        //
        // For common hierarchy/identity operations, prefer dedicated helper macros:
        // let name = get_node_name!(ctx, self_id).unwrap_or_default();
        // let parent = get_node_parent_id!(ctx, self_id).unwrap_or(NodeID::nil());
        // let children = get_node_children_ids!(ctx, self_id).unwrap_or_default();
        // let _renamed = set_node_name!(ctx, self_id, "Player");
        // let _ok = reparent!(ctx, NodeID::new(10), self_id);
        // let _moved = reparent_multi!(ctx, NodeID::new(10), [NodeID::new(11), NodeID::new(12)]);
        //
        // Script attachment helpers:
        // let _attached = attach_script!(ctx, self_id, "res://scripts/other.rs");
        // let _detached = detach_script!(ctx, self_id);
        // `attach_script!` takes a target node id + script path.
        // `detach_script!` takes a node/script id and removes the attached script instance.
        //
        // call_method! can invoke methods through the script interface by member id.
        // Here we call our own script through self_id for demonstration.
        call_method!(ctx, self_id, smid!("test"), params![7123_i32, "bodsasb"]);
        set_var!(ctx, self_id, smid!("count"), 77_i32.into());
        let remote_count = get_var!(ctx, self_id, smid!("count"));
        log_info!(remote_count);
        // For local/internal behavior and local state, prefer direct methods plus
        // with_state!/with_state_mut! (for example self.bump_count(...)).
        // Read-only helpers (`with_state!`, `with_node!`) are for non-mutable access.
        // Mutable helpers (`with_state_mut!`, `with_node_mut!`) can mutate and
        // can return a value if you need one; ignoring the return is also fine.
        // That is simpler and more performant than call_method!/get_var!/set_var!.

        // Typical NodeID lookup is runtime-dependent. NodeID is a handle, not the node value.
        // if let Some(enemy_id) = find_node!(ctx, "enemy") {
        //     // Cross-script call on another script instance:
        //     call_method!(ctx, enemy_id, smid!("test"), params![1_i32, "ping"]);
        //
        //     // Mutate enemy node directly if you know its runtime node type:
        //     with_node_mut!(ctx, MeshInstance3D, enemy_id, |enemy| {
        //         enemy.scale.x += 0.1;
        //     });
        //
        //     // If type is uncertain, check metadata/type first, then branch/match.
        // }
    }

    // on_fixed_update is called on a fixed timestep, independent of frame rate. This is useful for physics and other deterministic updates.
    fn on_fixed_update(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}

    // on_removal is called when the script instance is removed from a node or the node is removed from the scene. This can be used for cleanup.
    fn on_removal(&self, _ctx: &mut RuntimeContext<'_, R>, _self_id: NodeID) {}
});

methods!({
    // methods! defines callable behavior methods (local or cross-script via call_method!)...
    fn bump_count(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID) {
        //  Use `with_state_mut!` for mutable access to state
        with_state_mut!(ctx, ExampleState, self_id, |state| {
            state.count += 1;
        });
    }

    fn test(&self, ctx: &mut RuntimeContext<'_, R>, self_id: NodeID, param1: i32, msg: &str) {
        log_info!(param1);
        log_info!(msg);
        self.bump_count(ctx, self_id);
    }
});
