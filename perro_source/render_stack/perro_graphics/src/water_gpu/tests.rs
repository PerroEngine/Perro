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
            quality: WaterQuality::Ultra,
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
            quality: WaterQuality::Ultra,
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
        // per-chunk LOD contract: integer chunk uv + edge snap, else cracks
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_chunk_uv"));
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_chunk_edge_snap"));
        assert!(WATER_3D_RENDER_WGSL.contains("gy = (gy / r) * r;"));
        assert!(WATER_3D_RENDER_WGSL.contains("gx = (gx / r) * r;"));
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_chunk_side_vertex"));
        // normals differentiate at the sim cell, not the (per-chunk) mesh step
        assert!(WATER_3D_RENDER_WGSL.contains("1.0 / max(f32(w.sim.z), 2.0)"));
        assert!(!WATER_3D_RENDER_WGSL.contains("chunk.uv_origin"));
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

    /// All-pass frustum planes: nothing is culled.
    fn open_frustum() -> [[f32; 4]; 6] {
        [[0.0, 1.0, 0.0, 1.0e9]; 6]
    }

    fn chunks_for(
        water: &Water3DState,
        camera: [f32; 3],
        lod_scale: [f32; 2],
    ) -> Vec<WaterRenderChunkGpu> {
        let lod = water_lod_3d(water, camera, lod_scale);
        let gpu = water_gpu_3d(
            NodeID::from_parts(1, 0),
            water,
            lod.grid,
            0,
            water_cell_count(lod.grid.sim) as u32,
            lod.ripple_blend,
            [0.0, 0.0, 0.0],
        );
        let mut out = Vec::new();
        let mut scratch = Vec::new();
        build_render_chunks_3d(
            &mut out,
            &mut scratch,
            0,
            water,
            gpu,
            camera,
            lod_scale,
            &open_frustum(),
        );
        out
    }

    #[test]
    fn water_quality_tier_drives_sim_grid_and_readback_rate() {
        let mut water = test_water_3d();
        for (tier, sim, rate) in [
            (WaterQuality::Low, [64, 64], 10.0),
            (WaterQuality::Medium, [96, 96], 20.0),
            (WaterQuality::High, [160, 160], 30.0),
            (WaterQuality::Ultra, [256, 256], 60.0),
        ] {
            water.quality = tier;
            assert_eq!(
                water_lod_3d(&water, [0.0, 2.0, 0.0], [720.0, 0.0]).grid.sim,
                sim
            );
            assert_eq!(tier.sample_readback_rate(), rate);
        }
        assert_eq!(water_cell_count([0, 0]), 0);
        assert_eq!(water_cell_count([1, 1]), 1);
    }

    #[test]
    fn water_quality_default_is_the_low_tier() {
        let params = perro_structs::WaterQuality::default();
        assert_eq!(params, perro_structs::WaterQuality::Low);
        assert_eq!(params.target_edge_pixels(), 32.0);
        assert_eq!(params.max_chunk_quads(), 16);
    }

    #[test]
    fn chunk_quads_follow_screen_space_target_not_world_grid() {
        // Same world chunk, 4x further away -> 4x fewer quads per axis.
        let near = water_chunk_quads(WaterQuality::High, 16.0, 40.0, [800.0, 0.0]);
        let far = water_chunk_quads(WaterQuality::High, 16.0, 160.0, [800.0, 0.0]);
        assert!(near > far, "{near} vs {far}");
        assert_eq!(near / far, 4);
        // Half the window height -> half the projection scale -> half the quads.
        let small_window = water_chunk_quads(WaterQuality::High, 16.0, 40.0, [400.0, 0.0]);
        assert_eq!(small_window * 2, near);
        // A finer tier asks for more quads at the same distance.
        assert!(
            water_chunk_quads(WaterQuality::Ultra, 16.0, 40.0, [800.0, 0.0])
                >= water_chunk_quads(WaterQuality::Low, 16.0, 40.0, [800.0, 0.0])
        );
        // Always a power of 2 and capped by the tier.
        for tier in WaterQuality::ALL {
            for distance in [0.1, 1.0, 7.5, 100.0, 5000.0] {
                let q = water_chunk_quads(tier, 16.0, distance, [800.0, 0.0]);
                assert!(q.is_power_of_two(), "{tier} @ {distance} -> {q}");
                assert!(q <= tier.max_chunk_quads());
                assert!(q >= 1);
            }
        }
    }

    #[test]
    fn chunk_count_scales_with_body_size() {
        assert_eq!(water_chunk_counts([8.0, 8.0]), [1, 1]);
        assert_eq!(water_chunk_counts([100.0, 100.0]), [8, 8]);
        // Capped, but a big body still splits far past the old 2x2.
        assert_eq!(water_chunk_counts([5000.0, 12.0]), [8, 1]);
    }

    #[test]
    fn distant_chunks_get_lower_lod_than_near_chunks() {
        let mut water = test_water_3d();
        water.quality = WaterQuality::High;
        water.size = [200.0, 200.0];
        water.shape = WaterShapeState::Rect;
        // Camera off one edge, looking across the body.
        let chunks = chunks_for(&water, [0.0, 4.0, 140.0], [800.0, 0.0]);
        assert!(
            chunks.len() > 4,
            "expect a real chunk grid, got {}",
            chunks.len()
        );
        let near = chunks
            .iter()
            .max_by_key(|c| c.chunk[1])
            .expect("near chunk");
        let far = chunks.iter().min_by_key(|c| c.chunk[1]).expect("far chunk");
        assert!(
            near.quads > far.quads,
            "near {} should out-tessellate far {}",
            near.quads,
            far.quads
        );
    }

    #[test]
    fn chunk_edge_snap_ratios_stay_bounded_and_crack_free() {
        let mut water = test_water_3d();
        water.quality = WaterQuality::Ultra;
        water.size = [200.0, 200.0];
        let chunks = chunks_for(&water, [0.0, 4.0, 140.0], [800.0, 0.0]);
        let by_coord: std::collections::HashMap<(u32, u32), &WaterRenderChunkGpu> = chunks
            .iter()
            .map(|c| ((c.chunk[0], c.chunk[1]), c))
            .collect();
        for chunk in &chunks {
            assert!(chunk.quads.is_power_of_two());
            let (cx, cy) = (chunk.chunk[0], chunk.chunk[1]);
            let neighbours = [
                ((cx.wrapping_sub(1), cy), 0),
                ((cx + 1, cy), 1),
                ((cx, cy.wrapping_sub(1)), 2),
                ((cx, cy + 1), 3),
            ];
            for ((nx, ny), edge) in neighbours {
                let ratio = (chunk.edge_snap >> (edge * 8)) & 0xFF;
                assert!((1..=WATER_CHUNK_MAX_LOD_RATIO).contains(&ratio), "{ratio}");
                assert!(ratio.is_power_of_two(), "snap ratio must divide quads");
                let Some(other) = by_coord.get(&(nx, ny)) else {
                    // Body border: no neighbour, no snap, and a side wall.
                    assert_eq!(ratio, 1);
                    assert_ne!(chunk.flags & (1 << edge), 0);
                    continue;
                };
                // Snapping only ever collapses onto the COARSER neighbour, and
                // the ratio must divide this chunk's quad count exactly, else
                // the snapped vertex misses the neighbour knot -> crack.
                if other.quads < chunk.quads {
                    assert_eq!(ratio, chunk.quads / other.quads);
                } else {
                    assert_eq!(ratio, 1);
                }
                assert_eq!(chunk.quads % ratio, 0);
                assert_eq!(chunk.flags & (1 << edge), 0);
            }
        }
    }

    #[test]
    fn lower_quality_costs_fewer_vertices() {
        let mut water = test_water_3d();
        water.size = [200.0, 200.0];
        let mut previous = 0;
        for tier in WaterQuality::ALL {
            water.quality = tier;
            let lod = water_lod_3d(&water, [0.0, 4.0, 140.0], [800.0, 0.0]);
            let gpu = water_gpu_3d(
                NodeID::from_parts(1, 0),
                &water,
                lod.grid,
                0,
                water_cell_count(lod.grid.sim) as u32,
                lod.ripple_blend,
                [0.0, 0.0, 0.0],
            );
            let total: u32 = chunks_for(&water, [0.0, 4.0, 140.0], [800.0, 0.0])
                .iter()
                .map(|chunk| water_render_chunk_vertex_count(&gpu, chunk))
                .sum();
            assert!(total > previous, "{tier} -> {total} vs {previous}");
            previous = total;
        }
    }

    #[test]
    fn chunk_lod_ratio_clamp_lifts_coarse_neighbours() {
        let mut quads = vec![64, 1, 1, 1];
        clamp_chunk_lod_ratio(&mut quads, [4, 1]);
        assert_eq!(quads, vec![64, 16, 4, 1]);
        for pair in quads.windows(2) {
            assert!(pair[0] / pair[1] <= WATER_CHUNK_MAX_LOD_RATIO);
        }
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
                sim: water.quality.sim_resolution(),
                render: water.quality.sim_resolution(),
            },
            4,
            64,
            1.0,
        );
        assert_eq!(staged.node, 7);
        assert_eq!(staged.sim, [4, 64, 256, 256]);
        assert_eq!(staged.kind, 2);
        assert_eq!(staged.flags[2] & WATER_FLAG_PAUSED, 0);
        let mut paused = water;
        paused.paused = true;
        let paused_staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &paused,
            WaterGridResolution {
                sim: paused.quality.sim_resolution(),
                render: paused.quality.sim_resolution(),
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

    // --- coastline upload gating -------------------------------------------

    fn idle_coastline_water_2d() -> Water2DState {
        let mut water = test_water_2d();
        water.impacts = Arc::from([]);
        water.coastline_shapes = Arc::from([WaterCoastlineShape2D::Circle {
            center: [0.0, 0.0],
            radius: 4.0,
        }]);
        water
    }

    #[test]
    fn idle_coastline_raster_skips_repeat_work_2d() {
        let node = NodeID::from_parts(7, 0);
        let mut cache: HashMap<NodeID, CachedCoastline> = HashMap::new();
        let water = idle_coastline_water_2d();
        let cells = water_cell_count([256, 256]);
        let mut out = vec![[0.0f32; 4]; cells];

        assert!(raster_coastline_2d(
            &mut out,
            [256, 256],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
        let first = out.clone();
        // Same static signature, still no impacts -> no blend loop, no upload.
        for _ in 0..8 {
            assert!(!raster_coastline_2d(
                &mut out,
                [256, 256],
                &water,
                node,
                &mut cache,
                (0, cells)
            ));
        }
        assert_eq!(out, first);

        // A new impact re-opens the gate and the wake lands in the cells.
        let mut splashed = water.clone();
        splashed.impacts = Arc::from([perro_render_bridge::WaterImpact2D {
            position: [0.0, 0.0],
            velocity: [1.0, 0.0],
            strength: 2.0,
            radius: 6.0,
            cavitation: 0.5,
        }]);
        assert!(raster_coastline_2d(
            &mut out,
            [256, 256],
            &splashed,
            node,
            &mut cache,
            (0, cells)
        ));
        assert_ne!(out, first);
        // Dropping the impact must write once more to clear the wake, then gate.
        assert!(raster_coastline_2d(
            &mut out,
            [256, 256],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
        assert_eq!(out, first);
        assert!(!raster_coastline_2d(
            &mut out,
            [256, 256],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
        // A slot move (offsets shifted by another water resizing) forces a write.
        assert!(raster_coastline_2d(
            &mut out,
            [256, 256],
            &water,
            node,
            &mut cache,
            (cells, cells)
        ));
    }

    #[test]
    fn idle_coastline_raster_skips_repeat_work_3d() {
        let node = NodeID::from_parts(8, 0);
        let mut cache: HashMap<NodeID, CachedCoastline> = HashMap::new();
        let mut water = test_water_3d();
        water.impacts = Arc::from([]);
        water.coastline_shapes = Arc::from([WaterCoastlineShape3D::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 4.0,
        }]);
        let cells = water_cell_count([128, 128]);
        let mut out = vec![[0.0f32; 4]; cells];

        assert!(raster_coastline_3d(
            &mut out,
            [128, 128],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
        for _ in 0..4 {
            assert!(!raster_coastline_3d(
                &mut out,
                [128, 128],
                &water,
                node,
                &mut cache,
                (0, cells)
            ));
        }
    }

    #[test]
    fn impacts_only_water_gates_when_idle() {
        // No coastline shapes: the impacts-only raster path must gate too.
        let node = NodeID::from_parts(9, 0);
        let mut cache: HashMap<NodeID, CachedCoastline> = HashMap::new();
        let mut water = test_water_2d();
        water.impacts = Arc::from([]);
        let cells = water_cell_count([64, 64]);
        let mut out = vec![[1.0f32; 4]; cells];

        assert!(raster_coastline_2d(
            &mut out,
            [64, 64],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
        assert_eq!(out, vec![[0.0f32; 4]; cells]);
        assert!(!raster_coastline_2d(
            &mut out,
            [64, 64],
            &water,
            node,
            &mut cache,
            (0, cells)
        ));
    }

    async fn water_test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;
        adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("perro_water_gate_test_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
            })
            .await
            .ok()
    }

    fn camera_uniform_bgl(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    // The 3D water pass and the flip-splash pass behind it both attach the
    // depth target `Gpu3D::water_depth_attachment` hands out - now a private,
    // water-only one at 1 sample, with scene occlusion moved into the shaders.
    // The splash pass therefore binds the scene-depth group as well; encode both
    // passes under a validation scope so a layout/binding mismatch fails here.
    #[test]
    fn water_3d_and_splash_passes_encode_against_a_private_depth_target() {
        pollster::block_on(async {
            let Some((device, queue)) = water_test_device().await else {
                return;
            };
            let camera_2d_bgl = camera_uniform_bgl(&device, "perro_water_test_camera2d");
            let camera_3d_bgl = camera_uniform_bgl(&device, "perro_water_test_camera3d");
            let scene_depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_water_test_scene_depth"),
                size: wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let scene_depth_view = scene_depth.create_view(&wgpu::TextureViewDescriptor::default());
            // Stand-in for the pass's private depth: cleared, never a copy.
            let private_depth = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_water_test_private_depth"),
                size: wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let private_depth_view =
                private_depth.create_view(&wgpu::TextureViewDescriptor::default());
            let color = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("perro_water_test_color"),
                size: wgpu::Extent3d {
                    width: 32,
                    height: 32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
            let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_water_test_camera_buffer"),
                // Scene3D (view_proj, lights, inv_view_proj) as the 3D water
                // shader declares it; a short buffer trips late binding-size
                // validation.
                size: 2048,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_water_test_camera_bg"),
                layout: &camera_3d_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }],
            });
            let mut water_gpu = GpuWater::new(
                &device,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                1,
                &camera_2d_bgl,
                &camera_3d_bgl,
                &scene_depth_view,
                32,
                32,
            );
            water_gpu.set_scene_color_size(&device, &scene_depth_view, 32, 32);
            let waters = [(NodeID::from_parts(7, 0), test_water_3d())];
            water_gpu.prepare(
                &device,
                &queue,
                &[],
                &waters,
                WaterPrepareContext {
                    camera_3d_position: [0.0, 8.0, 24.0],
                    camera_3d_frustum_planes: [[0.0, 0.0, 0.0, 1.0e9]; 6],
                    camera_3d_lod_scale: [360.0, 0.0],
                    sky_color: [0.4, 0.6, 0.9],
                    time_seconds: 0.0,
                    delta_seconds: 1.0 / 60.0,
                    scene_geometry_present: true,
                },
            );
            assert!(
                water_gpu.flip_particle_count() > 0,
                "splash pass must draw, or its bindings validate vacuously"
            );
            let error_scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("perro_water_test_render_encoder"),
            });
            water_gpu.encode(&mut encoder);
            water_gpu.capture_scene_color(&device, &mut encoder, &color_view);
            water_gpu.render_3d(
                &mut encoder,
                &color_view,
                &private_depth_view,
                &camera_bind_group,
                true,
            );
            queue.submit([encoder.finish()]);
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            let validation_error = error_scope.pop().await;
            assert!(
                validation_error.is_none(),
                "water 3D / splash passes failed validation: {validation_error:?}"
            );
        });
    }

    #[test]
    fn idle_water_prepare_stops_uploading_coastline_cells() {
        let Some((device, queue)) = pollster::block_on(water_test_device()) else {
            // No adapter in this environment; the CPU gate tests still cover it.
            return;
        };
        let camera_2d_bgl = camera_uniform_bgl(&device, "perro_water_test_camera2d");
        let camera_3d_bgl = camera_uniform_bgl(&device, "perro_water_test_camera3d");
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("perro_water_test_depth"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let mut water_gpu = GpuWater::new(
            &device,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            1,
            &camera_2d_bgl,
            &camera_3d_bgl,
            &depth_view,
            4,
            4,
        );

        let waters = [(NodeID::from_parts(3, 0), idle_coastline_water_2d())];
        let ctx = WaterPrepareContext {
            camera_3d_position: [0.0, 0.0, 0.0],
            camera_3d_frustum_planes: [[0.0, 0.0, 0.0, 1.0]; 6],
            camera_3d_lod_scale: [0.0; 2],
            sky_color: [0.0, 0.0, 0.0],
            time_seconds: 0.0,
            delta_seconds: 1.0 / 60.0,
            scene_geometry_present: false,
        };
        water_gpu.prepare(&device, &queue, &waters, &[], ctx);
        let after_first = water_gpu.coastline_upload_bytes();
        let cells = water_cell_count([256, 256]) as u64;
        assert_eq!(after_first, cells * 16);

        for frame in 1..=10u32 {
            let mut ctx = ctx;
            ctx.time_seconds = frame as f32 / 60.0;
            water_gpu.prepare(&device, &queue, &waters, &[], ctx);
        }
        // Idle 256x256 water: 10 frames of coastline uploads elided entirely.
        assert_eq!(water_gpu.coastline_upload_bytes(), after_first);
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
        parse_and_validate(WATER_SCENE_COLOR_BLIT_WGSL, "water scene color blit");
        // Half-res refraction copy: every scene color load must rescale its
        // full-res pixel coord through the shared remap helper.
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_scene_color_coord("));
        assert_eq!(
            WATER_3D_RENDER_WGSL
                .matches("textureLoad(scene_color_tex,")
                .count(),
            WATER_3D_RENDER_WGSL
                .matches("textureLoad(scene_color_tex, water_scene_color_coord(")
                .count(),
        );
        assert!(WATER_3D_RENDER_WGSL.contains("fn water_refraction_offset("));
        assert!(WATER_3D_RENDER_WGSL.contains("let slope = clamp("));
        assert!(WATER_3D_RENDER_WGSL.contains("let wave_speed = clamp(abs(cell.y)"));
        assert!(WATER_3D_RENDER_WGSL.contains("return clamp(direction * (1.0 + motion)"));
    }
}
