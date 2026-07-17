use super::*;

impl Runtime {
    pub(crate) fn collect_water_coastline_shapes_2d(
        &mut self,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform2D>,
    ) -> Arc<[WaterCoastlineShape2D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_half = water.shape.surface_size() * 0.5;
        let mut shapes = Vec::new();
        // cached candidate ids (static/rigid/character bodies), gated on
        // physics_revision -> no per-tick full-arena scan. take out to iterate
        // while calling &mut self transform lookups, then restore.
        self.cached_water_collision_body_ids_2d();
        let body_ids = std::mem::take(&mut self.water_collision_body_ids_2d_cache);
        for body_id in body_ids.iter().copied() {
            let Some((enabled, layers, mask, scale_bias)) =
                self.nodes.get(body_id).and_then(|node| match &node.data {
                    SceneNodeData::StaticBody2D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        0.85f32,
                    )),
                    SceneNodeData::RigidBody2D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        0.50f32,
                    )),
                    SceneNodeData::CharacterBody2D(body) => Some((
                        body.enabled,
                        body.collision_layers,
                        body.collision_mask,
                        0.50f32,
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
            let Some(_body_global) = self.get_render_global_transform_2d(body_id) else {
                continue;
            };
            // defer children clone until after enabled/mask filter passes.
            let Some(children) = self
                .nodes
                .get(body_id)
                .map(|node| node.children_slice().to_vec())
            else {
                continue;
            };
            for child_id in children {
                let Some(shape_kind) = self.nodes.get(child_id).and_then(|child| {
                    let SceneNodeData::CollisionShape2D(shape) = &child.data else {
                        return None;
                    };
                    Some(shape.shape)
                }) else {
                    continue;
                };
                let Some(shape_global) = self.get_render_global_transform_2d(child_id) else {
                    continue;
                };
                let local = shape_global.position - water_global.position;
                if local.x.abs() > water_half.x + 512.0 || local.y.abs() > water_half.y + 512.0 {
                    continue;
                }
                match shape_kind {
                    Shape2D::Quad { width, height } => {
                        shapes.push(WaterCoastlineShape2D::Quad {
                            center: [local.x, local.y],
                            half_extents: [
                                width.abs() * shape_global.scale.x.abs() * 0.5 * scale_bias,
                                height.abs() * shape_global.scale.y.abs() * 0.5 * scale_bias,
                            ],
                            rotation: shape_global.rotation - water_global.rotation,
                        });
                    }
                    Shape2D::Circle { radius } => {
                        shapes.push(WaterCoastlineShape2D::Circle {
                            center: [local.x, local.y],
                            radius: radius.abs()
                                * shape_global.scale.x.abs().max(shape_global.scale.y.abs())
                                * scale_bias,
                        });
                    }
                    Shape2D::Triangle { width, height, .. } => {
                        let hw = width.abs() * shape_global.scale.x.abs() * 0.5;
                        let hh = height.abs() * shape_global.scale.y.abs() * 0.5;
                        let center = [local.x, local.y];
                        let points = [
                            [local.x, local.y + hh],
                            [local.x - hw, local.y - hh],
                            [local.x + hw, local.y - hh],
                        ];
                        shapes.push(WaterCoastlineShape2D::Triangle {
                            points: points.map(|point| {
                                [
                                    center[0] + (point[0] - center[0]) * scale_bias,
                                    center[1] + (point[1] - center[1]) * scale_bias,
                                ]
                            }),
                        });
                    }
                }
            }
        }
        self.water_collision_body_ids_2d_cache = body_ids;
        Arc::from(shapes)
    }

    pub(crate) fn collect_water_queries_2d(
        &mut self,
        water_id: NodeID,
    ) -> Arc<[WaterBodyQueryState]> {
        let Some(queries) = self.pending_water_queries_2d.get(&water_id) else {
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

    pub(crate) fn collect_water_impacts_2d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
        water_global: Option<perro_structs::Transform2D>,
    ) -> Arc<[WaterImpact2D]> {
        let Some(water_global) = water_global else {
            return Arc::from([]);
        };
        let water_inv = water_global.to_mat3().inverse();
        let half = water.shape.surface_size() * 0.5;
        self.cached_rigid_body_ids_2d();
        let body_ids = std::mem::take(&mut self.water_rigid_body_ids_2d_cache);
        let mut impacts = Vec::new();
        for body_id in body_ids.iter().copied() {
            let Some((layers, mask, mass, density, velocity)) =
                self.nodes.get(body_id).and_then(|node| {
                    let SceneNodeData::RigidBody2D(body) = &node.data else {
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
            let Some(body_global) = self.get_render_global_transform_2d(body_id) else {
                continue;
            };
            let local = water_local_point_2d(water_inv, body_global.position);
            if !water.shape.contains_surface(local) {
                continue;
            }
            let cached_sample = crate::runtime::physics::lookup_water_body_sample(
                &self.water_body_samples,
                water_id,
                body_id,
                0,
                local,
                self.time.elapsed,
            );
            let sample = crate::runtime::physics::water_physics_sample_for_body_cached(
                water,
                local,
                self.time.elapsed,
                cached_sample,
                self.water_samples.get(&water_id).copied(),
            );
            let target = crate::runtime::physics::water_target_submerged(density);
            let submerged = sample.height - local.y;
            let rel_down = sample.velocity.y - velocity.y;
            if submerged <= 0.0 || submerged > target * 2.25 || rel_down <= 0.35 {
                continue;
            }
            let strength =
                water_impact_strength(mass.max(density), velocity, water.physics.wake_strength)
                    .max(rel_down * mass.max(density) * water.physics.wake_strength);
            if strength <= 0.0 {
                continue;
            }
            impacts.push(WaterImpact2D {
                position: [local.x, local.y],
                velocity: [velocity.x, velocity.y],
                strength: strength * 1.15,
                radius: mass.max(density).sqrt().max(1.0) * 0.5,
                cavitation: 0.0,
            });
        }
        self.water_rigid_body_ids_2d_cache = body_ids;
        for impact in self.force_water_impacts_2d.iter() {
            let local = water_local_point_2d(water_inv, impact.position);
            if local.x.abs() > half.x + impact.radius || local.y.abs() > half.y + impact.radius {
                continue;
            }
            impacts.push(WaterImpact2D {
                position: [local.x, local.y],
                velocity: [impact.force.x, impact.force.y],
                strength: impact.strength,
                radius: impact.radius,
                cavitation: impact.cavitation,
            });
        }
        if let Some(contacts) = self.water_contacts_2d.get(&water_id) {
            for contact in contacts {
                let local = water_local_point_2d(water_inv, contact.position);
                if local.x.abs() > half.x + contact.radius
                    || local.y.abs() > half.y + contact.radius
                {
                    continue;
                }
                impacts.push(WaterImpact2D {
                    position: [local.x, local.y],
                    velocity: [contact.velocity.x, contact.velocity.y],
                    strength: (contact.foam_amount * 5.4).max(0.35),
                    radius: contact.radius,
                    cavitation: contact.foam_amount * 0.2,
                });
            }
        }
        for link in self.collect_water_links_2d(water_id, water).iter() {
            for impact in self.force_water_impacts_2d.iter() {
                let local = water_local_point_2d(water_inv, impact.position);
                if water.shape.contains_surface(local) {
                    continue;
                }
                let pad = link.blend_width + impact.radius;
                if local.x < link.overlap_min[0] - pad
                    || local.x > link.overlap_max[0] + pad
                    || local.y < link.overlap_min[1] - pad
                    || local.y > link.overlap_max[1] + pad
                {
                    continue;
                }
                let weight = water_link_overlap_weight(local, link);
                if weight <= 0.0 {
                    continue;
                }
                impacts.push(WaterImpact2D {
                    position: [local.x, local.y],
                    velocity: [impact.force.x, impact.force.y],
                    strength: impact.strength * link.wave_transfer * weight,
                    radius: impact.radius,
                    cavitation: impact.cavitation * weight,
                });
            }
        }
        Arc::from(impacts)
    }

    pub(crate) fn collect_water_links_2d(
        &mut self,
        water_id: NodeID,
        water: &perro_nodes::WaterSurfaceParams,
    ) -> Arc<[WaterLinkState]> {
        let Some(water_global) = self.get_render_global_transform_2d(water_id) else {
            return Arc::from([]);
        };
        self.cached_water_ids_2d();
        let other_ids = std::mem::take(&mut self.water_ids_2d_cache);
        let mut links = Vec::new();
        for other_id in other_ids.iter().copied() {
            if other_id == water_id {
                continue;
            }
            let Some(other_water) = self.nodes.get(other_id).and_then(|node| {
                let SceneNodeData::WaterBody2D(other) = &node.data else {
                    return None;
                };
                Some(other.water)
            }) else {
                continue;
            };
            let Some(other_global) = self.get_render_global_transform_2d(other_id) else {
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
                water_overlap_bounds_2d(water, water_global, other_water, other_global)
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
        self.water_ids_2d_cache = other_ids;
        Arc::from(links)
    }
}
