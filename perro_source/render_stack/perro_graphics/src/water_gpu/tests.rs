use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_water_2d() -> Water2DState {
        Water2DState {
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            z_index: 0,
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            render_resolution: [16, 16],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.35,
            foam_strength: 0.9,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: perro_structs::Color::new(0.02, 0.16, 0.28, 0.94),
            shallow_color: perro_structs::Color::new(0.08, 0.46, 0.62, 0.74),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: perro_structs::Color::new(0.86, 0.96, 1.0, 1.0),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: perro_structs::Color::new(0.9, 0.97, 1.0, 1.0),
            coastline_foam_strength: 0.75,
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([perro_render_bridge::WaterLinkState {
                other: NodeID::from_parts(99, 0),
                overlap_min: [-1.0, -1.0],
                overlap_max: [1.0, 1.0],
                blend_width: 1.0,
                wave_transfer: 1.0,
                flow_transfer: 1.0,
            }]),
            queries: Arc::from([]),
            impacts: Arc::from([perro_render_bridge::WaterImpact2D {
                position: [0.0, 0.0],
                velocity: [1.0, 0.0],
                strength: 2.0,
                radius: 2.0,
                cavitation: 0.5,
            }]),
            coastline_shapes: Arc::from([]),
        }
    }

    fn test_water_3d() -> Water3DState {
        Water3DState {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            render_resolution: [16, 16],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.35,
            foam_strength: 0.9,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: perro_structs::Color::new(0.02, 0.16, 0.28, 0.94),
            shallow_color: perro_structs::Color::new(0.08, 0.46, 0.62, 0.74),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: perro_structs::Color::new(0.86, 0.96, 1.0, 1.0),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: perro_structs::Color::new(0.9, 0.97, 1.0, 1.0),
            coastline_foam_strength: 0.75,
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
            queries: Arc::from([]),
            impacts: Arc::from([perro_render_bridge::WaterImpact3D {
                position: [0.0, 0.0, 0.0],
                velocity: [1.0, 0.0, 0.0],
                strength: 2.0,
                radius: 2.0,
                cavitation: 0.5,
            }]),
            coastline_shapes: Arc::from([]),
        }
    }

    #[test]
    fn water_wgsl_parses() {
        naga::front::wgsl::parse_str(WATER_WGSL).expect("water wgsl should parse");
        let render_wgsl = water_render_wgsl();
        naga::front::wgsl::parse_str(&render_wgsl).expect("water render wgsl should parse");
        naga::front::wgsl::parse_str(WATER_3D_RENDER_WGSL)
            .expect("water 3d render wgsl should parse");
        assert!(!WATER_3D_RENDER_WGSL.contains("water_screen_contact_outline"));
        assert!(!WATER_3D_RENDER_WGSL.contains("outline_white"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_idle_height"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_depth_thickness"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_ssr"));
        assert!(WATER_3D_RENDER_WGSL.contains("scene_color_tex"));
        assert!(WATER_3D_RENDER_WGSL.contains("transmitted_rgb"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_transmission_tap"));
        assert!(WATER_3D_RENDER_WGSL.contains("depth_weight"));
        assert!(WATER_3D_RENDER_WGSL.contains("in_scatter"));
        assert!(WATER_3D_RENDER_WGSL.contains("transmission_luma"));
        assert!(WATER_3D_RENDER_WGSL.contains("optical_opacity"));
        assert!(WATER_3D_RENDER_WGSL.contains("let depth_reflection = smoothstep"));
        assert!(WATER_3D_RENDER_WGSL.contains("let reflection_weight = mix(fresnel"));
        assert!(!WATER_3D_RENDER_WGSL.contains("let fresnel_tint"));
        assert!(WATER_3D_RENDER_WGSL.contains("foam_mask"));
        assert!(WATER_3D_RENDER_WGSL.contains("caustic"));
        // render + compute + CPU idle wave models must stay in lockstep
        assert!(WATER_WGSL.contains("water_crest_wave(a) * 0.42"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_crest_wave(a) * 0.42"));
        assert!(WATER_WGSL.contains("swell_a * 0.82 + swell_b * 0.56"));
        assert!(WATER_3D_RENDER_WGSL.contains("swell_a * 0.82 + swell_b * 0.56"));
        // waves must run on the runtime sim clock (wave_profile.y), not the
        // render backend frame clock, or physics phase drifts from visuals
        assert!(WATER_WGSL.contains("let t = w.wave_profile.y;"));
        assert!(WATER_3D_RENDER_WGSL.contains("let t = w.wave_profile.y;"));
        assert!(!WATER_3D_RENDER_WGSL.contains("params.time_seconds"));
        assert!(!WATER_3D_RENDER_WGSL.contains("water_surface_contact_foam"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_x.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_y.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_z.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.sim.z, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.flags.x, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_surface_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_side_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("horizontal_segments * 2u + vertical_segments"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(0u, 0u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(1u, 1u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(1u, 0u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(0u, 1u),"));
    }

    #[test]
    fn rect_water_3d_side_vertices_follow_grid_edges() {
        let mut water = water_gpu_3d(
            NodeID::from_parts(1, 0),
            &test_water_3d(),
            WaterGridResolution {
                sim: [8, 6],
                render: [8, 6],
            },
            0,
            water_cell_count([8, 6]) as u32,
            1.0,
            [0.0, 0.0, 0.0],
        );
        water.shape = [0.0, 16.0, 16.0, 4.0];

        let surface = (8 - 1) * (6 - 1) * 6;
        let side = ((8 - 1) + (6 - 1)) * 2 * 6;
        assert_eq!(water_3d_vertex_count(&water), surface + side);
    }

    #[test]
    fn rotated_box_coastline_distance_uses_shape_axes() {
        let shape = WaterCoastlineShape3D::Box {
            center: [0.0, 0.0, 0.0],
            half_extents: [4.0, 1.0, 1.0],
            axis_x: [0.0, 1.0],
            axis_z: [-1.0, 0.0],
        };

        assert!(signed_distance_3d_xz([0.0, 3.5], shape) < 0.0);
        assert!(signed_distance_3d_xz([3.5, 0.0], shape) > 0.0);
    }

    #[test]
    fn coastline_fill_keeps_foam_inside_one_meter_before_cutoff() {
        let (edge_solid, edge_foam, edge_energy) = coastline_fill(-0.25, 1.5, 0.25);
        assert!(edge_solid < 0.01);
        assert!(edge_foam > 0.8);
        assert!(edge_energy > 0.7);

        let (deep_solid, deep_foam, deep_energy) = coastline_fill(-1.5, 1.5, 0.25);
        assert!(deep_solid > 0.9);
        assert!(deep_foam <= 0.01);
        assert!(deep_energy < 0.1);
    }

    #[test]
    fn water_lod_resolution_clamps_with_distance() {
        assert_eq!(
            water_lod(
                [256, 256],
                [512, 512],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [0.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                grid: WaterGridResolution {
                    sim: [256, 256],
                    render: [512, 512],
                },
                ripple_blend: 1.0,
            }
        );
        let mid = water_lod(
            [256, 256],
            [512, 512],
            [64.0, 64.0],
            [128.0, 384.0, 896.0],
            [32, 32],
            [512.0, 0.0],
            [0.0, 0.0],
        );
        assert_eq!(mid.grid.sim, [91, 91]);
        assert_eq!(mid.grid.render, [201, 201]);
        assert!(mid.ripple_blend > 0.75 && mid.ripple_blend < 0.85);
        let high = water_lod(
            [4096, 4096],
            [4096, 4096],
            [64.0, 64.0],
            [128.0, 384.0, 896.0],
            [32, 32],
            [0.0, 0.0],
            [0.0, 0.0],
        );
        assert_eq!(high.grid.sim, [256, 256]);
        assert_eq!(high.grid.render, [1024, 1024]);
        assert_eq!(
            water_lod(
                [256, 256],
                [512, 512],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [2048.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                grid: WaterGridResolution {
                    sim: [0, 0],
                    render: [0, 0],
                },
                ripple_blend: 0.0,
            }
        );
        assert_eq!(water_cell_count([0, 0]), 0);
        assert_eq!(water_cell_count([1, 1]), 1);
    }

    #[test]
    fn water_lod_3d_keeps_simulation_active_while_render_lods() {
        let mut water = test_water_3d();
        water.resolution = [4096, 2048];
        water.render_resolution = [256, 256];
        let near = water_lod_3d(&water, [0.0, 2.0, 0.0]);
        let mid = water_lod_3d(&water, [260.0, 2.0, 0.0]);
        let culled = water_lod_3d(&water, [100_000.0, 2.0, 100_000.0]);

        assert_eq!(near.grid.sim, [256, 256]);
        assert_eq!(mid.grid.sim, near.grid.sim);
        assert_eq!(culled.grid.sim, near.grid.sim);
        assert!(mid.grid.render[0] < near.grid.render[0]);
        assert_eq!(culled.grid.render, [0, 0]);
        assert!(mid.ripple_blend < near.ripple_blend);
        assert_eq!(culled.ripple_blend, 0.0);
    }

    #[test]
    fn water_lod_3d_keeps_far_surface_dense_enough_for_smooth_specular() {
        let lod = water_lod_from_distance(
            [256, 256],
            [256, 128],
            [100.0, 200.0, 400.0],
            [16, 16],
            400.0,
            WATER_3D_MAX_RENDER_RESOLUTION,
            WATER_3D_RENDER_LOD_STRENGTH,
        );

        assert_eq!(lod.grid.render, [146, 73]);
        assert!(lod.grid.render[0] >= 256 / 2);
        assert!(lod.grid.render[1] >= 128 / 2);
    }

    #[test]
    fn water_readback_interval_uses_rate() {
        assert_eq!(readback_interval_seconds(0.0), 0.0);
        assert!((readback_interval_seconds(60.0) - (1.0 / 60.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(30.0) - (1.0 / 30.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(15.0) - (1.0 / 15.0)).abs() < 1.0e-6);
    }

    #[test]
    fn water_query_offsets_sample_four_cells_for_bilinear_height() {
        let water = water_gpu_3d(
            NodeID::from_parts(1, 0),
            &test_water_3d(),
            WaterGridResolution {
                sim: [4, 4],
                render: [4, 4],
            },
            10,
            16,
            1.0,
            [0.0, 0.0, 0.0],
        );
        let sample = water_query_sample_offsets(&water, [0.0, 0.0]);
        assert_eq!(sample.offsets, [15, 16, 19, 20]);
        assert_eq!(sample.frac, [0.5, 0.5]);
        let cell = water_lerp_cell(
            [0.0, 0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0, 0.0],
            [6.0, 0.0, 0.0, 0.0],
            sample.frac,
        );
        assert_eq!(cell[0], 3.0);
    }

    #[test]
    fn water_gpu_2d_staging_accepts_linked_water_state() {
        let water = test_water_2d();
        let staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &water,
            WaterGridResolution {
                sim: water.resolution,
                render: water.resolution,
            },
            4,
            64,
            1.0,
        );
        assert_eq!(staged.node, 7);
        assert_eq!(staged.sim, [4, 64, 8, 8]);
        assert_eq!(staged.kind, 2);
        assert_eq!(staged.flags[2] & WATER_FLAG_PAUSED, 0);
        let mut paused = water;
        paused.paused = true;
        let paused_staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &paused,
            WaterGridResolution {
                sim: paused.resolution,
                render: paused.resolution,
            },
            4,
            64,
            1.0,
        );
        assert_ne!(paused_staged.flags[2] & WATER_FLAG_PAUSED, 0);
    }

    #[test]
    fn water_gpu_raster_impacts_2d_and_3d_write_signed_wake_cells() {
        // wake is signed: crater (negative) under the impact, spill energy positive
        let water_2d = test_water_2d();
        let mut cells_2d = vec![[0.0; 4]; 64];
        raster_impacts_2d(&mut cells_2d, 8, 8, &water_2d);
        assert!(cells_2d.iter().any(|cell| cell[2] != 0.0 && cell[3] > 0.0));
        assert!(cells_2d.iter().any(|cell| cell[2] < 0.0));

        let water_3d = test_water_3d();
        let mut cells_3d = vec![[0.0; 4]; 64];
        raster_impacts_3d(&mut cells_3d, 8, 8, &water_3d);
        assert!(cells_3d.iter().any(|cell| cell[2] != 0.0 && cell[3] > 0.0));
        assert!(cells_3d.iter().any(|cell| cell[2] < 0.0));
    }

    #[test]
    fn water_readback_decode_uses_submitted_metadata() {
        let submitted_water = NodeID::from_parts(10, 1);
        let submitted_body = NodeID::from_parts(20, 2);
        let query = WaterReadbackQuery {
            query: WaterBodyQueryState {
                water: submitted_water,
                body: submitted_body,
                point: 3,
                local: [0.25, 0.75],
            },
            frac: [0.5, 0.5],
        };
        let cells = [
            [7.0, 2.0, 0.5, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [3.0, 0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0, 0.0],
            [7.0, 0.0, 0.0, 0.0],
        ];
        let mut samples = Vec::new();
        let mut body_samples = Vec::new();

        decode_water_readback(
            &cells,
            &[submitted_water],
            1,
            &[query],
            &mut samples,
            &mut body_samples,
        );

        assert_eq!(samples[0].node, submitted_water);
        assert_eq!(samples[0].height, 7.0);
        assert_eq!(body_samples[0].water, submitted_water);
        assert_eq!(body_samples[0].body, submitted_body);
        assert_eq!(body_samples[0].height, 4.0);
    }
}

#[cfg(test)]
mod wgsl_validation_tests {
    use super::*;

    fn parse_and_validate(wgsl: &str, label: &str) {
        let module =
            naga::front::wgsl::parse_str(wgsl).unwrap_or_else(|err| panic!("{label}: {err}"));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|err| panic!("{label}: {err}"));
    }

    #[test]
    fn water_shaders_validate() {
        parse_and_validate(WATER_WGSL, "water compute");
        parse_and_validate(WATER_3D_RENDER_WGSL, "water 3d render");
        parse_and_validate(&water_render_wgsl(), "water render composed");
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_refraction_offset("));
        assert!(WATER_3D_RENDER_WGSL.contains("let slope = clamp("));
        assert!(WATER_3D_RENDER_WGSL.contains("let wave_speed = clamp(abs(cell.y)"));
        assert!(WATER_3D_RENDER_WGSL.contains("return clamp(direction * (1.0 + motion)"));
    }
}
