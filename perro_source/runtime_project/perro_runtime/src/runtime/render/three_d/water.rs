use super::*;

impl Runtime {
    pub(crate) fn collect_water_coastline_shapes_3d(
        &mut self,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform3D>,
    ) -> Arc<[WaterCoastlineShape3D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_half = water.shape.surface_size() * 0.5;
        let water_top = water_global.position.y;
        let surface_band = water.coastline.foam_width.max(0.35) * 0.65;
        let surface_epsilon = surface_band.max(0.05) * 0.2;
        let mut shapes = Vec::new();
        // cached candidate ids (static/rigid/character bodies), gated on
        // physics_revision -> no per-tick full-arena scan. take out to iterate
        // while calling &mut self transform lookups, then restore.
        self.cached_water_collision_body_ids_3d();
        let body_ids = std::mem::take(&mut self.water_collision_body_ids_3d_cache);
        for body_id in body_ids.iter().copied() {
            let Some((enabled, layers, mask, scale_bias)) =
                self.nodes.get(body_id).and_then(|node| match &node.data {
                    SceneNodeData::StaticBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.02f32,
                    )),
                    SceneNodeData::RigidBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.00f32,
                    )),
                    SceneNodeData::CharacterBody3D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        1.00f32,
                    )),
                    _ => None,
                })
            else {
                continue;
            };
            if !enabled
                || water.collision_mask.intersects(layers)
                || mask.intersects(water.collision_layers)
            {
                continue;
            }
            let Some(_body_global) = self.get_render_global_transform_3d(body_id) else {
                continue;
            };
            // defer children clone until after enabled/mask filter passes.
            let Some(children) = self.nodes.children(body_id).map(<[NodeID]>::to_vec) else {
                continue;
            };
            for child_id in children {
                let Some((shape_kind, flip)) = self.nodes.get(child_id).and_then(|child| {
                    let SceneNodeData::CollisionShape3D(shape) = &child.data else {
                        return None;
                    };
                    Some((
                        shape.shape.clone(),
                        (shape.flip_x, shape.flip_y, shape.flip_z),
                    ))
                }) else {
                    continue;
                };
                let Some(shape_global) = self.get_render_global_transform_3d(child_id) else {
                    continue;
                };
                let local = shape_global.position - water_global.position;
                if local.x.abs() > water_half.x + 512.0 || local.z.abs() > water_half.y + 512.0 {
                    continue;
                }
                let scale = shape_global.scale;
                let mesh_scale = signed_collision_shape_scale(scale, flip);
                match &shape_kind {
                    Shape3D::Cube { size }
                    | Shape3D::TriPrism { size }
                    | Shape3D::TriangularPyramid { size }
                    | Shape3D::SquarePyramid { size } => {
                        let half = perro_structs::Vector3::new(
                            size.x.abs() * scale.x.abs() * 0.5,
                            size.y.abs() * scale.y.abs() * 0.5,
                            size.z.abs() * scale.z.abs() * 0.5,
                        );
                        let min_y = shape_global.position.y - half.y;
                        let max_y = shape_global.position.y + half.y;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Box {
                            center: [local.x, local.y, local.z],
                            half_extents: [half.x * scale_bias, half.y, half.z * scale_bias],
                            axis_x: water_local_axis_xz(
                                water_global,
                                shape_global,
                                perro_structs::Vector3::new(1.0, 0.0, 0.0),
                            ),
                            axis_z: water_local_axis_xz(
                                water_global,
                                shape_global,
                                perro_structs::Vector3::new(0.0, 0.0, 1.0),
                            ),
                        });
                    }
                    Shape3D::Sphere { radius } => {
                        let radius =
                            radius.abs() * scale.x.abs().max(scale.y.abs()).max(scale.z.abs());
                        let min_y = shape_global.position.y - radius;
                        let max_y = shape_global.position.y + radius;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Sphere {
                            center: [local.x, local.y, local.z],
                            radius: radius * scale_bias,
                        });
                    }
                    Shape3D::Capsule {
                        radius,
                        half_height,
                    }
                    | Shape3D::Cylinder {
                        radius,
                        half_height,
                    }
                    | Shape3D::Cone {
                        radius,
                        half_height,
                    } => {
                        let radius = radius.abs() * scale.x.abs().max(scale.z.abs());
                        let half_height = half_height.abs() * scale.y.abs();
                        let min_y = shape_global.position.y - half_height;
                        let max_y = shape_global.position.y + half_height;
                        let crosses_surface = min_y <= water_top + surface_epsilon
                            && max_y >= water_top - surface_epsilon;
                        if !crosses_surface
                            || max_y < water_top - surface_band
                            || min_y > water_top + surface_band
                        {
                            continue;
                        }
                        shapes.push(WaterCoastlineShape3D::Cylinder {
                            center: [local.x, local.y, local.z],
                            radius: radius * scale_bias,
                            half_height,
                        });
                    }
                    Shape3D::TriMesh { source } => {
                        let source_hash = parse_hashed_source_uri(source)
                            .unwrap_or_else(|| string_to_u64(source));
                        let Some(bytes) = self
                            .project()
                            .and_then(|project| project.static_collision_trimesh_lookup)
                            .map(|lookup| lookup(source_hash))
                            .filter(|bytes| !bytes.is_empty())
                        else {
                            continue;
                        };
                        let Some((vertices, triangles)) = perro_physics::decode_pmesh_trimesh(
                            bytes,
                            mesh_scale.x,
                            mesh_scale.y,
                            mesh_scale.z,
                        ) else {
                            continue;
                        };
                        for tri in triangles {
                            let Some(a) = vertices.get(tri[0] as usize) else {
                                continue;
                            };
                            let Some(b) = vertices.get(tri[1] as usize) else {
                                continue;
                            };
                            let Some(c) = vertices.get(tri[2] as usize) else {
                                continue;
                            };
                            let ay = shape_global.position.y + a.y;
                            let by = shape_global.position.y + b.y;
                            let cy = shape_global.position.y + c.y;
                            let min_y = ay.min(by).min(cy);
                            let max_y = ay.max(by).max(cy);
                            let crosses_surface = min_y <= water_top + surface_epsilon
                                && max_y >= water_top - surface_epsilon;
                            if !crosses_surface
                                || max_y < water_top - surface_band
                                || min_y > water_top + surface_band
                            {
                                continue;
                            }
                            let centroid_x = (a.x + b.x + c.x) / 3.0;
                            let centroid_z = (a.z + b.z + c.z) / 3.0;
                            let shrink = |x: f32, y: f32, z: f32| -> [f32; 3] {
                                [
                                    local.x + centroid_x + (x - centroid_x) * scale_bias,
                                    local.y + y,
                                    local.z + centroid_z + (z - centroid_z) * scale_bias,
                                ]
                            };
                            shapes.push(WaterCoastlineShape3D::Triangle {
                                points: [
                                    shrink(a.x, a.y, a.z),
                                    shrink(b.x, b.y, b.z),
                                    shrink(c.x, c.y, c.z),
                                ],
                            });
                        }
                    }
                }
            }
        }
        self.water_collision_body_ids_3d_cache = body_ids;
        Arc::from(shapes)
    }

    pub(crate) fn collect_water_queries_3d(
        &mut self,
        water_id: NodeID,
    ) -> Arc<[WaterBodyQueryState]> {
        let Some(queries) = self.pending_water_queries_3d.get(&water_id) else {
            return Arc::from([]);
        };
        Arc::from(
            queries
                .iter()
                .map(|query| WaterBodyQueryState {
                    water: water_id,
                    body: query.body,
                    point: query.point,
                    local: [query.local.x, query.local.y],
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }

    pub(crate) fn collect_water_impacts_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform3D>,
    ) -> Arc<[WaterImpact3D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_inv = water_global.to_mat4().inverse();
        let half = water.shape.surface_size() * 0.5;
        self.cached_rigid_body_ids_3d();
        let body_ids = std::mem::take(&mut self.water_rigid_body_ids_3d_cache);
        let mut impacts = Vec::new();
        for body_id in body_ids.iter().copied() {
            let Some((layers, mask, mass, density, velocity)) =
                self.nodes.get(body_id).and_then(|node| {
                    let SceneNodeData::RigidBody3D(body) = &node.data else {
                        return None;
                    };
                    Some((
                        body.collision_layers,
                        body.collision_mask,
                        body.mass,
                        body.density,
                        body.linear_velocity,
                    ))
                })
            else {
                continue;
            };
            if water.collision_mask.intersects(layers) || mask.intersects(water.collision_layers) {
                continue;
            }
            let Some(body_global) = self.get_render_global_transform_3d(body_id) else {
                continue;
            };
            let radius = mass.sqrt().max(1.0);
            let local = water_local_point_3d(water_inv, body_global.position);
            if !water
                .shape
                .contains_surface(perro_structs::Vector2::new(local.x, local.z))
                || local.y > radius
                || local.y < -water.shape.depth(water.depth)
            {
                continue;
            }
            let local_xz = perro_structs::Vector2::new(local.x, local.z);
            let cached_sample = crate::runtime::physics::lookup_water_body_sample(
                &self.water_body_samples,
                water_id,
                body_id,
                0,
                local_xz,
                self.time.elapsed,
            );
            let local = perro_structs::Vector3::new(local_xz.x, local.y, local_xz.y);
            let sample = crate::runtime::physics::water_physics_sample_for_body_cached(
                water,
                local_xz,
                self.time.elapsed,
                cached_sample,
                self.water_samples.get(&water_id).copied(),
            );
            let target = crate::runtime::physics::water_target_submerged(density);
            let submerged = sample.height - local.y;
            let rel_down = sample.velocity.y - velocity.y;
            // fast bodies cross the surface band in one tick; widen the window
            // by entry speed so high-velocity drops still register a splash
            let entry_window = (target * 2.25).max(rel_down.max(0.0) * (1.0 / 30.0) + target);
            if submerged <= 0.0 || submerged > entry_window || rel_down <= 1.1 {
                continue;
            }
            let velocity_2d = perro_structs::Vector2::new(velocity.x, velocity.z);
            let vertical_impact =
                (-velocity.y).max(0.0) * (1.0 - (local.y.abs() / radius).clamp(0.0, 1.0));
            let impact_velocity =
                perro_structs::Vector2::new(velocity_2d.length(), vertical_impact);
            let impact_strength =
                water_impact_strength(mass, impact_velocity, water.physics.wake_strength);
            let surface_contact = 1.0 - (local.y.abs() / radius).clamp(0.0, 1.0);
            let displacement_strength =
                mass.sqrt() * water.physics.wake_strength.max(0.0) * surface_contact * 0.42;
            let strength = impact_strength.max(displacement_strength.min(18.0));
            if strength <= 0.0 {
                continue;
            }
            impacts.push(WaterImpact3D {
                position: [local.x, local.y, local.z],
                velocity: [velocity.x, velocity.y, velocity.z],
                strength: strength * 1.18,
                radius: radius * 0.5,
                cavitation: (vertical_impact * 0.035 + surface_contact * 0.08).clamp(0.0, 1.0),
            });
        }
        self.water_rigid_body_ids_3d_cache = body_ids;
        for impact in self.force_water_impacts_3d.iter() {
            let local = water_local_point_3d(water_inv, impact.position);
            if local.x.abs() > half.x + impact.radius
                || local.z.abs() > half.y + impact.radius
                || local.y > impact.radius
                || local.y < -water.shape.depth(water.depth) - impact.radius
            {
                continue;
            }
            impacts.push(WaterImpact3D {
                position: [local.x, local.y, local.z],
                velocity: [impact.force.x, impact.force.y, impact.force.z],
                strength: impact.strength,
                radius: impact.radius,
                cavitation: impact.cavitation,
            });
        }
        if let Some(contacts) = self.water_contacts_3d.get(&water_id) {
            for contact in contacts {
                let local = water_local_point_3d(water_inv, contact.position);
                if local.x.abs() > half.x + contact.radius
                    || local.z.abs() > half.y + contact.radius
                    || local.y > contact.radius
                    || local.y < -water.shape.depth(water.depth) - contact.radius
                {
                    continue;
                }
                impacts.push(WaterImpact3D {
                    position: [local.x, local.y, local.z],
                    velocity: [contact.velocity.x, contact.velocity.y, contact.velocity.z],
                    strength: (contact.foam_amount * 5.8).max(0.22),
                    radius: contact.radius,
                    cavitation: (contact.foam_amount * 0.30).min(1.0),
                });
            }
        }
        for link in self.collect_water_links_3d(water_id, water).iter() {
            for impact in self.force_water_impacts_3d.iter() {
                let local = water_local_point_3d(water_inv, impact.position);
                if water
                    .shape
                    .contains_surface(perro_structs::Vector2::new(local.x, local.z))
                {
                    continue;
                }
                let pad = link.blend_width + impact.radius;
                if local.x < link.overlap_min[0] - pad
                    || local.x > link.overlap_max[0] + pad
                    || local.z < link.overlap_min[1] - pad
                    || local.z > link.overlap_max[1] + pad
                {
                    continue;
                }
                let weight =
                    water_link_overlap_weight(perro_structs::Vector2::new(local.x, local.z), link);
                if weight <= 0.0 {
                    continue;
                }
                impacts.push(WaterImpact3D {
                    position: [local.x, local.y, local.z],
                    velocity: [impact.force.x, impact.force.y, impact.force.z],
                    strength: impact.strength * link.wave_transfer * weight,
                    radius: impact.radius,
                    cavitation: impact.cavitation * weight,
                });
            }
        }
        Arc::from(impacts)
    }

    pub(crate) fn collect_water_links_3d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterLinkState]> {
        let Some(water_global) = self.get_render_global_transform_3d(water_id) else {
            return Arc::from([]);
        };
        self.cached_water_ids_3d();
        let other_ids = std::mem::take(&mut self.water_ids_3d_cache);
        let mut links = Vec::new();
        for other_id in other_ids.iter().copied() {
            if other_id == water_id {
                continue;
            }
            let Some(other_water) = self.nodes.get(other_id).and_then(|node| {
                let SceneNodeData::WaterBody3D(other) = &node.data else {
                    return None;
                };
                Some(other.water)
            }) else {
                continue;
            };
            let Some(other_global) = self.get_render_global_transform_3d(other_id) else {
                continue;
            };
            if water
                .link
                .link_mask
                .intersects(other_water.link.link_layers)
                || other_water
                    .link
                    .link_mask
                    .intersects(water.link.link_layers)
            {
                continue;
            }
            let Some((overlap_min, overlap_max)) =
                water_overlap_bounds_3d(water, water_global, other_water, other_global)
            else {
                continue;
            };
            let extent = (overlap_max.x - overlap_min.x).min(overlap_max.y - overlap_min.y);
            let blend_width = if water.link.blend_width > 0.0 {
                water.link.blend_width
            } else {
                (extent * 0.5).max(0.5)
            };
            links.push(WaterLinkState {
                other: other_id,
                overlap_min: [overlap_min.x, overlap_min.y],
                overlap_max: [overlap_max.x, overlap_max.y],
                blend_width,
                wave_transfer: water.link.wave_transfer.min(other_water.link.wave_transfer),
                flow_transfer: water.link.flow_transfer.min(other_water.link.flow_transfer),
            });
        }
        self.water_ids_3d_cache = other_ids;
        Arc::from(links)
    }
}
