use perro_api::prelude::*;

type SelfNodeType = Node3D;

#[State]
struct MeshMaterialsDemoState {
    #[default = NodeID::nil()]
    pub overlay: NodeID,
    #[default = NodeID::nil()]
    pub mirror_original: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_x: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_y: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_z: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_xy: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_xz: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_yz: NodeID,
    #[default = NodeID::nil()]
    pub mirror_flip_xyz: NodeID,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        self.install_mirror_mesh(ctx);
        self.push_overlay(ctx);
    }
});

methods!({
    fn set_info_overlay(&self, ctx: &mut ScriptContext<'_, API>, overlay: NodeID) {
        with_state_mut!(ctx.run, MeshMaterialsDemoState, ctx.id, |state| {
            state.overlay = overlay;
        });
        self.push_overlay(ctx);
    }

    fn push_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let overlay = with_state!(ctx.run, MeshMaterialsDemoState, ctx.id, |state| state
            .overlay).unwrap_or_default();
        if overlay.is_nil() {
            return;
        }
        let meshes = query!(ctx.run, all(node_type[MeshInstance3D]), in_subtree(ctx.id)).len();
        let lights = query!(ctx.run, all(node_type[AmbientLight3D]), in_subtree(ctx.id)).len()
            + query!(ctx.run, all(node_type[PointLight3D]), in_subtree(ctx.id)).len()
            + query!(ctx.run, all(node_type[SpotLight3D]), in_subtree(ctx.id)).len()
            + query!(ctx.run, all(node_type[RayLight3D]), in_subtree(ctx.id)).len();
        let body = format!(
            "mesh samples {}\nlight rigs {}\nmirror set same runtime mesh\nall flip_x/y/z combos",
            meshes, lights
        );
        let _ = call_method!(
            ctx.run,
            overlay,
            func!("set_content"),
            params!["Mesh + Materials".to_string(), body]
        );
    }

    fn install_mirror_mesh(&self, ctx: &mut ScriptContext<'_, API>) {
        let mesh = mesh_create!(ctx.res, mirror_sample_mesh());
        let nodes = with_state!(ctx.run, MeshMaterialsDemoState, ctx.id, |state| {
            [
                state.mirror_original,
                state.mirror_flip_x,
                state.mirror_flip_y,
                state.mirror_flip_z,
                state.mirror_flip_xy,
                state.mirror_flip_xz,
                state.mirror_flip_yz,
                state.mirror_flip_xyz,
            ]
        }).unwrap_or_default();
        for node in nodes {
            if node.is_nil() {
                continue;
            }
            with_node_mut!(ctx.run, MeshInstance3D, node, |mesh_node| {
                mesh_node.mesh = mesh;
            });
        }
    }
});

fn mirror_sample_mesh() -> Mesh3D {
    let mut mesh = Mesh3D {
        vertices: Vec::new(),
        indices: Vec::new(),
        surface_ranges: vec![MeshSurfaceRange {
            index_start: 0,
            index_count: 0,
        }],
        blend_shapes: Vec::new(),
    };

    add_box(&mut mesh, [-0.12, -0.95, -0.12], [0.12, 0.55, 0.12]);
    add_box(&mut mesh, [0.10, 0.15, -0.11], [0.95, 0.55, 0.11]);
    add_box(&mut mesh, [0.52, 0.45, -0.10], [0.88, 0.95, 0.10]);
    add_box(&mut mesh, [-0.10, -0.52, 0.10], [0.10, -0.22, 0.95]);
    add_box(&mut mesh, [-0.09, -0.46, 0.52], [0.09, -0.02, 0.84]);
    mesh.surface_ranges[0].index_count = mesh.indices.len() as u32;
    mesh
}

fn add_box(mesh: &mut Mesh3D, min: [f32; 3], max: [f32; 3]) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    add_quad(
        mesh,
        [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]],
        [1.0, 0.0, 0.0],
    );
    add_quad(
        mesh,
        [[x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [x0, y0, z0]],
        [-1.0, 0.0, 0.0],
    );
    add_quad(
        mesh,
        [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
        [0.0, 1.0, 0.0],
    );
    add_quad(
        mesh,
        [[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]],
        [0.0, -1.0, 0.0],
    );
    add_quad(
        mesh,
        [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
        [0.0, 0.0, 1.0],
    );
    add_quad(
        mesh,
        [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
        [0.0, 0.0, -1.0],
    );
}

fn add_quad(mesh: &mut Mesh3D, positions: [[f32; 3]; 4], normal: [f32; 3]) {
    let base = mesh.vertices.len() as u32;
    for position in positions {
        mesh.vertices.push(RuntimeMeshVertex {
            position,
            normal,
            uv: [0.0, 0.0],
            paint_uv: [0.0, 0.0],
            joints: [0, 0, 0, 0],
            weights: UnitVector4::ZERO,
        });
    }
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}
