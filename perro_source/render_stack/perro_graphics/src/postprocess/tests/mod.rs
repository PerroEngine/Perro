use super::*;

#[test]
fn flattened_lut_to_3d_reads_horizontal_tiles() {
    let size = 2;
    let width = size * size;
    let height = size;
    let rgba = lut_fixture_horizontal(size);

    let (converted, converted_size) =
        flattened_lut_to_3d(rgba, width, height, size).expect("convert LUT");

    assert_eq!(converted_size, size);
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                assert_eq!(
                    lut_texel(&converted, size, x, y, z),
                    [x as u8, y as u8, z as u8, 255]
                );
            }
        }
    }
}

#[test]
fn flattened_lut_to_3d_reads_vertical_tiles_and_infers_size() {
    let size = 3;
    let width = size;
    let height = size * size;
    let rgba = lut_fixture_vertical(size);

    let (converted, converted_size) =
        flattened_lut_to_3d(rgba, width, height, 0).expect("convert LUT");

    assert_eq!(converted_size, size);
    assert_eq!(lut_texel(&converted, size, 0, 0, 0), [0, 0, 0, 255]);
    assert_eq!(lut_texel(&converted, size, 2, 1, 2), [2, 1, 2, 255]);
}

#[test]
fn flattened_lut_to_3d_rejects_bad_layout() {
    let rgba = vec![0u8; 5 * 5 * 4];
    assert!(flattened_lut_to_3d(rgba, 5, 5, 0).is_none());
}

#[test]
fn lut_effect_params_keep_size_and_strength() {
    let encoded = encode_effect_params(&PostProcessEffect::Lut2D {
        texture_path: "res://luts/test.png".into(),
        size: 32,
        strength: 0.75,
    });

    assert_eq!(encoded.effect_type, EFFECT_LUT_2D);
    assert_eq!(encoded.params0, [0.75, 32.0, 0.0, 0.0]);
}

#[test]
fn pixel_art_params_keep_virtual_resolution_and_color_controls() {
    let encoded = encode_effect_params(&PostProcessEffect::PixelArt {
        virtual_height: 180,
        color_levels: 8,
        dither_strength: 0.125,
    });

    assert_eq!(encoded.effect_type, EFFECT_PIXEL_ART);
    assert_eq!(encoded.params0, [180.0, 8.0, 0.125, 0.0]);
}

#[test]
fn color_grade_params_pack_all_controls() {
    let encoded = encode_effect_params(&PostProcessEffect::ColorGrade {
        exposure: 0.1,
        contrast: 1.2,
        brightness: -0.1,
        saturation: 1.3,
        gamma: 0.9,
        temperature: 0.2,
        tint: -0.2,
        hue_shift: 0.5,
        vibrance: 0.4,
        lift: [0.01, 0.02, 0.03],
        gain: [1.1, 1.2, 1.3],
        offset: [-0.01, -0.02, -0.03],
    });

    assert_eq!(encoded.effect_type, EFFECT_COLOR_GRADE);
    assert_eq!(encoded.params0, [0.1, 1.2, -0.1, 1.3]);
    assert_eq!(encoded.params1, [0.9, 0.2, -0.2, 0.5]);
    assert_eq!(encoded.params2, [0.01, 0.02, 0.03, 0.4]);
    assert_eq!(encoded.params3, [1.1, 1.2, 1.3, 0.0]);
    assert_eq!(encoded.params4, [-0.01, -0.02, -0.03, 0.0]);
}

#[test]
fn chroma_key_params_pack_rgb_tolerance_and_softness() {
    let encoded = encode_effect_params(&PostProcessEffect::ChromaKey {
        color: perro_structs::Color::from_hex("#00ff00aa").expect("valid color"),
        tolerance: 0.1,
        softness: 0.05,
    });

    assert_eq!(encoded.effect_type, EFFECT_CHROMA_KEY);
    assert_eq!(encoded.params0, [0.0, 1.0, 0.0, 0.1]);
    assert_eq!(encoded.params1, [0.05, 0.0, 0.0, 0.0]);
}

#[test]
fn chroma_key_joins_merged_color_run() {
    let effects = [
        PostProcessEffect::Saturate { amount: 1.2 },
        PostProcessEffect::ChromaKey {
            color: perro_structs::Color::GREEN,
            tolerance: 0.1,
            softness: 0.05,
        },
    ];
    let mut steps = Vec::new();
    let mut descriptors = Vec::new();

    build_chain_steps_into(&effects, &mut steps, &mut descriptors);

    assert!(matches!(
        steps.as_slice(),
        [ChainStep::Merged { ops: 2, .. }]
    ));
    assert_eq!(descriptors[3][0] as u32, EFFECT_CHROMA_KEY);
}

#[test]
fn exposure_config_skips_post_chain_passes() {
    let effects = [PostProcessEffect::Exposure {
        exposure: 0.5,
        auto_exposure: true,
        min_exposure: -4.0,
        max_exposure: 4.0,
        speed_up: 3.0,
        speed_down: 1.0,
        target_luminance: 0.18,
    }];
    let mut steps = Vec::new();
    let mut descriptors = Vec::new();

    build_chain_steps_into(&effects, &mut steps, &mut descriptors);

    assert!(steps.is_empty());
    assert!(!PostProcessor::has_effects(&effects));
}

#[test]
fn bloom_params_keep_scene_threshold_and_radius() {
    let encoded = encode_effect_params(&PostProcessEffect::Bloom {
        strength: 0.7,
        threshold: 2.0,
        radius: 3.5,
    });

    assert_eq!(encoded.params0, [0.7, 2.0, 3.5, 0.0]);
}

#[test]
fn post_bind_group_key_changes_on_generations_and_inputs() {
    let a = PostBindGroupKey {
        input_kind: PostInputKind::External,
        external_input_view_key: 10,
        depth_view_key: 20,
        uniform_buffer_generation: 1,
        params_buffer_generation: 1,
        lut_2d_key: 0,
        lut_3d_key: 0,
    };
    let b = PostBindGroupKey {
        uniform_buffer_generation: 2,
        ..a
    };
    let c = PostBindGroupKey {
        input_kind: PostInputKind::PingA,
        external_input_view_key: 0,
        ..a
    };
    let d = PostBindGroupKey {
        external_input_view_key: 11,
        ..a
    };
    let e = PostBindGroupKey {
        depth_view_key: 21,
        ..a
    };

    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
    assert_ne!(a, e);
}

#[test]
fn lut_hashes_track_effect_identity() {
    let lut_2d = PostProcessEffect::Lut2D {
        texture_path: "res://luts/a.png".into(),
        size: 16,
        strength: 1.0,
    };
    let lut_3d = PostProcessEffect::Lut3D {
        texture_path: "res://luts/a.png".into(),
        size: 16,
        strength: 1.0,
    };

    assert_ne!(lut_hash_2d(&lut_2d), 0);
    assert_ne!(lut_hash_3d(&lut_3d), 0);
    assert_eq!(lut_hash_2d(&lut_3d), 0);
    assert_eq!(lut_hash_3d(&lut_2d), 0);
}

fn lut_fixture_horizontal(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * size * 4) as usize];
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let index = ((y * size * size + z * size + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&[x as u8, y as u8, z as u8, 255]);
            }
        }
    }
    rgba
}

fn lut_fixture_vertical(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * size * 4) as usize];
    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let index = (((z * size + y) * size + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&[x as u8, y as u8, z as u8, 255]);
            }
        }
    }
    rgba
}

fn lut_texel(rgba: &[u8], size: u32, x: u32, y: u32, z: u32) -> [u8; 4] {
    let index = (((z * size + y) * size + x) * 4) as usize;
    [
        rgba[index],
        rgba[index + 1],
        rgba[index + 2],
        rgba[index + 3],
    ]
}

#[test]
fn color_grade_merges_with_cheap_ops_using_variable_descriptors() {
    let effects = [
        PostProcessEffect::Saturate { amount: 1.2 },
        PostProcessEffect::ColorGrade {
            exposure: 0.5,
            contrast: 1.1,
            brightness: 0.05,
            saturation: 1.0,
            gamma: 2.2,
            temperature: 0.1,
            tint: -0.05,
            hue_shift: 0.25,
            vibrance: 0.3,
            lift: [0.01, 0.02, 0.03],
            gain: [1.1, 1.2, 1.3],
            offset: [-0.01, -0.02, -0.03],
        },
        PostProcessEffect::BlackWhite { amount: 0.5 },
    ];
    let mut steps = Vec::new();
    let mut descriptors_scratch = Vec::new();
    build_chain_steps_into(&effects, &mut steps, &mut descriptors_scratch);
    assert_eq!(steps.len(), 1, "whole run folds into one merged pass");
    let ChainStep::Merged { ops, descriptors } = &steps[0] else {
        panic!("expected merged step");
    };
    assert_eq!(*ops, 3);
    let descriptors = &descriptors_scratch[descriptors.clone()];
    // saturate: header + 2, color_grade: header + 5, black_white: header + 2.
    assert_eq!(descriptors.len(), 3 + 6 + 3);
    // Headers carry [type, param_vec4_count, ...] at the right cursors.
    assert_eq!(descriptors[0][0] as u32, EFFECT_SATURATE);
    assert_eq!(descriptors[0][1], 2.0);
    assert_eq!(descriptors[3][0] as u32, EFFECT_COLOR_GRADE);
    assert_eq!(descriptors[3][1], 5.0);
    assert_eq!(descriptors[4], [0.5, 1.1, 0.05, 1.0]);
    assert_eq!(descriptors[9][0] as u32, EFFECT_BLACK_WHITE);
    assert_eq!(descriptors[9][1], 2.0);
}

#[test]
fn chain_builder_reuses_step_and_descriptor_capacity() {
    let effects = [
        PostProcessEffect::Saturate { amount: 1.2 },
        PostProcessEffect::BlackWhite { amount: 0.5 },
        PostProcessEffect::Pixelate { size: 4.0 },
        PostProcessEffect::Vignette {
            radius: 0.75,
            softness: 0.2,
            strength: 0.6,
        },
        PostProcessEffect::ReverseFilter {
            color: [1.0, 0.5, 0.25],
            strength: 0.4,
            softness: 0.1,
        },
    ];
    let mut steps = Vec::new();
    let mut descriptors = Vec::new();

    build_chain_steps_into(&effects, &mut steps, &mut descriptors);
    let step_capacity = steps.capacity();
    let descriptor_capacity = descriptors.capacity();
    let step_count = steps.len();
    let descriptor_count = descriptors.len();

    build_chain_steps_into(&effects, &mut steps, &mut descriptors);

    assert_eq!(steps.capacity(), step_capacity);
    assert_eq!(descriptors.capacity(), descriptor_capacity);
    assert_eq!(steps.len(), step_count);
    assert_eq!(descriptors.len(), descriptor_count);
}

#[test]
fn exposure_cfg_stays_out_of_scene_referred_passes() {
    let effects = [
        PostProcessEffect::Exposure {
            exposure: 0.0,
            auto_exposure: true,
            min_exposure: -8.0,
            max_exposure: 8.0,
            speed_up: 3.0,
            speed_down: 1.0,
            target_luminance: 0.18,
        },
        PostProcessEffect::Bloom {
            strength: 0.7,
            threshold: 1.25,
            radius: 2.0,
        },
    ];
    let mut steps = Vec::new();
    let mut descriptors = Vec::new();

    build_chain_steps_into(&effects, &mut steps, &mut descriptors);

    assert_eq!(steps.len(), 1);
    assert!(matches!(steps[0], ChainStep::Single(1)));
    assert!(!PostProcessor::has_effects(&effects[..1]));
    assert!(PostProcessor::has_effects(&effects));
}

#[test]
fn lru_cache_evicts_least_recently_used_on_overflow() {
    let mut cache: PostLruCache<u32> = PostLruCache::new();
    for key in 0..4u64 {
        cache.insert_capped(key, key as u32, key, 4);
    }
    // Key 0 is oldest; touching it at a later clock makes key 1 the victim.
    assert!(cache.touch(0, 10));

    cache.insert_capped(9, 9, 11, 4);

    assert_eq!(cache.peek(9), Some(&9));
    assert_eq!(cache.peek(0), Some(&0));
    assert_eq!(cache.peek(1), None);
    assert_eq!(cache.peek(2), Some(&2));
    assert_eq!(cache.peek(3), Some(&3));
}

#[test]
fn lru_cache_keeps_entries_used_this_frame() {
    let mut cache: PostLruCache<u32> = PostLruCache::new();
    // A chain wider than the cap: every entry lands on the same frame, so
    // nothing it still draws with may drop.
    for key in 0..6u64 {
        cache.insert_capped(key, key as u32, 7, 4);
    }

    for key in 0..6u64 {
        assert_eq!(cache.peek(key), Some(&(key as u32)));
    }

    // Next frame the overflow drains, one entry per insert. Key 0 goes
    // untouched, so it is the only eviction candidate.
    for key in 1..6u64 {
        assert!(cache.touch(key, 8));
    }
    cache.insert_capped(6, 6, 8, 4);

    assert_eq!(cache.peek(0), None);
    assert_eq!(cache.peek(6), Some(&6));
    for key in 1..6u64 {
        assert_eq!(cache.peek(key), Some(&(key as u32)));
    }
}

#[test]
fn lru_cache_reinsert_of_live_key_evicts_nothing() {
    let mut cache: PostLruCache<u32> = PostLruCache::new();
    for key in 0..4u64 {
        cache.insert_capped(key, key as u32, key, 4);
    }

    cache.insert_capped(0, 100, 9, 4);

    assert_eq!(cache.peek(0), Some(&100));
    for key in 1..4u64 {
        assert_eq!(cache.peek(key), Some(&(key as u32)));
    }
}

#[test]
fn lru_cache_clear_drops_everything() {
    let mut cache: PostLruCache<u32> = PostLruCache::new();
    cache.insert_capped(1, 1, 0, 4);
    assert!(!cache.is_empty());

    cache.clear();

    assert!(cache.is_empty());
    assert_eq!(cache.peek(1), None);
    assert!(!cache.touch(1, 0));
}

#[test]
fn view_key_ring_retires_oldest_pair() {
    let mut slots = [PostViewKeys::default(); POST_VIEW_KEY_SLOTS];
    let mut next = 0usize;
    let pair = |n: u64| PostViewKeys {
        external_input: n,
        depth: n + 100,
    };

    // Empty slots retire nothing.
    for key in 1..=POST_VIEW_KEY_SLOTS as u64 {
        assert_eq!(rotate_view_key_slot(&mut slots, &mut next, pair(key)), None);
    }
    // Live pairs never rotate: the main chain runs two per frame.
    assert_eq!(rotate_view_key_slot(&mut slots, &mut next, pair(2)), None);
    assert_eq!(rotate_view_key_slot(&mut slots, &mut next, pair(1)), None);

    let retired = rotate_view_key_slot(&mut slots, &mut next, pair(99));

    assert_eq!(retired, Some(pair(1)));
    assert!(slots.contains(&pair(99)));
    assert!(!slots.contains(&pair(1)));
}

#[test]
fn view_key_ring_holds_a_full_frames_worth_of_pairs() {
    let mut slots = [PostViewKeys::default(); POST_VIEW_KEY_SLOTS];
    let mut next = 0usize;
    let pair = |n: u64| PostViewKeys {
        external_input: n,
        depth: n + 100,
    };
    // Two chains per frame, one key generation: the frame's own pairs must
    // survive the flip that follows, or bind groups die mid-frame.
    rotate_view_key_slot(&mut slots, &mut next, pair(1));
    rotate_view_key_slot(&mut slots, &mut next, pair(2));

    assert_eq!(rotate_view_key_slot(&mut slots, &mut next, pair(9)), None);
    assert_eq!(
        rotate_view_key_slot(&mut slots, &mut next, pair(10)),
        Some(pair(1))
    );

    assert!(slots.contains(&pair(9)));
    assert!(slots.contains(&pair(10)));
    assert!(slots.contains(&pair(2)));
}

#[test]
fn view_key_ring_ignores_a_pair_it_already_holds() {
    let mut slots = [PostViewKeys::default(); POST_VIEW_KEY_SLOTS];
    let mut next = 0usize;
    let keys = PostViewKeys {
        external_input: 4,
        depth: 5,
    };

    rotate_view_key_slot(&mut slots, &mut next, keys);

    for _ in 0..8 {
        assert_eq!(rotate_view_key_slot(&mut slots, &mut next, keys), None);
    }
    assert_eq!(next, 1);
}
