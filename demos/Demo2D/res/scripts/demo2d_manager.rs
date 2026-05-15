use perro_api::prelude::*;

type SelfNodeType = Node2D;

const SPRITE_SHEET: &ResPath = res_path!("res://sprite_sheet.png");
const HERO_SHEET: &ResPath = res_path!("res://hero_sheet.png");
const LIGHT_DISC: &ResPath = res_path!("res://light_disc.png");
const RIG_SCENE: &ResPath = res_path!("res://scenes/rig_actor.scn");
const PLAYER_BOB: &ResPath = res_path!("res://animations/player_bob.panim");
const MAIN_MENU_SCENE: &ResPath = res_path!("res://Menu/MainMenu.scn");
const PAUSE_MENU_SCENE: &ResPath = res_path!("res://Menu/PauseMenu.scn");
const TRANSITION_FADE_SCENE: &ResPath = res_path!("res://Menu/TransitionFade.scn");
const PROFILING_OVERLAY_SCENE: &ResPath = res_path!("res://Menu/ProfilingOverlay.scn");
const INFO_OVERLAY_SCENE: &ResPath = res_path!("res://Menu/InfoOverlay.scn");
const DEMO_UI_ROOT_NODE_NAME: &str = "demo_ui_root";
const CAMERA_NODE_NAME: &str = "Camera";
const TOP_BAR_NODE_NAME: &str = "TopBar";
const TRANSITION_FADE_PANEL_NODE_NAME: &str = "transition_fade_panel";
const FADE_SECONDS: f32 = 0.30;

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum DemoKind {
    #[default]
    None,
    MeshMaterials,
    Lights,
    Water,
    Animations,
    PhysicsBones,
    PhysicsCollisions,
    SkyGap,
    BlendGap,
    MultiMesh,
    ParticlesGap,
    AudioGap,
}

#[derive(Variant, Clone, Copy, Default)]
struct DemoAssets {
    sprite_sheet: TextureID,
    hero_sheet: TextureID,
    light_disc: TextureID,
    rig_scene: PreloadedSceneID,
    player_bob: AnimationID,
    main_menu: PreloadedSceneID,
    pause_menu: PreloadedSceneID,
    fade: PreloadedSceneID,
    profiling_overlay: PreloadedSceneID,
    info_overlay: PreloadedSceneID,
}

#[derive(Variant, Clone, Default)]
struct DemoUiRefs {
    main_menu_root: NodeID,
    pause_menu_root: NodeID,
    fade_root: NodeID,
    fade_panel: NodeID,
    profiling_overlay_root: NodeID,
    info_overlay_root: NodeID,
    top_bar_root: NodeID,
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum FadeAction {
    #[default]
    None,
    ActivateDemo,
    ShowHub,
    RestartDemo,
}

#[derive(Variant, Clone, Copy)]
struct DemoRuntimeState {
    active_demo: DemoKind,
    queued_demo: DemoKind,
    paused: bool,
    fade_alpha: f32,
    fade_active: bool,
    fade_midpoint_done: bool,
    fade_action: FadeAction,
}

impl Default for DemoRuntimeState {
    fn default() -> Self {
        Self {
            active_demo: DemoKind::None,
            queued_demo: DemoKind::None,
            paused: false,
            fade_alpha: 0.0,
            fade_active: false,
            fade_midpoint_done: false,
            fade_action: FadeAction::None,
        }
    }
}

#[State]
struct Demo2DState {
    #[default = DemoAssets::default()]
    assets: DemoAssets,
    #[default = DemoUiRefs::default()]
    ui: DemoUiRefs,
    #[default = DemoRuntimeState::default()]
    runtime: DemoRuntimeState,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let assets = DemoAssets {
            sprite_sheet: texture_load!(ctx.res, SPRITE_SHEET),
            hero_sheet: texture_load!(ctx.res, HERO_SHEET),
            light_disc: texture_load!(ctx.res, LIGHT_DISC),
            rig_scene: scene_preload!(ctx.run, RIG_SCENE).expect("preload rig scene"),
            player_bob: animation_load!(ctx.res, PLAYER_BOB),
            main_menu: scene_preload!(ctx.run, MAIN_MENU_SCENE).expect("preload main menu"),
            pause_menu: scene_preload!(ctx.run, PAUSE_MENU_SCENE).expect("preload pause menu"),
            fade: scene_preload!(ctx.run, TRANSITION_FADE_SCENE).expect("preload fade"),
            profiling_overlay: scene_preload!(ctx.run, PROFILING_OVERLAY_SCENE)
                .expect("preload profiling overlay"),
            info_overlay: scene_preload!(ctx.run, INFO_OVERLAY_SCENE).expect("preload info overlay"),
        };
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| state.assets = assets);
        self.load_ui(ctx);
        self.connect_ui_signals(ctx);
        self.sync_ui(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        self.update_fade(ctx);
        let (paused, fade_active) = with_state!(ctx.run, Demo2DState, ctx.id, |state| {
            (state.runtime.paused, state.runtime.fade_active)
        });
        if fade_active {
            return;
        }

        if key_pressed!(ctx.ipt, KeyCode::Escape) {
            if paused {
                self.resume_demo(ctx);
            } else if self.active_demo(ctx) != DemoKind::None {
                self.open_pause(ctx);
            }
        }

        if key_pressed!(ctx.ipt, KeyCode::KeyR) {
            self.restart_active_demo(ctx);
        }
    }
});

methods!({
    fn on_demo_mesh_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::MeshMaterials);
    }
    fn on_demo_lights_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::Lights);
    }
    fn on_demo_water_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::Water);
    }
    fn on_demo_animations_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::Animations);
    }
    fn on_demo_physics_bones_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::PhysicsBones);
    }
    fn on_demo_physics_collisions_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::PhysicsCollisions);
    }
    fn on_demo_sky_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::SkyGap);
    }
    fn on_demo_blend_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::BlendGap);
    }
    fn on_demo_multimesh_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::MultiMesh);
    }
    fn on_demo_particles_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::ParticlesGap);
    }
    fn on_demo_audio_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::AudioGap);
    }
    fn on_pause_resume_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.resume_demo(ctx);
    }
    fn on_pause_restart_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.restart_active_demo(ctx);
        self.resume_demo(ctx);
    }
    fn on_pause_hub_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.show_hub(ctx);
    }

    fn rebuild(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        self.clear_dynamic(ctx);
        match demo {
            DemoKind::None => {}
            DemoKind::MeshMaterials => {
                self.spawn_static_sprite_zone(ctx, Vector2::new(-2400.0, 900.0), 16, 16, 34.0, "static_256");
                self.spawn_static_sprite_zone(ctx, Vector2::new(-1700.0, 900.0), 32, 32, 18.0, "static_1024");
                self.spawn_static_sprite_zone(ctx, Vector2::new(-450.0, 900.0), 64, 64, 10.0, "static_4096");
            }
            DemoKind::Lights => {
                self.spawn_light_zone(ctx, Vector2::new(1450.0, 850.0));
            }
            DemoKind::Water => {
                self.spawn_water_zone(ctx, Vector2::new(1500.0, 120.0));
            }
            DemoKind::Animations => {
                self.spawn_animated_sprite_zone(ctx, Vector2::new(-2400.0, 260.0), 8, 8, 44.0);
                self.spawn_animated_sprite_zone(ctx, Vector2::new(-1700.0, 260.0), 16, 16, 24.0);
                self.spawn_animated_sprite_zone(ctx, Vector2::new(-450.0, 260.0), 32, 32, 13.0);
                self.spawn_animation_player_zone(ctx, Vector2::new(-1750.0, -920.0));
            }
            DemoKind::PhysicsBones => {
                self.spawn_skeletal_zone(ctx, Vector2::new(100.0, -920.0));
            }
            DemoKind::PhysicsCollisions => {
                self.spawn_physics_zone(ctx, Vector2::new(1350.0, -920.0));
            }
            DemoKind::SkyGap | DemoKind::BlendGap | DemoKind::ParticlesGap | DemoKind::AudioGap => {}
            DemoKind::MultiMesh => {
                self.spawn_static_sprite_zone(ctx, Vector2::new(-450.0, 900.0), 64, 64, 10.0, "multimesh_2d");
            }
        }
    }

    fn load_ui(&self, ctx: &mut ScriptContext<'_, API>) {
        let parent = scene_ui_parent(ctx, ctx.id);
        let scene_root = get_node_parent_id!(ctx.run, ctx.id).unwrap_or(ctx.id);
        let assets = self.assets(ctx);
        let main_menu_root = scene_load!(ctx.run, assets.main_menu).unwrap_or(NodeID::nil());
        let pause_menu_root = scene_load!(ctx.run, assets.pause_menu).unwrap_or(NodeID::nil());
        let fade_root = scene_load!(ctx.run, assets.fade).unwrap_or(NodeID::nil());
        let profiling_overlay_root =
            scene_load!(ctx.run, assets.profiling_overlay).unwrap_or(NodeID::nil());
        let info_overlay_root = scene_load!(ctx.run, assets.info_overlay).unwrap_or(NodeID::nil());
        if !main_menu_root.is_nil() {
            reparent!(ctx.run, parent, main_menu_root);
        }
        if !pause_menu_root.is_nil() {
            reparent!(ctx.run, parent, pause_menu_root);
        }
        if !fade_root.is_nil() {
            reparent!(ctx.run, parent, fade_root);
        }
        if !profiling_overlay_root.is_nil() {
            reparent!(ctx.run, parent, profiling_overlay_root);
        }
        if !info_overlay_root.is_nil() {
            reparent!(ctx.run, parent, info_overlay_root);
        }
        let fade_panel = if fade_root.is_nil() {
            NodeID::nil()
        } else {
            get_child!(ctx.run, fade_root, TRANSITION_FADE_PANEL_NODE_NAME).unwrap_or(NodeID::nil())
        };
        let top_bar_root = get_child!(ctx.run, scene_root, TOP_BAR_NODE_NAME).unwrap_or(NodeID::nil());
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.ui.main_menu_root = main_menu_root;
            state.ui.pause_menu_root = pause_menu_root;
            state.ui.fade_root = fade_root;
            state.ui.fade_panel = fade_panel;
            state.ui.profiling_overlay_root = profiling_overlay_root;
            state.ui.info_overlay_root = info_overlay_root;
            state.ui.top_bar_root = top_bar_root;
        });
        self.apply_transition_fade(ctx, 0.0, false);
    }

    fn connect_ui_signals(&self, ctx: &mut ScriptContext<'_, API>) {
        for (sig, func_name) in [
            ("demo_mesh_click", "on_demo_mesh_click"),
            ("demo_lights_click", "on_demo_lights_click"),
            ("demo_water_click", "on_demo_water_click"),
            ("demo_animations_click", "on_demo_animations_click"),
            ("demo_physics_bones_click", "on_demo_physics_bones_click"),
            ("demo_physics_collisions_click", "on_demo_physics_collisions_click"),
            ("demo_sky_click", "on_demo_sky_click"),
            ("demo_blend_click", "on_demo_blend_click"),
            ("demo_multimesh_click", "on_demo_multimesh_click"),
            ("demo_particles_click", "on_demo_particles_click"),
            ("demo_audio_click", "on_demo_audio_click"),
            ("pause_resume_click", "on_pause_resume_click"),
            ("pause_restart_click", "on_pause_restart_click"),
            ("pause_hub_click", "on_pause_hub_click"),
        ] {
            signal_connect!(ctx.run, ctx.id, signal!(sig), func!(func_name));
        }
    }

    fn activate_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        if demo == DemoKind::None {
            return;
        }
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.queued_demo = demo;
            state.runtime.fade_action = FadeAction::ActivateDemo;
        });
        self.start_transition_fade(ctx);
    }

    fn show_hub(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.queued_demo = DemoKind::None;
            state.runtime.fade_action = FadeAction::ShowHub;
        });
        self.start_transition_fade(ctx);
    }

    fn open_pause(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.paused = true;
        });
        physics_pause!(ctx.run, true);
        self.sync_ui(ctx);
    }

    fn resume_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.paused = false;
        });
        physics_pause!(ctx.run, false);
        self.sync_ui(ctx);
    }

    fn restart_active_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let demo = self.active_demo(ctx);
        if demo == DemoKind::None {
            return;
        }
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.queued_demo = demo;
            state.runtime.fade_action = FadeAction::RestartDemo;
        });
        self.start_transition_fade(ctx);
    }

    fn start_transition_fade(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.fade_alpha = 1.0;
            state.runtime.fade_active = true;
            state.runtime.fade_midpoint_done = false;
        });
        self.apply_transition_fade(ctx, 1.0, true);
    }

    fn update_fade(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let Some((alpha, active, midpoint)) = with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            if !state.runtime.fade_active {
                return (0.0, false, false);
            }
            let prev_alpha = state.runtime.fade_alpha;
            let step = if FADE_SECONDS <= 0.0001 { 1.0 } else { dt / FADE_SECONDS };
            state.runtime.fade_alpha = (state.runtime.fade_alpha - step).max(0.0);
            let midpoint =
                !state.runtime.fade_midpoint_done && prev_alpha > 0.5 && state.runtime.fade_alpha <= 0.5;
            if midpoint {
                state.runtime.fade_midpoint_done = true;
            }
            if state.runtime.fade_alpha <= 0.001 {
                state.runtime.fade_alpha = 0.0;
                state.runtime.fade_active = false;
                state.runtime.fade_midpoint_done = false;
                state.runtime.fade_action = FadeAction::None;
            }
            (state.runtime.fade_alpha, state.runtime.fade_active, midpoint)
        }) else {
            return;
        };
        if midpoint {
            self.apply_fade_action(ctx);
        }
        self.apply_transition_fade(ctx, alpha, active);
        if !active {
            self.sync_ui(ctx);
        }
    }

    fn apply_transition_fade(&self, ctx: &mut ScriptContext<'_, API>, alpha: f32, visible: bool) {
        let (root, panel) = with_state!(ctx.run, Demo2DState, ctx.id, |state| {
            (state.ui.fade_root, state.ui.fade_panel)
        });
        let clamped = if visible { alpha.clamp(0.0, 1.0) } else { 0.0 };
        let show = visible && clamped > 0.001;
        let color = color_with_alpha("#000000", clamped);
        for id in [root, panel] {
            if id.is_nil() {
                continue;
            }
            let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
                if let Some(color) = color {
                    node.style.fill = color;
                    node.style.stroke = color;
                    node.style.stroke_width = 0.0;
                }
                node.visible = show;
                node.input_enabled = false;
            });
        }
    }

    fn jump_camera_to_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        let camera = find_descendant_by_name(ctx, ctx.id, CAMERA_NODE_NAME);
        if camera.is_nil() {
            return;
        }
        let target = demo_anchor(demo);
        let _ = set_local_pos_2d!(ctx.run, camera, target);
    }

    fn apply_fade_action(&self, ctx: &mut ScriptContext<'_, API>) {
        let (action, demo) = with_state!(ctx.run, Demo2DState, ctx.id, |state| {
            (state.runtime.fade_action, state.runtime.queued_demo)
        });
        match action {
            FadeAction::None => {}
            FadeAction::ActivateDemo | FadeAction::RestartDemo => {
                with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
                    state.runtime.active_demo = demo;
                    state.runtime.paused = false;
                });
                self.rebuild(ctx, demo);
                self.jump_camera_to_demo(ctx, demo);
                physics_pause!(ctx.run, false);
            }
            FadeAction::ShowHub => {
                with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
                    state.runtime.active_demo = DemoKind::None;
                    state.runtime.paused = false;
                });
                self.clear_dynamic(ctx);
                self.jump_camera_to_demo(ctx, DemoKind::MeshMaterials);
                physics_pause!(ctx.run, false);
            }
        }
        self.sync_info_overlay(ctx);
        self.sync_ui(ctx);
    }

    fn sync_ui(&self, ctx: &mut ScriptContext<'_, API>) {
        let (menu, pause, top, profiling, info, active_demo, paused) =
            with_state!(ctx.run, Demo2DState, ctx.id, |state| {
                (
                    state.ui.main_menu_root,
                    state.ui.pause_menu_root,
                    state.ui.top_bar_root,
                    state.ui.profiling_overlay_root,
                    state.ui.info_overlay_root,
                    state.runtime.active_demo,
                    state.runtime.paused,
                )
            });
        let in_hub = active_demo == DemoKind::None;
        set_ui_tree_visible(ctx, menu, in_hub);
        set_ui_tree_visible(ctx, pause, paused);
        set_ui_tree_visible(ctx, top, !in_hub);
        set_ui_tree_visible(ctx, profiling, true);
        set_ui_tree_visible(ctx, info, true);
        self.sync_info_overlay(ctx);
    }

    fn clear_dynamic(&self, ctx: &mut ScriptContext<'_, API>) {
        for node in query!(ctx.run, all(tags["demo2d_dynamic"]), in_subtree(ctx.id)) {
            let _ = remove_node!(ctx.run, node);
        }
    }

    fn spawn_static_sprite_zone(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        origin: Vector2,
        cols: i32,
        rows: i32,
        step: f32,
        _name: &str,
    ) {
        let tex = self.assets(ctx).sprite_sheet;
        for y in 0..rows {
            for x in 0..cols {
                let idx = ((y * cols + x) % 64) as f32;
                let node = create_node!(ctx.run, Sprite2D, "static_sprite", tags!["demo2d_dynamic"], ctx.id);
                let px = origin.x + x as f32 * step;
                let py = origin.y - y as f32 * step;
                let _ = with_node_mut!(ctx.run, Sprite2D, node, |sprite| {
                    sprite.texture = tex;
                    sprite.texture_region = Some([32.0 * (idx % 8.0), 32.0 * (idx / 8.0).floor(), 32.0, 32.0]);
                    sprite.transform.position = Vector2::new(px, py);
                    sprite.transform.scale = Vector2::new(0.55, 0.55);
                });
            }
        }
    }

    fn spawn_animated_sprite_zone(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        origin: Vector2,
        cols: i32,
        rows: i32,
        step: f32,
    ) {
        let tex = self.assets(ctx).hero_sheet;
        for y in 0..rows {
            for x in 0..cols {
                let node = create_node!(ctx.run, AnimatedSprite2D, "anim_sprite", tags!["demo2d_dynamic"], ctx.id);
                let px = origin.x + x as f32 * step;
                let py = origin.y - y as f32 * step;
                let _ = with_node_mut!(ctx.run, AnimatedSprite2D, node, |sprite| {
                    sprite.texture = tex;
                    sprite.animations = vec![AnimatedSprite {
                        name: "run".into(),
                        start: [0.0, 0.0],
                        frame_size: [32.0, 32.0],
                        frame_count: 4,
                        columns: 4,
                        fps: 10.0 + ((x + y) % 4) as f32,
                    }];
                    sprite.current_animation = "run".into();
                    sprite.playing = true;
                    sprite.looping = true;
                    sprite.transform.position = Vector2::new(px, py);
                    sprite.transform.scale = Vector2::new(0.75, 0.75);
                });
            }
        }
    }

    fn spawn_light_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        self.spawn_static_sprite_zone(ctx, origin + Vector2::new(-360.0, 120.0), 16, 16, 22.0, "light_bg");

        let disc = self.assets(ctx).light_disc;
        for i in 0..8 {
            let node = create_node!(ctx.run, PointLight2D, "point_light", tags!["demo2d_dynamic"], ctx.id);
            let x = origin.x - 280.0 + i as f32 * 80.0;
            let y = origin.y + if i % 2 == 0 { 100.0 } else { -120.0 };
            let hue = i as f32 / 8.0;
            let _ = with_node_mut!(ctx.run, PointLight2D, node, |light| {
                light.color = palette_rgb(hue);
                light.intensity = 2.1;
                light.range = 180.0;
                light.transform.position = Vector2::new(x, y);
            });
            self.spawn_marker_sprite(ctx, disc, Vector2::new(x, y), 1.5);
        }

        for i in 0..2 {
            let node = create_node!(ctx.run, SpotLight2D, "spot_light", tags!["demo2d_dynamic"], ctx.id);
            let _ = with_node_mut!(ctx.run, SpotLight2D, node, |light| {
                light.color = if i == 0 { [1.0, 0.9, 0.45] } else { [0.45, 0.95, 1.0] };
                light.intensity = 2.6;
                light.range = 260.0;
                light.inner_angle_radians = 0.25;
                light.outer_angle_radians = 0.75;
                light.transform.position = origin + Vector2::new(-100.0 + i as f32 * 220.0, 240.0);
                light.transform.rotation = if i == 0 { -1.2 } else { -1.9 };
            });
        }

        for i in 0..2 {
            let node = create_node!(ctx.run, RayLight2D, "ray_light", tags!["demo2d_dynamic"], ctx.id);
            let _ = with_node_mut!(ctx.run, RayLight2D, node, |light| {
                light.color = if i == 0 { [1.0, 0.35, 0.5] } else { [0.55, 1.0, 0.45] };
                light.intensity = 2.0;
                light.transform.position = origin + Vector2::new(-250.0 + i as f32 * 500.0, -260.0);
                light.transform.rotation = if i == 0 { 0.4 } else { -0.4 };
            });
        }
    }

    fn spawn_water_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        for pool in 0..3 {
            let water = create_node!(ctx.run, WaterBody2D, "water", tags!["demo2d_dynamic"], ctx.id);
            let base = origin + Vector2::new(pool as f32 * 240.0 - 240.0, 0.0);
            let _ = with_node_mut!(ctx.run, WaterBody2D, water, |node| {
                node.transform.position = base;
                node.water.shape = WaterShape::rect(Vector2::new(170.0, 90.0));
                node.water.resolution = [96, 48];
                node.water.render_resolution = [96, 48];
                node.water.flow = Vector2::new(0.3 * (pool as f32 - 1.0), 0.0);
                node.water.wind = Vector2::new(1.0, 0.0);
                node.water.optics.deep_color = [0.02, 0.15 + pool as f32 * 0.08, 0.26 + pool as f32 * 0.06, 0.96].into();
                node.water.optics.shallow_color = [0.12, 0.48, 0.72, 0.72].into();
                node.water.visual.foam_color = [0.86, 0.96, 1.0, 1.0].into();
                node.water.physics.buoyancy = 2.8;
                node.water.physics.drag = 0.7;
            });

            let floor = create_node!(ctx.run, StaticBody2D, "water_floor", tags!["demo2d_dynamic"], ctx.id);
            let _ = with_node_mut!(ctx.run, StaticBody2D, floor, |body| {
                body.transform.position = base + Vector2::new(0.0, -70.0);
            });
            let shape = create_node!(ctx.run, CollisionShape2D, "water_floor_shape", tags!["demo2d_dynamic"], floor);
            let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                s.shape = Shape2D::Quad { width: 180.0, height: 16.0 };
            });

            for i in 0..16 {
                let body = create_node!(ctx.run, RigidBody2D, "floater", tags!["demo2d_dynamic"], ctx.id);
                let px = base.x - 68.0 + (i % 4) as f32 * 44.0;
                let py = base.y - 16.0 + (i / 4) as f32 * 34.0;
                let _ = with_node_mut!(ctx.run, RigidBody2D, body, |rb| {
                    rb.transform.position = Vector2::new(px, py);
                    rb.gravity_scale = 1.0;
                    rb.linear_damping = 0.35;
                    rb.angular_damping = 0.22;
                    rb.density = 0.65;
                    rb.friction = 0.4;
                });
                let shape = create_node!(ctx.run, CollisionShape2D, "floater_shape", tags!["demo2d_dynamic"], body);
                let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                    s.shape = Shape2D::Quad { width: 20.0, height: 20.0 };
                });
                let sprite_sheet = self.assets(ctx).sprite_sheet;
                self.spawn_child_sprite(ctx, body, sprite_sheet, 32.0 * ((i % 8) as f32), 64.0, 32.0, 32.0, 0.65);
            }
        }
    }

    fn spawn_physics_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let ground = create_node!(ctx.run, StaticBody2D, "ground", tags!["demo2d_dynamic"], ctx.id);
        let _ = with_node_mut!(ctx.run, StaticBody2D, ground, |body| {
            body.transform.position = origin + Vector2::new(0.0, -40.0);
        });
        let ground_shape = create_node!(ctx.run, CollisionShape2D, "ground_shape", tags!["demo2d_dynamic"], ground);
        let _ = with_node_mut!(ctx.run, CollisionShape2D, ground_shape, |shape| {
            shape.shape = Shape2D::Quad { width: 860.0, height: 24.0 };
        });

        for i in 0..240 {
            let body = create_node!(ctx.run, RigidBody2D, "crate", tags!["demo2d_dynamic"], ctx.id);
            let col = (i % 20) as f32;
            let row = (i / 20) as f32;
            let _ = with_node_mut!(ctx.run, RigidBody2D, body, |rb| {
                rb.transform.position = origin + Vector2::new(-350.0 + col * 36.0, row * 34.0 + 10.0);
                rb.angular_velocity = (i % 3) as f32 * 0.15;
                rb.friction = 0.6;
                rb.restitution = 0.08;
            });
            let shape = create_node!(ctx.run, CollisionShape2D, "crate_shape", tags!["demo2d_dynamic"], body);
            let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                s.shape = if i % 5 == 0 {
                    Shape2D::Circle { radius: 12.0 }
                } else {
                    Shape2D::Quad { width: 24.0, height: 24.0 }
                };
            });
            let sprite_sheet = self.assets(ctx).sprite_sheet;
            self.spawn_child_sprite(ctx, body, sprite_sheet, 32.0 * ((i % 8) as f32), 96.0, 32.0, 32.0, 0.7);
        }
    }

    fn spawn_animation_player_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let clip = self.assets(ctx).player_bob;
        let tex = self.assets(ctx).hero_sheet;
        for i in 0..48 {
            let actor = create_node!(ctx.run, Node2D, "actor_root", tags!["demo2d_dynamic"], ctx.id);
            let col = (i % 12) as f32;
            let row = (i / 12) as f32;
            let _ = with_base_node_mut!(ctx.run, Node2D, actor, |node| {
                node.transform.position = origin + Vector2::new(-330.0 + col * 62.0, row * 84.0);
            });

            let sprite = create_node!(ctx.run, AnimatedSprite2D, "actor_sprite", tags!["demo2d_dynamic"], actor);
            let _ = with_node_mut!(ctx.run, AnimatedSprite2D, sprite, |node| {
                node.texture = tex;
                node.animations = vec![AnimatedSprite {
                    name: "run".into(),
                    start: [0.0, 0.0],
                    frame_size: [32.0, 32.0],
                    frame_count: 4,
                    columns: 4,
                    fps: 9.0 + (i % 4) as f32,
                }];
                node.current_animation = "run".into();
                node.transform.scale = Vector2::new(1.0, 1.0);
            });

            let player = create_node!(ctx.run, AnimationPlayer, "actor_player", tags!["demo2d_dynamic"], ctx.id);
            let _ = with_node_mut!(ctx.run, AnimationPlayer, player, |anim| {
                anim.animation = clip;
                anim.speed = 0.8 + (i % 5) as f32 * 0.08;
                anim.paused = false;
                anim.playback_type = AnimationPlaybackType::Loop;
                anim.set_binding("Actor", actor);
            });
        }
    }

    fn spawn_skeletal_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let scene = self.assets(ctx).rig_scene;
        for i in 0..12 {
            let Ok(root) = scene_load!(ctx.run, scene) else { continue; };
            reparent!(ctx.run, ctx.id, root);
            tag_add!(ctx.run, root, tags!["demo2d_dynamic"]);
            let col = (i % 4) as f32;
            let row = (i / 4) as f32;
            let pos = origin + Vector2::new(-260.0 + col * 180.0, row * 170.0);
            let _ = set_local_pos_2d!(ctx.run, root, pos);
            let _ = set_local_scale_2d!(ctx.run, root, Vector2::new(1.5, 1.5));
        }
    }

    fn spawn_marker_sprite(&self, ctx: &mut ScriptContext<'_, API>, tex: TextureID, pos: Vector2, scale: f32) {
        let node = create_node!(ctx.run, Sprite2D, "light_marker", tags!["demo2d_dynamic"], ctx.id);
        let _ = with_node_mut!(ctx.run, Sprite2D, node, |sprite| {
            sprite.texture = tex;
            sprite.transform.position = pos;
            sprite.transform.scale = Vector2::new(scale, scale);
        });
    }

    fn spawn_child_sprite(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        parent: NodeID,
        tex: TextureID,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        scale: f32,
    ) {
        let sprite = create_node!(ctx.run, Sprite2D, "child_sprite", tags!["demo2d_dynamic"], parent);
        let _ = with_node_mut!(ctx.run, Sprite2D, sprite, |node| {
            node.texture = tex;
            node.texture_region = Some([x, y, w, h]);
            node.transform.scale = Vector2::new(scale, scale);
        });
    }

    fn assets(&self, ctx: &mut ScriptContext<'_, API>) -> DemoAssets {
        with_state!(ctx.run, Demo2DState, ctx.id, |state| state.assets)
    }

    fn active_demo(&self, ctx: &mut ScriptContext<'_, API>) -> DemoKind {
        with_state!(ctx.run, Demo2DState, ctx.id, |state| state.runtime.active_demo)
    }

    fn sync_info_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let (overlay, active_demo) = with_state!(ctx.run, Demo2DState, ctx.id, |state| {
            (state.ui.info_overlay_root, state.runtime.active_demo)
        });
        if overlay.is_nil() {
            return;
        }
        let title = match active_demo {
            DemoKind::None => "Demo2D".to_string(),
            DemoKind::MeshMaterials => "Sprite Stress".to_string(),
            DemoKind::Lights => "Lights".to_string(),
            DemoKind::Water => "Water".to_string(),
            DemoKind::Animations => "Animations".to_string(),
            DemoKind::PhysicsBones => "Physics Bones".to_string(),
            DemoKind::PhysicsCollisions => "Physics Collisions".to_string(),
            DemoKind::SkyGap => "Reserved".to_string(),
            DemoKind::BlendGap => "Reserved".to_string(),
            DemoKind::MultiMesh => "Sprite Batch".to_string(),
            DemoKind::ParticlesGap => "Reserved".to_string(),
            DemoKind::AudioGap => "Reserved".to_string(),
        };
        let body = match active_demo {
            DemoKind::None => "pick lane frm hub".to_string(),
            DemoKind::MeshMaterials => "sprites 256/1024/4096".to_string(),
            DemoKind::Lights => "point 8 | spot 2 | ray 2".to_string(),
            DemoKind::Water => "water 3 | floaters 48".to_string(),
            DemoKind::Animations => "anim sprites 64/256/1024 | players 48".to_string(),
            DemoKind::PhysicsBones => "rigs 12 | bone chains 12".to_string(),
            DemoKind::PhysicsCollisions => "rigid 240".to_string(),
            DemoKind::SkyGap | DemoKind::BlendGap | DemoKind::ParticlesGap | DemoKind::AudioGap => {
                "gap lane".to_string()
            }
            DemoKind::MultiMesh => "dense retained sprite batch".to_string(),
        };
        let _ = call_method!(ctx.run, overlay, func!("set_content"), params![title, body]);
    }
});

fn palette_rgb(t: f32) -> [f32; 3] {
    let a = std::f32::consts::TAU * t;
    [
        0.5 + 0.5 * a.cos(),
        0.5 + 0.5 * (a + 2.094).cos(),
        0.5 + 0.5 * (a + 4.188).cos(),
    ]
}

fn scene_ui_parent<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    manager: NodeID,
) -> NodeID {
    let scene_root = get_node_parent_id!(ctx.run, manager).unwrap_or(manager);
    get_child!(ctx.run, scene_root, DEMO_UI_ROOT_NODE_NAME).unwrap_or(scene_root)
}

fn set_ui_tree_visible<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    visible: bool,
) {
    if root.is_nil() {
        return;
    }
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
            node.visible = visible;
        });
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.visible = visible;
            node.input_enabled = visible;
        });
        if let Some(children) = get_node_children_ids!(ctx.run, id) {
            for child in children {
                if !child.is_nil() {
                    stack.push(child);
                }
            }
        }
    }
    let _ = force_rerender!(ctx.run, root);
}

fn find_descendant_by_name<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
    name: &str,
) -> NodeID {
    if root.is_nil() {
        return NodeID::nil();
    }
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if let Some(hit) = get_child!(ctx.run, id, name) {
            return hit;
        }
        if let Some(children) = get_node_children_ids!(ctx.run, id) {
            for child in children {
                if !child.is_nil() {
                    stack.push(child);
                }
            }
        }
    }
    NodeID::nil()
}

fn demo_anchor(demo: DemoKind) -> Vector2 {
    match demo {
        DemoKind::None | DemoKind::MeshMaterials => Vector2::new(-1450.0, 650.0),
        DemoKind::Lights => Vector2::new(1450.0, 850.0),
        DemoKind::Water => Vector2::new(1500.0, 120.0),
        DemoKind::Animations => Vector2::new(-1750.0, -920.0),
        DemoKind::PhysicsBones => Vector2::new(100.0, -920.0),
        DemoKind::PhysicsCollisions => Vector2::new(1350.0, -920.0),
        DemoKind::SkyGap => Vector2::new(0.0, 1250.0),
        DemoKind::BlendGap => Vector2::new(650.0, 1250.0),
        DemoKind::MultiMesh => Vector2::new(-450.0, 900.0),
        DemoKind::ParticlesGap => Vector2::new(1300.0, 1250.0),
        DemoKind::AudioGap => Vector2::new(1950.0, 1250.0),
    }
}

fn color_with_alpha(base: &str, alpha: f32) -> Option<Color> {
    let byte = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_hex(&format!("{base}{byte:02X}"))
}
