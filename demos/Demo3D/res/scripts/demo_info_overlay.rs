use perro_api::prelude::*;

type SelfNodeType = UiPanel;

const TITLE_LABEL_NODE_NAME: &str = "info_overlay_title";
const BODY_LABEL_NODE_NAME: &str = "info_overlay_body";
const REFRESH_SECONDS: f32 = 0.35;

#[State]
struct DemoInfoOverlayState {
    #[default = NodeID::nil()]
    pub title_label: NodeID,
    #[default = NodeID::nil()]
    pub body_label: NodeID,
    #[default = String::new()]
    pub title_override: String,
    #[default = String::new()]
    pub body_override: String,
    #[default = 0.0]
    pub refresh_timer: f32,
    #[default = NodeID::nil()]
    pub last_root: NodeID,
    #[default = String::new()]
    pub last_demo: String,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let title = get_child!(ctx.run, ctx.id, TITLE_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        let body = get_child!(ctx.run, ctx.id, BODY_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
            state.title_label = title;
            state.body_label = body;
        });
        self.refresh(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run).max(0.0);
        let demo = get_var!(ctx.run, ctx.id, var!("active_demo"))
            .as_str()
            .unwrap_or("none")
            .to_string();
        let root = get_var!(ctx.run, ctx.id, var!("active_demo_root"))
            .as_node()
            .unwrap_or(NodeID::nil());
        let do_refresh = with_state_mut!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
            let changed = state.last_root != root || state.last_demo != demo;
            state.last_root = root;
            state.last_demo = demo.clone();
            state.refresh_timer += dt;
            if changed || state.refresh_timer >= REFRESH_SECONDS {
                state.refresh_timer = 0.0;
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if do_refresh {
            self.refresh(ctx);
        }
    }
});

methods!({
    fn refresh(&self, ctx: &mut ScriptContext<'_, API>) {
        let demo = get_var!(ctx.run, ctx.id, var!("active_demo"))
            .as_str()
            .unwrap_or("none")
            .to_string();
        let root = get_var!(ctx.run, ctx.id, var!("active_demo_root"))
            .as_node()
            .unwrap_or(NodeID::nil());
        let (title_label, body_label, title_override, body_override) =
            with_state!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
                (
                    state.title_label,
                    state.body_label,
                    state.title_override.clone(),
                    state.body_override.clone(),
                )
            });
        let title = if !title_override.is_empty() {
            title_override
        } else if demo == "none" {
            "Demo".to_string()
        } else {
            demo_title(&demo).to_string()
        };
        let body = if !body_override.is_empty() {
            body_override
        } else if root.is_nil() || demo == "none" {
            String::new()
        } else {
            demo_info_text(ctx, root, &demo)
        };
        set_label_text(ctx, title_label, title);
        set_label_text(ctx, body_label, body);
    }

    fn set_content(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        title: String,
        body: String,
    ) {
        with_state_mut!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
            state.title_override = title.clone();
            state.body_override = body.clone();
        });
        self.refresh(ctx);
    }

    fn clear_content(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, DemoInfoOverlayState, ctx.id, |state| {
            state.title_override.clear();
            state.body_override.clear();
        });
        self.refresh(ctx);
    }
});

fn demo_info_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    demo: &str,
) -> String {
    match demo {
        "multimesh" => multimesh_text(ctx, root),
        "water" => water_text(ctx, root),
        "lights" => lights_text(ctx, root),
        "animations" => animation_text(ctx, root),
        "particles" => particles_text(ctx, root),
        "physics_bones" => physics_bones_text(ctx, root),
        "physics_collisions" => physics_collisions_text(ctx, root),
        "positional_audio" => positional_audio_text(ctx, root),
        "mesh_blending" => mesh_text(ctx, root, "blend meshes"),
        "mesh_materials" => mesh_text(ctx, root, "mesh samples"),
        "sky" => mesh_text(ctx, root, "sky props"),
        _ => mesh_text(ctx, root, "scene meshes"),
    }
}

fn multimesh_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let multimeshes = query!(ctx.run, all(node_type[MultiMeshInstance3D]), in_subtree(root));
    let mut total_instances = 0usize;
    let mut per_mesh = Vec::new();
    for node in multimeshes.iter().copied() {
        let count = with_node!(ctx.run, MultiMeshInstance3D, node, |mesh| mesh.instances.len());
        total_instances += count;
        per_mesh.push(count.to_string());
    }
    format!(
        "multimeshes {} | total inst {}\ninst/mesh {}",
        multimeshes.len(),
        total_instances,
        if per_mesh.is_empty() {
            "0".into()
        } else {
            per_mesh.join(", ")
        }
    )
}

fn water_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let bodies = query!(ctx.run, all(node_type[WaterBody3D]), in_subtree(root));
    let rigid = query!(ctx.run, all(node_type[RigidBody3D]), in_subtree(root));
    let statics = query!(ctx.run, all(node_type[StaticBody3D]), in_subtree(root));
    let mut depths = Vec::new();
    for node in bodies.iter().copied() {
        let depth = with_node!(ctx.run, WaterBody3D, node, |water| water.water.depth);
        depths.push(format!("{depth:.1}"));
    }
    format!(
        "water bodies {} | float bodies {}\ncoast blocks {} | depth {}",
        bodies.len(),
        rigid.len(),
        statics.len(),
        if depths.is_empty() {
            "0".into()
        } else {
            depths.join(", ")
        }
    )
}

fn lights_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let points = query!(ctx.run, all(node_type[PointLight3D]), in_subtree(root)).len();
    let spots = query!(ctx.run, all(node_type[SpotLight3D]), in_subtree(root)).len();
    let rays = query!(ctx.run, all(node_type[RayLight3D]), in_subtree(root)).len();
    let ambient = query!(ctx.run, all(node_type[AmbientLight3D]), in_subtree(root)).len();
    format!("point {} | spot {} | ray {}\nambient {}", points, spots, rays, ambient)
}

fn animation_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let players = query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(root)).len();
    let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(root)).len();
    format!("anim players {} | meshes {}", players, meshes)
}

fn particles_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let emitters = query!(ctx.run, all(node_type[ParticleEmitter3D]), in_subtree(root)).len();
    let players = query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(root)).len();
    let points = query!(ctx.run, all(node_type[PointLight3D]), in_subtree(root)).len();
    format!("emitters {} | anim rigs {}\npoint lights {}", emitters, players, points)
}

fn physics_bones_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID) -> String {
    let players = query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(root)).len();
    let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(root)).len();
    let bones = query!(ctx.run, all(node_type[PhysicsBoneChain3D]), in_subtree(root)).len();
    format!("bone chains {} | anim players {}\nmesh props {}", bones, players, meshes)
}

fn physics_collisions_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) -> String {
    let rigid = query!(ctx.run, all(node_type[RigidBody3D]), in_subtree(root)).len();
    let statics = query!(ctx.run, all(node_type[StaticBody3D]), in_subtree(root)).len();
    let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(root)).len();
    format!("rigid bodies {} | static bodies {}\nmesh vis {}", rigid, statics, meshes)
}

fn positional_audio_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) -> String {
    let masks = query!(ctx.run, all(node_type[AudioMask3D]), in_subtree(root)).len();
    let zones = query!(ctx.run, all(node_type[AudioEffectZone3D]), in_subtree(root)).len();
    let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(root)).len();
    let speakers = meshes.saturating_sub(2);
    format!("audio masks {} | fx zones {}\nspeaker meshes {}", masks, zones, speakers)
}

fn mesh_text<API: ScriptAPI + ?Sized>(ctx: &mut ScriptContext<'_, API>, root: NodeID, label: &str) -> String {
    let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(root)).len();
    let multimeshes = query!(ctx.run, all(node_type[MultiMeshInstance3D]), in_subtree(root)).len();
    let lights = query!(ctx.run, all(node_type[PointLight3D]), in_subtree(root)).len()
        + query!(ctx.run, all(node_type[SpotLight3D]), in_subtree(root)).len()
        + query!(ctx.run, all(node_type[RayLight3D]), in_subtree(root)).len()
        + query!(ctx.run, all(node_type[AmbientLight3D]), in_subtree(root)).len();
    format!("{label} {meshes} | multimeshes {multimeshes}\nlights {lights}")
}

fn demo_title(demo: &str) -> &'static str {
    match demo {
        "mesh_materials" => "Mesh + Materials",
        "lights" => "Lights",
        "water" => "Water",
        "animations" => "Animations",
        "physics_bones" => "Physics Bones",
        "physics_collisions" => "Physics Collisions",
        "sky" => "Sky",
        "mesh_blending" => "Mesh Blending",
        "multimesh" => "MultiMesh",
        "particles" => "Particles",
        "positional_audio" => "Positional Audio",
        _ => "Demo",
    }
}

fn set_label_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    id: NodeID,
    text: String,
) {
    if id.is_nil() {
        return;
    }
    with_node_mut!(ctx.run, UiLabel, id, |label| {
        label.text = text.into();
    });
}
