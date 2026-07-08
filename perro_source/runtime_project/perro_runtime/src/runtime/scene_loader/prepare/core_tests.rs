#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::SignalID;
    use perro_nodes::SceneNodeData;
    use perro_scene::Parser;
    use perro_structs::{BitMask, Color, CustomPostParamValue, Vector2, Vector3};

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
                assert_eq!(
                    node.water.visual.foam_color,
                    Color::new(0.7, 0.9, 1.0, 1.0)
                );
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
    fn sky3d_horizon_and_shader_stack_parse() {
        let scene = Parser::new(
            r#"
            $root = @sky
            [sky]
            [Sky3D]
                day_colors = [(0.1, 0.2, 0.3), (0.4, 0.5, 0.6)]
                evening_colors = [(0.7, 0.4, 0.2), (0.2, 0.1, 0.3)]
                night_colors = [(0.0, 0.0, 0.1), (0.0, 0.0, 0.2)]
                horizon_colors = [(0.6, 0.6, 0.6), (0.3, 0.3, 0.3)]
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
                assert_eq!(node.horizon_colors.len(), 2);
                assert_eq!(node.time.time_of_day, 0.25);
                assert!(node.time.paused);
                assert_eq!(node.shaders.len(), 1);
                assert_eq!(node.shaders[0].path.as_ref(), "res://shaders/sky.wgsl");
                assert_eq!(node.shaders[0].params.len(), 2);
                assert_eq!(node.shaders[0].params[0].value, CustomPostParamValue::F32(1.0));
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

        assert!(prepared
            .nodes
            .iter()
            .any(|pending| pending.key_name == "base_child"));
        assert!(prepared
            .nodes
            .iter()
            .any(|pending| pending.key_name == "local_child"));
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

        assert!(!prepared
            .scripts
            .iter()
            .any(|pending| pending.node_key_name == "host"));
    }

    #[test]
    fn root_of_without_host_type_block_inherits_template_root_data() {
        let host = Parser::new(
            r#"
            $root = @host
            [host]
            root_of = "res://base.scn"
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            $root = @base_root
            [base_root]
            [Node2D]
                position = (7, 8)
                rotation = 1.25
            [/Node2D]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 7.0);
                assert_eq!(node_2d.position.y, 8.0);
                assert_eq!(node_2d.rotation, 1.25);
            }
            other => panic!("expected inherited Node2D host node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_rotation_deg_for_spatial_nodes() {
        let scene = Parser::new(
            r#"
            $root = @scene_root
            [scene_root]
            [Node2D]
                rotation_deg = 90
            [/Node2D]
            [/scene_root]

            [camera]
            [Camera3D]
                rotation_deg = (0, 90, 0)
            [/Camera3D]
            [/camera]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let root = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "scene_root")
            .expect("root node");
        match &root.node.data {
            SceneNodeData::Node2D(node) => {
                assert!((node.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
            }
            other => panic!("expected Node2D root node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => {
                assert!(
                    (node.transform.rotation.y.abs() - std::f32::consts::FRAC_1_SQRT_2).abs()
                        < 1e-5
                );
            }
            other => panic!("expected Camera3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_button_state_style_inherits_base_fields() {
        let scene = Parser::new(
            r##"
            $root = @button
            [button]
            [UiButton]
                style = {
                    fill = "#101820"
                    stroke = "#A0A8B0"
                    radius = 1.0
                    shadow = { color = "#00000066" distance = 8 falloff = 12 vector = (1, -1) size = 1.5 }
                    highlight = { color = "#FFFFFF55" distance = 2 falloff = 4 vector = (-1, 1) size = 1.0 }
                }
                hover_fill = "#202830"
                pressed = {
                    style = { fill = "#303840" }
                }
            [/UiButton]
            [/button]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &node.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.style.corner_radius(), 1.0);
                assert_eq!(
                    button.style.outer_shadow.color,
                    Color::from_hex("#00000066").unwrap()
                );
                assert_eq!(button.style.outer_shadow.distance, 8.0);
                assert_eq!(button.style.outer_shadow.falloff, 12.0);
                assert_eq!(button.style.outer_shadow.vector, Vector2::new(1.0, -1.0));
                assert_eq!(button.style.outer_shadow.size, 1.5);
                assert_eq!(
                    button.style.inner_highlight.color,
                    Color::from_hex("#FFFFFF55").unwrap()
                );
                assert_eq!(button.style.inner_highlight.distance, 2.0);
                assert_eq!(button.style.inner_highlight.falloff, 4.0);
                assert_eq!(button.style.inner_highlight.vector, Vector2::new(-1.0, 1.0));
                assert_eq!(button.style.inner_highlight.size, 1.0);
                assert_eq!(button.hover_style.fill, Color::from_hex("#202830").unwrap());
                assert_eq!(button.hover_style.stroke, button.style.stroke);
                assert_eq!(button.hover_style.corner_radius(), 1.0);
                assert_eq!(
                    button.pressed_style.fill,
                    Color::from_hex("#303840").unwrap()
                );
                assert_eq!(button.pressed_style.stroke, button.style.stroke);
                assert_eq!(button.pressed_style.corner_radius(), 1.0);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_input_masks_apply_to_button_and_text_edit() {
        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                input_only_players = [0, 2]
                input_block_gamepads = [1]
                input_allow_kbm = true
            [/UiButton]
            [/button]

            [field]
            [UiTextBox]
                input_only_joycons = [3]
                input_block_players = [4]
                input_deny_kbm = true
                h_align = "center"
                v_align = "end"
            [/UiTextBox]
            [/field]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.input_mask.allow_players, vec![0, 2]);
                assert_eq!(button.input_mask.deny_gamepads, vec![1]);
                assert!(button.input_mask.allow_kbm);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }

        let field = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "field")
            .expect("field node");
        match &field.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(text_box.inner.input_mask.allow_joycons, vec![3]);
                assert_eq!(text_box.inner.input_mask.deny_players, vec![4]);
                assert!(text_box.inner.input_mask.deny_kbm);
                assert_eq!(text_box.inner.h_align, perro_ui::UiTextAlign::Center);
                assert_eq!(text_box.inner.v_align, perro_ui::UiTextAlign::End);
            }
            other => panic!("expected UiTextBox node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_button_web_href_parses() {
        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                web = { href = "docs/" }
            [/UiButton]
            [/button]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                let web = button.web.as_ref().expect("web config");
                assert_eq!(web.href.as_ref(), "/docs");
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_style_resource_applies_base_state_and_focus() {
        static BASE: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.10, 0.20, 0.30, 1.0),
            stroke: Color::new(0.40, 0.50, 0.60, 1.0),
            stroke_width: 2.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.4),
            ..perro_ui::UiStyle::panel()
        };
        static HOVER: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.20, 0.30, 0.40, 1.0),
            stroke: Color::new(0.50, 0.60, 0.70, 1.0),
            stroke_width: 3.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.5),
            ..perro_ui::UiStyle::panel()
        };
        static PRESSED: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.05, 0.10, 0.15, 1.0),
            stroke: Color::new(0.30, 0.40, 0.50, 1.0),
            stroke_width: 4.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.6),
            ..perro_ui::UiStyle::panel()
        };
        static FOCUS: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.70, 0.80, 0.90, 1.0),
            stroke: Color::new(0.10, 0.20, 0.30, 1.0),
            stroke_width: 5.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.7),
            ..perro_ui::UiStyle::panel()
        };
        static EMPTY: perro_ui::UiStyle = perro_ui::UiStyle::panel();

        fn lookup(path_hash: u64) -> &'static perro_ui::UiStyle {
            match path_hash {
                hash if hash == perro_ids::string_to_u64("res://ui/base.uistyle") => &BASE,
                hash if hash == perro_ids::string_to_u64("res://ui/hover.uistyle") => &HOVER,
                hash if hash == perro_ids::string_to_u64("res://ui/pressed.uistyle") => &PRESSED,
                hash if hash == perro_ids::string_to_u64("res://ui/focus.uistyle") => &FOCUS,
                _ => &EMPTY,
            }
        }

        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                style = "res://ui/base.uistyle"
                hover = { style = "res://ui/hover.uistyle" }
                pressed = { style = "res://ui/pressed.uistyle" }
            [/UiButton]
            [/button]

            [input]
            parent = button
            [UiTextBox]
                style = "res://ui/base.uistyle"
                focused_style = "res://ui/focus.uistyle"
            [/UiTextBox]
            [/input]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader_and_styles(
            &scene,
            &|path| Err(format!("unknown scene path `{path}`")),
            Some(lookup),
        )
        .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.style.fill, BASE.fill);
                assert_eq!(button.hover_style.fill, HOVER.fill);
                assert_eq!(button.pressed_style.fill, PRESSED.fill);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }

        let input = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "input")
            .expect("input node");
        match &input.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(text_box.style.fill, BASE.fill);
                assert_eq!(text_box.focused_style.fill, FOCUS.fill);
            }
            other => panic!("expected UiTextBox node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_style_parses_gradient_and_split_depth_fields() {
        let scene = Parser::new(
            r##"
            $root = @panel
            [panel]
            [UiPanel]
                style = {
                    fill_kind = "linear"
                    gradient = { start_color = "#445566" end_color = "#112233" vector = (0, -1) }
                    corner_radii = (0.1, 0.2, 0.3, 0.4)
                    outer_shadow = { color = "#00000088" distance = 6 falloff = 9 vector = (1, -1) size = 1.2 }
                    inner_shadow = { color = "#00000044" distance = 2 falloff = 4 vector = (0, -1) size = 1.0 }
                    outer_highlight = { color = "#FFFFFF22" distance = 1 falloff = 3 vector = (-1, 1) size = 1.0 }
                    inner_highlight = { color = "#FFFFFF33" distance = 1 falloff = 2 vector = (-1, 1) size = 1.0 }
                }
            [/UiPanel]
            [/panel]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "panel")
            .expect("panel node");
        match &node.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(panel.style.fill_kind, perro_ui::UiFillKind::Linear);
                assert_eq!(
                    panel.style.gradient.start_color,
                    Color::from_hex("#445566").unwrap()
                );
                assert_eq!(panel.style.corner_radii.tl, 0.1);
                assert_eq!(panel.style.corner_radii.tr, 0.2);
                assert_eq!(panel.style.corner_radii.br, 0.3);
                assert_eq!(panel.style.corner_radii.bl, 0.4);
                assert_eq!(panel.style.outer_shadow.distance, 6.0);
                assert_eq!(panel.style.inner_shadow.distance, 2.0);
                assert_eq!(panel.style.outer_highlight.falloff, 3.0);
                assert_eq!(panel.style.inner_highlight.falloff, 2.0);
            }
            other => panic!("expected UiPanel node, got {other:?}"),
        }
    }

    #[test]
    fn ui_nodes_ignore_absolute_pixel_size() {
        let scene = Parser::new(
            r#"
            [button]
            [UiButton]
                size = (160, 48)
                size_px = (170, 50)
                pixel_size = (180, 52)
                min_size = (120, 40)
                max_size = (240, 96)
                min_width = 110
                min_height = 32
                max_width = 220
                max_height = 80
            [/UiButton]
            [/button]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("scene prepare");
        let button = prepared
            .nodes
            .iter()
            .find(|pending| matches!(pending.node.data, SceneNodeData::UiButton(_)))
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.layout.size, perro_ui::UiVector2::ZERO);
                assert_eq!(button.layout.min_size, Vector2::ZERO);
                assert_eq!(
                    button.layout.max_size,
                    perro_ui::UiLayoutData::NO_MAX_SIZE
                );
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ui_nodes_from_scene_blocks() {
        let scene = Parser::new(
            r##"
            $root = @menu
            [menu]
            [UiButton]
                visible = false
                input_enabled = false
                mouse_filter = "pass"
                clip_children = true
                anchor = "tr"
                position_ratio = (0.5, 0.25)
                size_ratio = (0.5, 0.1)
                scale = (2, 0.5)
                rotation = 0.25
                h_size = "fill"
                v_size = "fit_children"
                pivot_ratio = (0, 0)
                padding = (0.1, 0.2, 0.3, 0.4)
                style = { fill = "#101820" stroke = "#A0A8B0" radius = 0.3 }
                hover_fill = "#202830"
                cursor_icon = "grab"
                pressed_fill = "#303840"
                hover_signals = ["ui_hover"]
                pressed_signals = ["ui_down", "ui_press_any"]
                clicked_signals = ["ui_click"]
                hover = {
                    size_ratio = (0.65, 0.08666667)
                    scale = (1.1, 1.2)
                    rotation = 0.5
                    style = { fill = "#405060" stroke = "#C0D0E0" radius = 0.4 }
                }
                pressed = {
                    size_ratio = (0.55, 0.07)
                    scale = (0.9, 0.8)
                    rotation = -0.25
                    style = { fill = "#182028" stroke = "#8090A0" radius = 0.2 }
                }
                radius = "full"
                disabled = true
            [/UiButton]
            [/menu]

            [items]
            parent = menu
            [UiGrid]
                columns = 3
                h_spacing = 8
                v_spacing = "fill"
            [/UiGrid]
            [/items]

            [generic]
            parent = menu
            [UiLayout]
                mode = "grid"
                columns = 2
                spacing = 4
            [/UiLayout]
            [/generic]

            [forced_h]
            parent = menu
            [UiHLayout]
                mode = "v"
                spacing = "fill"
            [/UiHLayout]
            [/forced_h]

            [forced_v]
            parent = menu
            [UiVLayout]
                mode = "grid"
            [/UiVLayout]
            [/forced_v]

            [defaults]
            parent = menu
            [UiPanel/]
            [/defaults]

            [entry]
            parent = menu
            [UiTextBox]
                hover_signals = ["entry_hover"]
                hover_exit_signals = ["entry_unhover"]
                focused_signals = ["entry_focus"]
                unfocused_signals = ["entry_unfocus"]
                text_changed_signals = ["entry_text"]
                input_type = "f32"
            [/UiTextBox]
            [/entry]

            [scroller]
            parent = menu
            [UiScrollContainer]
                scroll = (12, 34)
                scroll_dir = "horizontal"
                scrollbar_side = "left"
                scrollbar_padding = 9
            [/UiScrollContainer]
            [/scroller]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let menu = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "menu")
            .expect("menu node");
        match &menu.node.data {
            SceneNodeData::UiButton(button) => {
                assert!(!button.visible);
                assert!(!button.input_enabled);
                assert_eq!(button.mouse_filter, UiMouseFilter::Pass);
                assert!(button.clip_children);
                assert_eq!(button.layout.anchor, perro_ui::UiAnchor::TopRight);
                assert!(button.disabled);
                assert_eq!(button.style.corner_radius(), 0.3);
                assert_eq!(button.style.fill, Color::from_hex("#101820").unwrap());
                assert_eq!(button.style.stroke, Color::from_hex("#A0A8B0").unwrap());
                assert_eq!(button.hover_style.fill, Color::from_hex("#405060").unwrap());
                assert_eq!(
                    button.hover_style.stroke,
                    Color::from_hex("#C0D0E0").unwrap()
                );
                assert_eq!(button.hover_style.corner_radius(), 0.4);
                assert_eq!(button.cursor_icon, perro_ui::CursorIcon::Grab);
                assert_eq!(
                    button.pressed_style.fill,
                    Color::from_hex("#182028").unwrap()
                );
                assert_eq!(
                    button.hover_signals,
                    vec![perro_ids::SignalID::from_string("ui_hover")]
                );
                assert_eq!(
                    button.pressed_signals,
                    vec![
                        perro_ids::SignalID::from_string("ui_down"),
                        perro_ids::SignalID::from_string("ui_press_any"),
                    ]
                );
                assert_eq!(
                    button.clicked_signals,
                    vec![perro_ids::SignalID::from_string("ui_click")]
                );
                let hover = button.hover_base.as_ref().expect("hover base");
                assert_eq!(
                    hover.layout.size,
                    perro_ui::UiVector2::ratio(0.65, 0.08666667)
                );
                assert_eq!(hover.transform.scale, Vector2::new(1.1, 1.2));
                assert_eq!(hover.transform.rotation, 0.5);
                assert!(button.hover_size_override);
                let pressed = button.pressed_base.as_ref().expect("pressed base");
                assert_eq!(pressed.layout.size, perro_ui::UiVector2::ratio(0.55, 0.07));
                assert_eq!(pressed.transform.scale, Vector2::new(0.9, 0.8));
                assert_eq!(pressed.transform.rotation, -0.25);
                assert!(button.pressed_size_override);
                assert_eq!(button.transform.scale, Vector2::new(2.0, 0.5));
                assert_eq!(button.transform.rotation, 0.25);
                assert_eq!(button.layout.h_size, perro_ui::UiSizeMode::Fill);
                assert_eq!(button.layout.v_size, perro_ui::UiSizeMode::FitChildren);
                assert_eq!(
                    button.layout.padding,
                    perro_ui::UiRect::new(0.1, 0.2, 0.3, 0.4)
                );
                match button.transform.position.x {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 50.0),
                    other => panic!("expected percent x, got {other:?}"),
                }
                match button.transform.position.y {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 50.0),
                    other => panic!("expected percent y, got {other:?}"),
                }
                match button.transform.pivot.x {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 0.0),
                    other => panic!("expected percent pivot x, got {other:?}"),
                }
            }
            other => panic!("expected UiButton menu node, got {other:?}"),
        }

        let items = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "items")
            .expect("items node");
        match &items.node.data {
            SceneNodeData::UiGrid(grid) => {
                assert_eq!(grid.columns, 3);
                assert_eq!(grid.h_spacing, 8.0);
                assert_eq!(grid.v_spacing_mode, perro_ui::UiLayoutSpacingMode::Fill);
            }
            other => panic!("expected UiGrid items node, got {other:?}"),
        }

        let generic = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "generic")
            .expect("generic node");
        match &generic.node.data {
            SceneNodeData::UiLayout(layout) => {
                assert_eq!(layout.inner.mode, perro_ui::UiLayoutMode::Grid);
                assert_eq!(layout.inner.columns, 2);
                assert_eq!(layout.inner.spacing, 4.0);
            }
            other => panic!("expected UiLayout generic node, got {other:?}"),
        }

        let forced_h = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "forced_h")
            .expect("forced_h node");
        match &forced_h.node.data {
            SceneNodeData::UiHLayout(layout) => {
                assert_eq!(layout.mode(), perro_ui::UiLayoutMode::H);
                assert_eq!(
                    layout.inner.spacing_mode,
                    perro_ui::UiLayoutSpacingMode::Fill
                );
            }
            other => panic!("expected UiHLayout forced_h node, got {other:?}"),
        }

        let forced_v = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "forced_v")
            .expect("forced_v node");
        match &forced_v.node.data {
            SceneNodeData::UiVLayout(layout) => {
                assert_eq!(layout.mode(), perro_ui::UiLayoutMode::V);
            }
            other => panic!("expected UiVLayout forced_v node, got {other:?}"),
        }

        let defaults = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "defaults")
            .expect("defaults node");
        match &defaults.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(panel.layout.anchor, perro_ui::UiAnchor::Center);
                assert_eq!(
                    panel.transform.position,
                    perro_ui::UiVector2::ratio(0.5, 0.5)
                );
                assert_eq!(panel.layout.h_align, perro_ui::UiHorizontalAlign::Center);
                assert_eq!(panel.layout.v_align, perro_ui::UiVerticalAlign::Center);
            }
            other => panic!("expected UiPanel defaults node, got {other:?}"),
        }

        let entry = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "entry")
            .expect("entry node");
        match &entry.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(
                    text_box.inner.hover_signals,
                    vec![perro_ids::SignalID::from_string("entry_hover")]
                );
                assert_eq!(
                    text_box.inner.hover_exit_signals,
                    vec![perro_ids::SignalID::from_string("entry_unhover")]
                );
                assert_eq!(
                    text_box.inner.focused_signals,
                    vec![perro_ids::SignalID::from_string("entry_focus")]
                );
                assert_eq!(
                    text_box.inner.unfocused_signals,
                    vec![perro_ids::SignalID::from_string("entry_unfocus")]
                );
                assert_eq!(
                    text_box.inner.text_changed_signals,
                    vec![perro_ids::SignalID::from_string("entry_text")]
                );
                assert_eq!(
                    text_box.inner.input_type,
                    perro_ui::UiTextInputType::SignedFloat
                );
            }
            other => panic!("expected UiTextBox entry node, got {other:?}"),
        }

        let scroller = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "scroller")
            .expect("scroller node");
        match &scroller.node.data {
            SceneNodeData::UiScrollContainer(scroller) => {
                assert!(scroller.clip_children);
                assert_eq!(scroller.scroll, Vector2::new(12.0, 34.0));
                assert_eq!(
                    scroller.scroll_dir,
                    perro_ui::UiScrollDirection::Horizontal
                );
                assert_eq!(scroller.scroll_bar_side, perro_ui::UiScrollBarSide::Left);
                assert_eq!(scroller.scroll_bar_padding, 9.0);
            }
            other => panic!("expected UiScrollContainer scroller node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ignores_ui_position_ratio_and_uses_translation_ratios() {
        let scene = Parser::new(
            r#"
            $root = @panel
            [panel]
            [UiPanel]
                position_ratio = (0.5, 0.98)
                position_percent = (20, 80)
                translation_ratio = (0.25, -0.5)
                self_translation_ratio = (1.0, 0.0)
            [/UiPanel]
            [/panel]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let panel = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "panel")
            .expect("panel node");

        match &panel.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(panel.transform.position, perro_ui::UiVector2::ratio(0.5, 0.5));
                assert_eq!(panel.transform.translation, Vector2::new(0.25, -0.5));
                assert_eq!(panel.transform.self_translation, Vector2::new(1.0, 0.0));
            }
            other => panic!("expected UiPanel node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ik_target_3d_fields_and_skeleton_link() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [HandTarget]
            [IKTarget3D]
                skeleton = @Rig
                bone = 5
                chain_length = 3
                iterations = 12
                tolerance = 0.05
                weight = 0.75
                match_rotation = false
            [/IKTarget3D]
            [/HandTarget]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let target = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "HandTarget")
            .expect("ik target node");
        match &target.node.data {
            SceneNodeData::IKTarget3D(ik) => {
                assert_eq!(ik.params.bone_index, 5);
                assert_eq!(ik.params.chain_length, 3);
                assert_eq!(ik.params.iterations, 12);
                assert_eq!(ik.params.tolerance, 0.05);
                assert_eq!(ik.params.weight, 0.75);
                assert!(!ik.params.match_rotation);
            }
            other => panic!("expected IKTarget3D node, got {other:?}"),
        }
        assert!(target.ik_target_skeleton_target.is_some());
    }

    #[test]
    fn scene_loader_parses_physics_bone_chain_iters_alias() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [Tail2D]
            [PhysicsBoneChain2D]
                skeleton = @Rig
                bone = 4
                iters = 2
            [/PhysicsBoneChain2D]
            [/Tail2D]

            [Tail3D]
            [PhysicsBoneChain3D]
                skeleton = @Rig
                bone = 5
                iters = 4
            [/PhysicsBoneChain3D]
            [/Tail3D]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let tail_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Tail2D")
            .expect("2d physics chain");
        match &tail_2d.node.data {
            SceneNodeData::PhysicsBoneChain2D(chain) => assert_eq!(chain.iterations, 2),
            other => panic!("expected PhysicsBoneChain2D node, got {other:?}"),
        }

        let tail_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Tail3D")
            .expect("3d physics chain");
        match &tail_3d.node.data {
            SceneNodeData::PhysicsBoneChain3D(chain) => assert_eq!(chain.iterations, 4),
            other => panic!("expected PhysicsBoneChain3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_rejects_bone_2d_node() {
        let err = std::panic::catch_unwind(|| {
            Parser::new(
            r#"
            $root = @Rig2D
            [Rig2D]
            [Skeleton2D]
                position = (10, 20)
            [/Skeleton2D]
            [/Rig2D]

            [UpperArm]
            parent = @Rig2D
            [Bone2D]
                position = (4, 5)
                rotation = 0.25
                scale = (1, 1)
                rest = { position = (4, 5), rotation = 0.25, scale = (1, 1) }
                pose = { position = (6, 7), rotation = 0.5, scale = (1, 1) }
            [/Bone2D]
            [/UpperArm]
            "#,
            )
            .parse_scene()
        })
        .expect_err("expected bone2d scene node rejection");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(msg.contains("unsupported scene node type `Bone2D`"));
    }

    #[test]
    fn scene_loader_builds_skeleton_2d_mirror_nodes() {
        let scene = Parser::new(
            r#"
            $root = @Rig2D
            [Rig2D]
            [Skeleton2D]
                position = (10, 20)
                skeleton = "res://rig.pskel2d"
            [/Skeleton2D]
            [/Rig2D]

            [Hand]
            parent = @Rig2D
            [BoneAttachment2D]
                skeleton = @Rig2D
                bone = 1
            [/BoneAttachment2D]
            [/Hand]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let rig = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Rig2D")
            .expect("rig node");
        assert!(matches!(rig.node.data, SceneNodeData::Skeleton2D(_)));
        assert_eq!(rig.skeleton_source.as_deref(), Some("res://rig.pskel2d"));

        let hand = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Hand")
            .expect("hand node");
        match &hand.node.data {
            SceneNodeData::BoneAttachment2D(node) => {
                assert_eq!(node.bone_index, 1);
                assert!(hand.bone_attachment_skeleton_target.is_some());
            }
            other => panic!("expected BoneAttachment2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_rejects_quoted_skeleton_node_refs() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [Mesh]
            [MeshInstance3D]
                skeleton = "Rig"
            [/MeshInstance3D]
            [/Mesh]
            "#,
        )
        .parse_scene();

        let err = match prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        }) {
            Ok(_) => panic!("expected quoted skeleton node ref rejection"),
            Err(err) => err,
        };
        assert!(err.contains("MeshInstance3D.skeleton must be a node ref like @SkeletonNode"));
    }

    #[test]
    fn scene_loader_parses_multimesh_instance_grid() {
        let scene = Parser::new(
            r#"
            $root = @Batch
            [Batch]
            [MultiMeshInstance3D]
                instance_grid = { counts=(4, 3, 2) spacing=(2, 1.5, 3) origin=(-3, 0.5, -4) scale=(1.5, 0.75, 2.0) height_wave=0.0 rotation_step_deg=(0, 10, 0) }
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.instances.len(), 24);
                assert_eq!(
                    mesh.instances[0].transform.position,
                    Vector3::new(-3.0, 0.5, -4.0)
                );
                assert_eq!(
                    mesh.instances[23].transform.position,
                    Vector3::new(3.0, 3.5, -1.0)
                );
                assert_eq!(
                    mesh.instances[0].transform.scale,
                    Vector3::new(1.5, 0.75, 2.0)
                );
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_mesh_lod_options() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                min_lod = 1
                max_lod = 3
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                lod_min = 2
                lod_max = 4
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert_eq!(mesh.lod.min_lod, 1);
                assert_eq!(mesh.lod.max_lod, 3);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.lod.min_lod, 2);
                assert_eq!(mesh.lod.max_lod, 4);
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_blend_shape_weights_and_aliases() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                shape_key_weights = [-1.0, 0.5, 2.0]
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                morph_weights = [0.2, 1.5]
                instances = [
                    { position=[1.0, 0.0, 0.0] blend_shape_weights=[0.7, 0.4] },
                    { position=[2.0, 0.0, 0.0] }
                ]
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert_eq!(mesh.blend_shape_weights, vec![0.0, 0.5, 1.0]);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.blend_shape_weights, vec![0.2, 1.0]);
                assert_eq!(
                    mesh.instances[0].blend_shape_weights.as_deref(),
                    Some([0.7, 0.4].as_slice())
                );
                assert!(mesh.instances[1].blend_shape_weights.is_none());
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_mesh_blend_options() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                cast_shadows = false
                receive_shadows = false
                blend = { enabled=true screen_blending=false normal_blending=true blend_layers=[2, 4] blend_mask=[1, 3] distance=0.5 min_distance=0.05 noise=0.25 noise_scale=6.0 }
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                cast_shadows = false
                receive_shadows = false
                blend_enabled = true
                blend_screen = false
                blend_normals = true
                blend_layers = [5]
                blend_mask = none
                blend_distance = 0.25
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert!(!mesh.cast_shadows);
                assert!(!mesh.receive_shadows);
                assert!(mesh.blend.enabled);
                assert!(!mesh.blend.screen_blending);
                assert!(mesh.blend.normal_blending);
                assert_eq!(mesh.blend.blend_layers, BitMask::with([2, 4]));
                assert_eq!(mesh.blend.blend_mask, BitMask::with([1, 3]));
                assert_eq!(mesh.blend.distance, 0.5);
                assert_eq!(mesh.blend.min_distance, 0.05);
                assert_eq!(mesh.blend.noise_factor, 0.25);
                assert_eq!(mesh.blend.noise_scale, 6.0);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert!(!mesh.cast_shadows);
                assert!(!mesh.receive_shadows);
                assert!(mesh.blend.enabled);
                assert!(!mesh.blend.screen_blending);
                assert!(mesh.blend.normal_blending);
                assert_eq!(mesh.blend.blend_layers, BitMask::with([5]));
                assert_eq!(mesh.blend.blend_mask, BitMask::NONE);
                assert_eq!(mesh.blend.distance, 0.25);
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_defaults_mesh_normal_blending_false() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
            [/MeshInstance3D]
            [/Mesh]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert!(!mesh.blend.normal_blending);
                assert!(mesh.blend.screen_blending);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_locale_text_markers() {
        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = "%loc:\"ui.center\""
            [/UiLabel]
            [/label]

            [box]
            [UiTextBox]
                text = %loc: "ui.entry"
                placeholder = "%loc:\"ui.placeholder\""
            [/UiTextBox]
            [/box]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let label = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label")
            .expect("label node");
        assert_eq!(label.locale_text_bindings.len(), 1);
        assert_eq!(label.locale_text_bindings[0].key, "ui.center");
        assert_eq!(
            label.locale_text_bindings[0].field,
            crate::runtime::state::LocaleTextField::LabelText
        );
        match &label.node.data {
            SceneNodeData::UiLabel(label) => assert_eq!(label.text.as_ref(), "ui.center"),
            other => panic!("expected UiLabel node, got {other:?}"),
        }

        let text_box = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "box")
            .expect("box node");
        assert_eq!(text_box.locale_text_bindings.len(), 2);
        assert!(text_box
            .locale_text_bindings
            .iter()
            .any(|binding| binding.key == "ui.entry"
                && binding.field == crate::runtime::state::LocaleTextField::TextEditText));
        assert!(text_box
            .locale_text_bindings
            .iter()
            .any(|binding| binding.key == "ui.placeholder"
                && binding.field == crate::runtime::state::LocaleTextField::TextEditPlaceholder));
    }

    #[test]
    fn scene_loader_parses_world_label_locale_text_markers() {
        let scene = Parser::new(
            r#"
            [label_2d]
            [Label2D]
                text = "%loc:\"ui.hp\""
            [/Label2D]
            [/label_2d]

            [label_3d]
            [Label3D]
                text = %loc: "ui.name"
            [/Label3D]
            [/label_3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let label_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label_2d")
            .expect("label2d node");
        assert_eq!(label_2d.locale_text_bindings.len(), 1);
        assert_eq!(label_2d.locale_text_bindings[0].key, "ui.hp");
        match &label_2d.node.data {
            SceneNodeData::Label2D(label) => assert_eq!(label.text.as_ref(), "ui.hp"),
            other => panic!("expected Label2D node, got {other:?}"),
        }

        let label_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label_3d")
            .expect("label3d node");
        assert_eq!(label_3d.locale_text_bindings.len(), 1);
        assert_eq!(label_3d.locale_text_bindings[0].key, "ui.name");
        match &label_3d.node.data {
            SceneNodeData::Label3D(label) => assert_eq!(label.text.as_ref(), "ui.name"),
            other => panic!("expected Label3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_escapes_locale_text_marker_prefix() {
        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = "%%loc:not_key"
            [/UiLabel]
            [/label]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let label = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label")
            .expect("label node");
        assert!(label.locale_text_bindings.is_empty());
        match &label.node.data {
            SceneNodeData::UiLabel(label) => assert_eq!(label.text.as_ref(), "%loc:not_key"),
            other => panic!("expected UiLabel node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_animated_sprite_2d_animations() {
        let scene = Parser::new(
            r#"
            $root = @hero
            [hero]
            [AnimatedSprite2D]
                texture = "res://hero.png"
                current_animation = "run"
                current_frame = 1
                fps_scale = 1.5
                animations = [
                    { name = "idle", start = (0, 0), frame_size = (32, 32), frame_count = 4, fps = 8 },
                    { name = "run", start = (0, 32), frame_size = (32, 32), frame_count = 6, fps = 12 }
                ]
            [/AnimatedSprite2D]
            [/hero]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let hero = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "hero")
            .expect("hero node");
        assert_eq!(hero.texture_source.as_deref(), Some("res://hero.png"));
        match &hero.node.data {
            SceneNodeData::AnimatedSprite2D(sprite) => {
                assert_eq!(sprite.current_animation.as_ref(), "run");
                assert_eq!(sprite.current_frame, 1);
                assert_eq!(sprite.fps_scale, 1.5);
                assert_eq!(sprite.animations.len(), 2);
                assert_eq!(sprite.animations[1].name.as_ref(), "run");
                assert_eq!(sprite.animations[1].frame_count, 6);
                assert_eq!(
                    sprite.current_texture_region(),
                    Some([32.0, 32.0, 32.0, 32.0])
                );
            }
            other => panic!("expected AnimatedSprite2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_joint_body_links() {
        let scene = Parser::new(
            r#"
            [AnchorBody]
                [RigidBody2D/]
            [/AnchorBody]

            [SwingBody]
                [RigidBody2D/]
            [/SwingBody]

            [Link]
                [FixedJoint2D]
                    body_a = @AnchorBody
                    body_b = @SwingBody
                    anchor_a = (0, 0)
                    anchor_b = (0, 1)
                [/FixedJoint2D]
            [/Link]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|_| unreachable!()).unwrap();
        let link = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Link")
            .expect("joint node");

        assert_eq!(link.joint_body_links.len(), 2);
        assert!(link.joint_body_links.iter().any(|body| {
            body.field == PendingJointBodyField::BodyA
                && scene.key_name(SceneKey::new(body.target_key)) == Some("AnchorBody")
        }));
        assert!(link.joint_body_links.iter().any(|body| {
            body.field == PendingJointBodyField::BodyB
                && scene.key_name(SceneKey::new(body.target_key)) == Some("SwingBody")
        }));
    }

    #[test]
    fn scene_loader_builds_rigid_body_gravity_scale() {
        let scene = Parser::new(
            r#"
            [Body2D]
                [RigidBody2D]
                    gravity_scale = 0.5
                [/RigidBody2D]
            [/Body2D]

            [Body3D]
                [RigidBody3D]
                    gravity_scale = 0.25
                [/RigidBody3D]
            [/Body3D]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|_| unreachable!()).unwrap();
        let body_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Body2D")
            .expect("body2d node");
        let body_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Body3D")
            .expect("body3d node");

        match &body_2d.node.data {
            SceneNodeData::RigidBody2D(body) => assert_eq!(body.gravity_scale, 0.5),
            other => panic!("expected RigidBody2D node, got {other:?}"),
        }
        match &body_3d.node.data {
            SceneNodeData::RigidBody3D(body) => assert_eq!(body.gravity_scale, 0.25),
            other => panic!("expected RigidBody3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_audio_effect_zone_effect_fields() {
        let scene = Parser::new(
            r#"
            [zone2d]
            [AudioEffectZone2D]
                enabled = false
                audio_mask = [2, 4]
                bounce = true
                effects = [
                    {
                        reverb_send: 0.8,
                        echo: 0.4,
                        dampening: 0.2
                    },
                    {
                        reverb_send: 0.3,
                        echo: 0.1,
                        dampening: 0.7
                    }
                ]
            [/AudioEffectZone2D]
            [/zone2d]

            [zone3d]
            [AudioEffectZone3D]
                reverb = 0.6
                echo = 0.3
                low_pass = 0.5
                bounce = true
            [/AudioEffectZone3D]
            [/zone3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let zone2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "zone2d")
            .expect("zone2d");
        let SceneNodeData::AudioEffectZone2D(zone2d) = &zone2d.node.data else {
            panic!("expected AudioEffectZone2D");
        };
        assert!(!zone2d.enabled);
        assert_eq!(zone2d.audio_mask.bits(), 0b1010);
        assert!(zone2d.bounce);
        assert_eq!(zone2d.effects.len(), 2);
        assert_eq!(zone2d.effects[0].reverb_send, 0.8);
        assert_eq!(zone2d.effects[0].echo, 0.4);
        assert_eq!(zone2d.effects[0].dampening, 0.2);
        assert_eq!(zone2d.effects[1].reverb_send, 0.3);
        assert_eq!(zone2d.effects[1].echo, 0.1);
        assert_eq!(zone2d.effects[1].dampening, 0.7);

        let zone3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "zone3d")
            .expect("zone3d");
        let SceneNodeData::AudioEffectZone3D(zone3d) = &zone3d.node.data else {
            panic!("expected AudioEffectZone3D");
        };
        assert!(zone3d.bounce);
        assert_eq!(zone3d.effects[0].reverb_send, 0.6);
        assert_eq!(zone3d.effects[0].echo, 0.3);
        assert_eq!(zone3d.effects[0].dampening, 0.5);
    }

    #[test]
    fn scene_loader_builds_audio_portal_link_fields() {
        let scene = Parser::new(
            r#"
            [portal2d]
            [AudioPortal2D]
                enabled = false
                strength = 0.55
                targets = [8, 9, 10]
            [/AudioPortal2D]
            [/portal2d]

            [portal3d]
            [AudioPortal3D]
                strength = 0.75
                connections = [11, 12]
            [/AudioPortal3D]
            [/portal3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let portal2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "portal2d")
            .expect("portal2d");
        let SceneNodeData::AudioPortal2D(portal2d) = &portal2d.node.data else {
            panic!("expected AudioPortal2D");
        };
        assert!(!portal2d.enabled);
        assert_eq!(portal2d.strength, 0.55);
        assert_eq!(
            portal2d.targets,
            vec![
                NodeID::from_u32(8),
                NodeID::from_u32(9),
                NodeID::from_u32(10)
            ]
        );

        let portal3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "portal3d")
            .expect("portal3d");
        let SceneNodeData::AudioPortal3D(portal3d) = &portal3d.node.data else {
            panic!("expected AudioPortal3D");
        };
        assert_eq!(portal3d.strength, 0.75);
        assert_eq!(
            portal3d.targets,
            vec![NodeID::from_u32(11), NodeID::from_u32(12)]
        );
    }

    #[test]
    fn scene_loader_builds_color_grade_and_luts() {
        let scene = Parser::new(
            r#"
            [cam]
            [Camera3D]
                active = true
                post_processing = [
                    {
                        type = "color_grade",
                        exposure = 0.25,
                        contrast = 1.1,
                        brightness = 0.02,
                        saturation = 1.2,
                        gamma = 0.95,
                        temperature = 0.1,
                        tint = -0.05,
                        hue_shift = 0.2,
                        vibrance = 0.3,
                        lift = (0.01, 0.02, 0.03),
                        gain = (1.1, 1.0, 0.9),
                        offset = (-0.01, 0.0, 0.01)
                    },
                    { type = "lut2d", texture = "res://luts/film_32.png", lut_size = 32, strength = 0.75 },
                    { type = "lut3d", texture = "res://luts/print_32.png", lut_size = 16, strength = 1.0 }
                ]
            [/Camera3D]
            [/cam]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let cam = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam")
            .expect("cam");
        let SceneNodeData::Camera3D(cam) = &cam.node.data else {
            panic!("expected Camera3D");
        };
        let effects = cam.post_processing.to_effects_vec();
        assert_eq!(effects.len(), 3);
        match &effects[0] {
            PostProcessEffect::ColorGrade {
                exposure,
                gain,
                offset,
                ..
            } => {
                assert_eq!(*exposure, 0.25);
                assert_eq!(*gain, [1.1, 1.0, 0.9]);
                assert_eq!(*offset, [-0.01, 0.0, 0.01]);
            }
            other => panic!("expected color grade, got {other:?}"),
        }
        match &effects[1] {
            PostProcessEffect::Lut2D {
                texture_path,
                size,
                strength,
            } => {
                assert_eq!(texture_path.as_ref(), "res://luts/film_32.png");
                assert_eq!(*size, 32);
                assert_eq!(*strength, 0.75);
            }
            other => panic!("expected lut2d, got {other:?}"),
        }
        match &effects[2] {
            PostProcessEffect::Lut3D {
                texture_path,
                size,
                strength,
            } => {
                assert_eq!(texture_path.as_ref(), "res://luts/print_32.png");
                assert_eq!(*size, 16);
                assert_eq!(*strength, 1.0);
            }
            other => panic!("expected lut3d, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_camera_audio_options() {
        let scene = Parser::new(
            r#"
            [cam2d]
            [Camera2D]
                audio_options = {
                    audio_mask = [1, 3],
                    effects = [
                        { reverb_send: 0.6, echo: 0.2, dampening: 0.4 }
                    ]
                }
            [/Camera2D]
            [/cam2d]

            [cam3d]
            [Camera3D]
                audio_mask = [2]
                reverb_send = 0.7
                echo = 0.1
                dampening = 0.5
            [/Camera3D]
            [/cam3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let cam2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam2d")
            .expect("cam2d");
        let SceneNodeData::Camera2D(cam2d) = &cam2d.node.data else {
            panic!("expected Camera2D");
        };
        assert_eq!(cam2d.audio_options.audio_mask.bits(), 0b101);
        assert_eq!(cam2d.audio_options.effects.len(), 1);
        assert_eq!(cam2d.audio_options.effects[0].reverb_send, 0.6);
        assert_eq!(cam2d.audio_options.effects[0].echo, 0.2);
        assert_eq!(cam2d.audio_options.effects[0].dampening, 0.4);

        let cam3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam3d")
            .expect("cam3d");
        let SceneNodeData::Camera3D(cam3d) = &cam3d.node.data else {
            panic!("expected Camera3D");
        };
        assert_eq!(cam3d.audio_options.audio_mask.bits(), 0b10);
        assert_eq!(cam3d.audio_options.effects.len(), 1);
        assert_eq!(cam3d.audio_options.effects[0].reverb_send, 0.7);
        assert_eq!(cam3d.audio_options.effects[0].echo, 0.1);
        assert_eq!(cam3d.audio_options.effects[0].dampening, 0.5);
    }

    #[test]
    fn scene_loader_builds_ui_image_button_fields() {
        let scene = Parser::new(
            r##"
            $root = @icon
            [icon]
            [UiImageButton]
                texture = "res://ui/play.png"
                size_ratio = (0.08, 0.12)
                scale_mode = "fit"
                tint = "#11223344"
                hover_tint = "#55667788"
                pressed_tint = "#99AABBCC"
                texture_region = (1, 2, 16, 32)
                clicked_signals = ["play_clicked"]
                hover = { scale = (1.1, 1.1), tint = "#FFFFFFFF" }
                pressed = { scale = (0.9, 0.9), tint = "#CCCCCCFF" }
            [/UiImageButton]
            [/icon]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let icon = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "icon")
            .expect("icon node");
        assert_eq!(icon.texture_source.as_deref(), Some("res://ui/play.png"));
        match &icon.node.data {
            SceneNodeData::UiImageButton(button) => {
                assert_eq!(button.layout.size, perro_ui::UiVector2::ratio(0.08, 0.12));
                assert_eq!(button.scale_mode, perro_ui::UiImageScaleMode::Fit);
                assert_eq!(button.tint, Color::new(0.06666667, 0.13333334, 0.2, 0.26666668));
                assert_eq!(button.hover_tint, Color::WHITE);
                assert_eq!(
                    button.pressed_tint,
                    Color::new(0.8, 0.8, 0.8, 1.0)
                );
                assert_eq!(button.texture_region, Some([1.0, 2.0, 16.0, 32.0]));
                assert_eq!(
                    button.clicked_signals,
                    vec![SignalID::from_string("play_clicked")]
                );
                assert!(button.hover_base.is_some());
                assert!(button.pressed_base.is_some());
            }
            other => panic!("expected UiImageButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ui_shape_fields() {
        let scene = Parser::new(
            r##"
            $root = @shape
            [shape]
            [UiShape]
                shape = "triangle"
                fill = "#336699FF"
                stroke = "#CCDDEEFF"
                stroke_width = 2.5
            [/UiShape]
            [/shape]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let shape = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "shape")
            .expect("shape node");
        match &shape.node.data {
            SceneNodeData::UiShape(shape) => {
                assert_eq!(shape.kind, perro_ui::UiShapeKind::Triangle);
                assert_eq!(shape.fill, Color::new(0.2, 0.4, 0.6, 1.0));
                assert_eq!(
                    shape.stroke,
                    Color::new(0.8, 0.8666667, 0.93333334, 1.0)
                );
                assert_eq!(shape.stroke_width, 2.5);
            }
            other => panic!("expected UiShape node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_2d_button_fields() {
        let scene = Parser::new(
            r##"
            $root = @play
            [play]
            [Button2D]
                position = (12, 34)
                size = (96, 40)
                fill = "#112233FF"
                hover_fill = "#445566FF"
                pressed_fill = "#778899FF"
                clicked_signals = ["play_clicked"]
            [/Button2D]
            [/play]

            [icon]
            [ImageButton2D]
                texture = "res://ui/icon.png"
                size = (24, 18)
                tint = "#FFFFFFFF"
                hover_tint = "#CCCCCCFF"
                pressed_tint = "#999999FF"
                texture_region = (1, 2, 8, 9)
            [/ImageButton2D]
            [/icon]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let play = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "play")
            .expect("play node");
        match &play.node.data {
            SceneNodeData::Button2D(button) => {
                assert_eq!(button.transform.position, Vector2::new(12.0, 34.0));
                assert_eq!(button.size, Vector2::new(96.0, 40.0));
                assert_eq!(button.style.fill, Color::new(0.06666667, 0.13333334, 0.2, 1.0));
                assert_eq!(button.hover_style.fill, Color::new(0.26666668, 0.33333334, 0.4, 1.0));
                assert_eq!(button.pressed_style.fill, Color::new(0.46666667, 0.53333336, 0.6, 1.0));
                assert_eq!(
                    button.clicked_signals,
                    vec![SignalID::from_string("play_clicked")]
                );
            }
            other => panic!("expected Button2D node, got {other:?}"),
        }

        let icon = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "icon")
            .expect("icon node");
        assert_eq!(icon.texture_source.as_deref(), Some("res://ui/icon.png"));
        match &icon.node.data {
            SceneNodeData::ImageButton2D(button) => {
                assert_eq!(button.size, Vector2::new(24.0, 18.0));
                assert_eq!(button.hover_tint, Color::new(0.8, 0.8, 0.8, 1.0));
                assert_eq!(button.pressed_tint, Color::new(0.6, 0.6, 0.6, 1.0));
                assert_eq!(button.texture_region, Some([1.0, 2.0, 8.0, 9.0]));
            }
            other => panic!("expected ImageButton2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_nine_slice_fields() {
        let scene = Parser::new(
            r##"
            $root = @panel
            [panel]
            [UiNineSlice]
                texture = "res://ui/panel.png"
                size_ratio = (0.3, 0.2)
                margins = (4, 5, 6, 7)
                texture_region = (1, 2, 30, 20)
                tint = "#FFFFFFFF"
            [/UiNineSlice]
            [/panel]

            [world]
            [NineSlice2D]
                texture = "res://ui/world_panel.png"
                size = (90, 30)
                margins = (3, 4, 5, 6)
                texture_region = (2, 4, 20, 10)
            [/NineSlice2D]
            [/world]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let panel = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "panel")
            .expect("panel node");
        assert_eq!(panel.texture_source.as_deref(), Some("res://ui/panel.png"));
        match &panel.node.data {
            SceneNodeData::UiNineSlice(node) => {
                assert_eq!(node.layout.size, perro_ui::UiVector2::ratio(0.3, 0.2));
                assert_eq!(node.margins, [4.0, 5.0, 6.0, 7.0]);
                assert_eq!(node.texture_region, Some([1.0, 2.0, 30.0, 20.0]));
            }
            other => panic!("expected UiNineSlice node, got {other:?}"),
        }

        let world = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "world")
            .expect("world node");
        assert_eq!(
            world.texture_source.as_deref(),
            Some("res://ui/world_panel.png")
        );
        match &world.node.data {
            SceneNodeData::NineSlice2D(node) => {
                assert_eq!(node.size, Vector2::new(90.0, 30.0));
                assert_eq!(node.margins, [3.0, 4.0, 5.0, 6.0]);
                assert_eq!(node.texture_region, Some([2.0, 4.0, 20.0, 10.0]));
            }
            other => panic!("expected NineSlice2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_layer_arrays_for_bitmasks() {
        let scene = Parser::new(
            r#"
            $root = @sprite
            [sprite]
            [Sprite2D]
                render_layers = [1, 3]
            [/Sprite2D]
            [/sprite]

            [body]
            [StaticBody2D]
                collision_layers = [2, 4]
                collision_mask = [1, 3]
            [/StaticBody2D]
            [/body]

            [camera]
            [Camera3D]
                render_mask = [2, 5]
            [/Camera3D]
            [/camera]

            [area]
            [Area3D]
                collision_layers = [5]
                collision_mask = [1, 2]
            [/Area3D]
            [/area]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sprite = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sprite")
            .expect("sprite node");
        match &sprite.node.data {
            SceneNodeData::Sprite2D(node) => assert_eq!(node.render_layers.bits(), 0b101),
            other => panic!("expected Sprite2D node, got {other:?}"),
        }

        let body = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "body")
            .expect("body node");
        match &body.node.data {
            SceneNodeData::StaticBody2D(node) => {
                assert_eq!(node.collision_layers.bits(), 0b1010);
                assert_eq!(node.collision_mask.bits(), 0b101);
            }
            other => panic!("expected StaticBody2D node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => assert_eq!(node.render_mask.bits(), 0b10010),
            other => panic!("expected Camera3D node, got {other:?}"),
        }

        let area = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "area")
            .expect("area node");
        match &area.node.data {
            SceneNodeData::Area3D(node) => {
                assert_eq!(node.collision_layers.bits(), 0b10000);
                assert_eq!(node.collision_mask.bits(), 0b11);
            }
            other => panic!("expected Area3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_bitmask_only_and_without_calls() {
        let scene = Parser::new(
            r#"
            $root = @sprite
            [sprite]
            [Sprite2D]
                render_layers = only(1, 3)
            [/Sprite2D]
            [/sprite]

            [camera]
            [Camera3D]
                render_mask = without(1)
            [/Camera3D]
            [/camera]

            [body]
            [StaticBody2D]
                collision_layers = without([1, 32])
                collision_mask = only([2, 4])
            [/StaticBody2D]
            [/body]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sprite = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sprite")
            .expect("sprite node");
        match &sprite.node.data {
            SceneNodeData::Sprite2D(node) => assert_eq!(node.render_layers.bits(), 0b101),
            other => panic!("expected Sprite2D node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => {
                assert_eq!(node.render_mask.bits(), !0b1);
            }
            other => panic!("expected Camera3D node, got {other:?}"),
        }

        let body = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "body")
            .expect("body node");
        match &body.node.data {
            SceneNodeData::StaticBody2D(node) => {
                assert_eq!(node.collision_layers.bits(), !0b1 & !(1u32 << 31));
                assert_eq!(node.collision_mask.bits(), 0b1010);
            }
            other => panic!("expected StaticBody2D node, got {other:?}"),
        }
    }
}
