mod water_lighting {
    use super::*;

    #[test]
    fn water_body_scene_fields_parse() {
        let scene = Parser::new(
            r#"
            $root = @water
            [water]
            [WaterBody2D]
                shape = { type="quad" width=64 height=32 }
                resolution = (256, 128)
                render_resolution = (512, 256)
                depth = 7.5
                flow = (2, 0)
                wind = (0, 1)
                idle_mode = "storm"
                wave_speed = 3.0
                wave_scale = 1.5
                wave_length = 12.0
                wake_strength = 2.0
                foam_strength = 0.8
                damping = 0.96
                buoyancy = 4.0
                drag = 0.25
                sample_readback_rate = 20
                lod_near_distance = 80
                lod_mid_distance = 240
                lod_far_distance = 720
                lod_min_resolution = 16
                collision_layers = [2, 4]
                collision_mask = [1, 3]
                deep_color = (0.0, 0.1, 0.2, 0.9)
                shallow_color = (0.1, 0.5, 0.7, 0.35)
                shallow_depth = 10
                sky_bias = { ratio=0.4 }
                material = { transparency=0.31 reflectivity=0.52 roughness=0.19 fresnel_power=4.5 normal_strength=1.4 ripple_scale=0.8 foam_color=(0.7, 0.9, 1.0, 1.0) foam_amount=0.67 crest_foam_threshold=0.43 caustic_strength=0.21 refraction_strength=0.13 scattering_strength=0.18 distance_fog_strength=0.35 }
                coastline = { foam_color=(0.8, 0.9, 1.0, 1.0) foam_strength=0.9 foam_width=2.0 cutoff_softness=0.4 wave_reflection=0.5 wave_damping=0.25 edge_noise=0.1 }
                debug = true
            [/WaterBody2D]
            [/water]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let water = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "water")
            .expect("water node");

        match &water.node.data {
            SceneNodeData::WaterBody2D(node) => {
                assert_eq!(
                    node.water.shape,
                    perro_nodes::WaterShape::Rect {
                        size: Vector2::new(64.0, 32.0),
                    }
                );
                assert_eq!(node.water.resolution, [256, 128]);
                assert_eq!(node.water.render_resolution, [512, 256]);
                assert_eq!(node.water.depth, 7.5);
                assert_eq!(node.water.flow.x, 2.0);
                assert_eq!(node.water.wind.y, 1.0);
                assert_eq!(node.water.idle_mode, perro_nodes::WaterIdleMode::Storm);
                assert_eq!(node.water.wave.speed, 3.0);
                assert_eq!(node.water.wave.scale, 1.5);
                assert_eq!(node.water.wave.length, 12.0);
                assert_eq!(node.water.wave.damping, 0.96);
                assert_eq!(node.water.physics.wake_strength, 2.0);
                assert_eq!(node.water.physics.foam_strength, 0.8);
                assert_eq!(node.water.physics.buoyancy, 4.0);
                assert_eq!(node.water.physics.drag, 0.25);
                assert_eq!(node.water.physics.sample_readback_rate, 20.0);
                assert_eq!(node.water.lod.near_distance, 80.0);
                assert_eq!(node.water.lod.mid_distance, 240.0);
                assert_eq!(node.water.lod.far_distance, 720.0);
                assert_eq!(node.water.lod.min_resolution, [16, 16]);
                assert_eq!(node.water.collision_layers.bits(), 0b1010);
                assert_eq!(node.water.collision_mask.bits(), 0b101);
                assert_eq!(node.water.optics.deep_color, Color::new(0.0, 0.1, 0.2, 0.9));
                assert_eq!(
                    node.water.optics.shallow_color,
                    Color::new(0.1, 0.5, 0.7, 0.35)
                );
                assert_eq!(node.water.optics.shallow_depth, 10.0);
                assert_eq!(node.water.optics.sky_bias.ratio(), 0.4);
                assert_eq!(node.water.visual.transparency, 0.31);
                assert_eq!(node.water.visual.reflectivity, 0.52);
                assert_eq!(node.water.visual.roughness, 0.19);
                assert_eq!(node.water.visual.fresnel_power, 4.5);
                assert_eq!(node.water.visual.normal_strength, 1.4);
                assert_eq!(node.water.visual.ripple_scale, 0.8);
                assert_eq!(node.water.visual.foam_color, Color::new(0.7, 0.9, 1.0, 1.0));
                assert_eq!(node.water.visual.foam_amount, 0.67);
                assert_eq!(node.water.visual.crest_foam_threshold, 0.43);
                assert_eq!(node.water.visual.caustic_strength, 0.21);
                assert_eq!(node.water.visual.refraction_strength, 0.13);
                assert_eq!(node.water.visual.scattering_strength, 0.18);
                assert_eq!(node.water.visual.distance_fog_strength, 0.35);
                assert_eq!(
                    node.water.coastline.foam_color,
                    Color::new(0.8, 0.9, 1.0, 1.0)
                );
                assert_eq!(node.water.coastline.foam_strength, 0.9);
                assert_eq!(node.water.coastline.foam_width, 2.0);
                assert_eq!(node.water.coastline.cutoff_softness, 0.4);
                assert_eq!(node.water.coastline.wave_reflection, 0.5);
                assert_eq!(node.water.coastline.wave_damping, 0.25);
                assert_eq!(node.water.coastline.edge_noise, 0.1);
                assert!(node.water.debug);
            }
            other => panic!("expected WaterBody2D node, got {other:?}"),
        }
    }

    #[test]
    fn prepare_adds_default_ray_light_3d_when_missing() {
        let scene = Parser::new(
            r#"
            $root = @root
            [root]
            [Node3D/]
            [/root]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let lights = prepared
            .nodes
            .iter()
            .filter(|pending| matches!(pending.node.data, SceneNodeData::RayLight3D(_)))
            .collect::<Vec<_>>();
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0].key_name, "__perro_default_ray_light");
    }

    #[test]
    fn prepare_skips_default_ray_light_3d_for_2d_scene() {
        let scene = Parser::new(
            r#"
            $root = @root
            [root]
            [Node2D/]
            [/root]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let lights = prepared
            .nodes
            .iter()
            .filter(|pending| matches!(pending.node.data, SceneNodeData::RayLight3D(_)))
            .collect::<Vec<_>>();
        assert!(lights.is_empty());
    }

    #[test]
    fn prepare_keeps_existing_ray_light_3d_only() {
        let scene = Parser::new(
            r#"
            $root = @root
            [root]
            [Node3D/]
            [/root]

            [sun]
            [RayLight3D/]
            [/sun]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let lights = prepared
            .nodes
            .iter()
            .filter(|pending| matches!(pending.node.data, SceneNodeData::RayLight3D(_)))
            .collect::<Vec<_>>();
        assert_eq!(lights.len(), 1);
        assert_eq!(lights[0].key_name, "sun");
    }

    #[test]
    fn scene_loader_parses_light_colors_as_color() {
        let scene = Parser::new(
            r##"
            $root = @root
            [root]
            [Node2D/]
            [/root]

            [lamp2d]
            [PointLight2D]
                color = (0.25, 0.5, 0.75, 0.4)
            [/PointLight2D]
            [/lamp2d]

            [lamp3d]
            [RayLight3D]
                color = "#336699"
            [/RayLight3D]
            [/lamp3d]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let point = prepared
            .nodes
            .iter()
            .find_map(|pending| match &pending.node.data {
                SceneNodeData::PointLight2D(node) => Some(node),
                _ => None,
            })
            .expect("point light");
        assert_eq!(point.color, Color::new(0.25, 0.5, 0.75, 0.4));

        let ray = prepared
            .nodes
            .iter()
            .find_map(|pending| match &pending.node.data {
                SceneNodeData::RayLight3D(node) if pending.key_name == "lamp3d" => Some(node),
                _ => None,
            })
            .expect("ray light");
        assert_eq!(ray.color, Color::from_hex("#336699").unwrap());
    }

    #[test]
    fn scene_loader_maps_sprite_3d_color_aliases_to_modulate() {
        let scene = Parser::new(
            r##"
            $root = @root
            [root]
            [Node3D/]
            [/root]

            [sprite_tint]
            [Sprite3D]
                tint = "#11223344"
            [/Sprite3D]
            [/sprite_tint]

            [sprite_color]
            [Sprite3D]
                color = "#55667788"
            [/Sprite3D]
            [/sprite_color]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sprite_color = |name: &str| {
            prepared
                .nodes
                .iter()
                .find_map(|pending| match &pending.node.data {
                    SceneNodeData::Sprite3D(node) if pending.key_name == name => {
                        Some(node.modulate.modulate)
                    }
                    _ => None,
                })
                .expect("sprite")
        };

        assert_eq!(
            sprite_color("sprite_tint"),
            Color::from_hex("#11223344").unwrap()
        );
        assert_eq!(
            sprite_color("sprite_color"),
            Color::from_hex("#55667788").unwrap()
        );
    }

    #[test]
    fn scene_loader_builds_typed_asset_refs() {
        let scene = Parser::new(
            r#"
            $root = @root
            [root]
            [Node2D/]
            [/root]

            [map]
            [TileMap2D]
                tileset = "res://tiles/world.ptileset"
            [/TileMap2D]
            [/map]

            [particles]
            [ParticleEmitter2D]
                profile = "res://particles/smoke.pparticle"
            [/ParticleEmitter2D]
            [/particles]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let tilemap = prepared
            .nodes
            .iter()
            .find_map(|pending| match &pending.node.data {
                SceneNodeData::TileMap2D(node) => Some(node),
                _ => None,
            })
            .expect("tilemap");
        assert_eq!(tilemap.tileset.source(), "res://tiles/world.ptileset");
        assert_eq!(
            tilemap.tileset.id(),
            perro_ids::TileSetID::from_string("res://tiles/world.ptileset")
        );

        let emitter = prepared
            .nodes
            .iter()
            .find_map(|pending| match &pending.node.data {
                SceneNodeData::ParticleEmitter2D(node) => Some(node),
                _ => None,
            })
            .expect("emitter");
        assert_eq!(emitter.profile.source(), "res://particles/smoke.pparticle");
        assert_eq!(
            emitter.profile.id(),
            perro_ids::ParticleProfileID::from_string("res://particles/smoke.pparticle")
        );
    }

    #[test]
    fn light3d_shadow_tuning_fields_parse() {
        let scene = Parser::new(
            r#"
            $root = @root
            [root]
            [Node3D/]
            [/root]

            [sun]
            [RayLight3D]
                shadow = { strength = 0.44 depth_bias = 0.002 normal_bias = 0.09 }
            [/RayLight3D]
            [/sun]

            [bulb]
            [PointLight3D]
                shadow_opacity = 0.33
                shadow_bias = 0.003
                shadow_normal_bias = 0.11
            [/PointLight3D]
            [/bulb]

            [cone]
            [SpotLight3D]
                shadow_strength = 0.22
                shadow_depth_bias = 0.004
                shadow_normal_bias = 0.13
            [/SpotLight3D]
            [/cone]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sun = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sun")
            .expect("sun node");
        match &sun.node.data {
            SceneNodeData::RayLight3D(light) => {
                assert_eq!(light.shadow_strength, 0.44);
                assert_eq!(light.shadow_depth_bias, 0.002);
                assert_eq!(light.shadow_normal_bias, 0.09);
            }
            other => panic!("expected RayLight3D, got {other:?}"),
        }

        let bulb = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "bulb")
            .expect("bulb node");
        match &bulb.node.data {
            SceneNodeData::PointLight3D(light) => {
                assert_eq!(light.shadow_strength, 0.33);
                assert_eq!(light.shadow_depth_bias, 0.003);
                assert_eq!(light.shadow_normal_bias, 0.11);
            }
            other => panic!("expected PointLight3D, got {other:?}"),
        }

        let cone = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "cone")
            .expect("cone node");
        match &cone.node.data {
            SceneNodeData::SpotLight3D(light) => {
                assert_eq!(light.shadow_strength, 0.22);
                assert_eq!(light.shadow_depth_bias, 0.004);
                assert_eq!(light.shadow_normal_bias, 0.13);
            }
            other => panic!("expected SpotLight3D, got {other:?}"),
        }
    }

    #[test]
    fn sky3d_horizon_and_shader_stack_parse() {
        let scene = Parser::new(
            r#"
            $root = @sky
            [sky]
            [Sky3D]
                palette = {
                    day_colors = [(0.1, 0.2, 0.3), (0.4, 0.5, 0.6)]
                    evening_colors = [(0.7, 0.4, 0.2), (0.2, 0.1, 0.3)]
                    night_colors = [(0.0, 0.0, 0.1), (0.0, 0.0, 0.2)]
                    horizon_colors = [(0.6, 0.6, 0.6), (0.3, 0.3, 0.3)]
                }
                time = { time_of_day = 0.25 paused = true scale = 0.5 }
                shaders = [
                    { path = "res://shaders/sky.wgsl", params = [1.0, (1.0, 2.0, 3.0)] }
                ]
            [/Sky3D]
            [/sky]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let sky = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sky")
            .expect("sky node");

        match &sky.node.data {
            SceneNodeData::Sky3D(node) => {
                assert_eq!(node.palette.horizon_colors.len(), 2);
                assert_eq!(node.palette.day_colors[0], [0.1, 0.2, 0.3]);
                assert_eq!(node.time.time_of_day, 0.25);
                assert!(node.time.paused);
                assert_eq!(node.shaders.len(), 1);
                assert_eq!(node.shaders[0].path.as_ref(), "res://shaders/sky.wgsl");
                assert_eq!(node.shaders[0].params.len(), 2);
                assert_eq!(
                    node.shaders[0].params[0].value,
                    CustomPostParamValue::F32(1.0)
                );
                assert_eq!(
                    node.shaders[0].params[1].value,
                    CustomPostParamValue::Vec3([1.0, 2.0, 3.0])
                );
            }
            other => panic!("expected Sky3D, got {other:?}"),
        }
    }

    #[test]
    fn sky3d_old_fields_do_not_resolve() {
        for field in [
            "cloud_size",
            "cloud_shader",
            "star_size",
            "moon_size",
            "sun_size",
            "sky_shader",
            "sky_angle",
        ] {
            assert!(resolve_node_field("Sky3D", field).is_none(), "{field}");
        }
    }

    #[test]
    fn water_body_shape_fields_parse() {
        let scene = Parser::new(
            r#"
            $root = @lake2d
            [lake2d]
            [WaterBody2D]
                shape = { type="circle" radius=24 }
            [/WaterBody2D]
            [/lake2d]
            [tank3d]
            [WaterBody3D]
                shape = { type="cylinder" radius=16 half_height=5 }
            [/WaterBody3D]
            [/tank3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let lake = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "lake2d")
            .expect("lake node");
        let tank = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "tank3d")
            .expect("tank node");

        match &lake.node.data {
            SceneNodeData::WaterBody2D(node) => {
                assert_eq!(
                    node.water.shape,
                    perro_nodes::WaterShape::Circle { radius: 24.0 }
                );
            }
            other => panic!("expected WaterBody2D node, got {other:?}"),
        }
        match &tank.node.data {
            SceneNodeData::WaterBody3D(node) => {
                assert_eq!(node.water.depth, 10.0);
                assert_eq!(
                    node.water.shape,
                    perro_nodes::WaterShape::Cylinder {
                        radius: 16.0,
                        half_height: 5.0,
                    }
                );
            }
            other => panic!("expected WaterBody3D node, got {other:?}"),
        }
    }

    #[test]
    fn water_vertices_per_meter_derives_resolution_from_shape() {
        let scene = Parser::new(
            r#"
            $root = @water
            [water]
            [WaterBody3D]
                sim_cells_per_meter = 2
                render_vertices_per_meter = 4
                shape = { type="cube" size=(20, 8, 10) }
            [/WaterBody3D]
            [/water]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let water = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "water")
            .expect("water node");

        match &water.node.data {
            SceneNodeData::WaterBody3D(node) => {
                assert_eq!(node.water.resolution, [41, 21]);
                assert_eq!(node.water.render_resolution, [81, 41]);
            }
            other => panic!("expected WaterBody3D node, got {other:?}"),
        }
    }

    #[test]
    fn water_resolution_fields_set_sim_and_render_density() {
        let scene = Parser::new(
            r#"
            $root = @water
            [water]
            [WaterBody3D]
                shape = { type="cube" size=(20, 8, 10) }
                sim_cells_per_meter = 25
                render_vertices_per_meter = 50
            [/WaterBody3D]
            [/water]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let water = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "water")
            .expect("water node");

        match &water.node.data {
            SceneNodeData::WaterBody3D(node) => {
                assert_eq!(node.water.resolution, [501, 251]);
                assert_eq!(node.water.render_resolution, [1001, 501]);
            }
            other => panic!("expected WaterBody3D node, got {other:?}"),
        }
    }

    #[test]
    fn root_of_merges_root_defaults_overrides_and_children() {
        let host = Parser::new(
            r#"
            $root = @host
            [host]
            root_of = "res://base.scn"
            script_vars = {
                keep: 5,
                remove_me: __unset__,
                nested: { b: 20, c: 30 },
                added: true
            }
            [Node2D]
                rotation = 3.0
            [/Node2D]
            [/host]

            [local_child]
            parent = host
            [Node/]
            [/local_child]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            $root = @base_root
            [base_root]
            script = "res://base_script.rs"
            script_vars = {
                keep: 1,
                remove_me: 2,
                nested: { a: 10, b: 11 },
                old_only: 9
            }
            [Node2D]
                position = (1, 2)
                rotation = 1.0
            [/Node2D]
            [/base_root]

            [base_child]
            parent = base_root
            [Node/]
            [/base_child]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_script = prepared
            .scripts
            .iter()
            .find(|pending| pending.node_key_name == "host")
            .expect("host script");
        assert_eq!(
            host_script.script_path_hash,
            string_to_u64("res://base_script.rs")
        );

        let mut vars = BTreeMap::new();
        for (name, value) in &host_script.scene_injected_vars {
            vars.insert(name.as_str(), value);
        }
        assert!(vars.contains_key("keep"));
        assert!(vars.contains_key("added"));
        assert!(vars.contains_key("nested"));
        assert!(vars.contains_key("old_only"));
        assert!(!vars.contains_key("remove_me"));

        match vars.get("nested").expect("nested var") {
            SceneValue::Object(fields) => {
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "a"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "b"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "c"));
            }
            other => panic!("expected nested object, got {other:?}"),
        }

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 1.0);
                assert_eq!(node_2d.position.y, 2.0);
                assert_eq!(node_2d.rotation, 3.0);
            }
            other => panic!("expected Node2D host node, got {other:?}"),
        }

        assert!(
            prepared
                .nodes
                .iter()
                .any(|pending| pending.key_name == "base_child")
        );
        assert!(
            prepared
                .nodes
                .iter()
                .any(|pending| pending.key_name == "local_child")
        );
    }

    #[test]
    fn root_of_script_clear_prevents_inherited_script() {
        let host = Parser::new(
            r#"
            $root = @host
            [host]
            root_of = "res://base.scn"
            script = null
            [Node/]
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            $root = @base_root
            [base_root]
            script = "res://base_script.rs"
            [Node/]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        assert!(
            !prepared
                .scripts
                .iter()
                .any(|pending| pending.node_key_name == "host")
        );
    }

}
