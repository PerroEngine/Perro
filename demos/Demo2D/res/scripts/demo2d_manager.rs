use perro_api::prelude::*;
use std::time::Duration;

type SelfNodeType = Node2D;

const SPRITE_SHEET: &ResPath = res_path!("res://sprite_sheet.png");
const PERRO_LOGO: &ResPath = res_path!("res://perro.svg");
const HERO_SHEET: &ResPath = res_path!("res://hero_sheet.png");
const LIGHT_DISC: &ResPath = res_path!("res://light_disc.png");
const RIG_SCENE: &ResPath = res_path!("res://scenes/rig_actor.scn");
const ANIMATED_SPRITE_SCENE: &ResPath = res_path!("res://scenes/animated_sprite_actor.scn");
const PLAYER_BOB: &ResPath = res_path!("res://animations/player_bob.panim");
const MAIN_MENU_SCENE: &ResPath = res_path!("res://Menu/MainMenu.scn");
const PAUSE_MENU_SCENE: &ResPath = res_path!("res://Menu/PauseMenu.scn");
const TRANSITION_FADE_SCENE: &ResPath = res_path!("res://Menu/TransitionFade.scn");
const PROFILING_OVERLAY_SCENE: &ResPath = res_path!("res://Menu/ProfilingOverlay.scn");
const INFO_OVERLAY_SCENE: &ResPath = res_path!("res://Menu/InfoOverlay.scn");
const WEBCAM_SCENE: &ResPath = res_path!("res://scenes/webcam.scn");
const FPS_TESTER_SCENE: &ResPath = res_path!("res://scenes/fps_tester.scn");
const DEMO_UI_ROOT_NODE_NAME: &str = "demo_ui_root";
const CAMERA_NODE_NAME: &str = "Camera";
const TRANSITION_FADE_PANEL_NODE_NAME: &str = "transition_fade_panel";
const FADE_IN_SECONDS: f32 = 0.20;
const FADE_OUT_SECONDS: f32 = 0.22;
const FADE_COLOR: Color = color!("#000000");

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum DemoKind {
    #[default]
    None,
    MeshMaterials,
    Lights,
    Water,
    AnimatedSprites,
    Animations,
    PhysicsBones,
    PhysicsCollisions,
    SkyGap,
    BlendGap,
    MultiMesh,
    ParticlesGap,
    AudioGap,
    Webcam,
    FpsTester,
}

#[derive(Variant, Clone, Copy, Default)]
struct DemoAssets {
    rig_scene: PreloadedSceneID,
    animated_sprite_scene: PreloadedSceneID,
    player_bob: AnimationID,
    main_menu: PreloadedSceneID,
    pause_menu: PreloadedSceneID,
    fade: PreloadedSceneID,
    profiling_overlay: PreloadedSceneID,
    info_overlay: PreloadedSceneID,
    webcam: PreloadedSceneID,
    fps_tester: PreloadedSceneID,
}

#[derive(Variant, Clone, Default)]
struct DemoUiRefs {
    main_menu_root: NodeID,
    pause_menu_root: NodeID,
    fade_root: NodeID,
    fade_panel: NodeID,
    profiling_overlay_root: NodeID,
    info_overlay_root: NodeID,
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum FadeAction {
    #[default]
    None,
    ActivateDemo,
    ShowHub,
    RestartDemo,
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum FadePhase {
    #[default]
    Idle,
    FadeIn,
    FadeOut,
}

#[derive(Variant, Clone, Copy)]
struct DemoRuntimeState {
    active_demo: DemoKind,
    queued_demo: DemoKind,
    paused: bool,
    fade_alpha: f32,
    fade_active: bool,
    fade_phase: FadePhase,
    fade_action: FadeAction,
    audio_timer: f32,
    audio_debug: bool,
}

impl Default for DemoRuntimeState {
    fn default() -> Self {
        Self {
            active_demo: DemoKind::None,
            queued_demo: DemoKind::None,
            paused: false,
            fade_alpha: 0.0,
            fade_active: false,
            fade_phase: FadePhase::Idle,
            fade_action: FadeAction::None,
            audio_timer: 0.0,
            audio_debug: false,
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
            rig_scene: scene_preload!(ctx.run, RIG_SCENE).expect("preload rig scene"),
            animated_sprite_scene: scene_preload!(ctx.run, ANIMATED_SPRITE_SCENE)
                .expect("preload animated sprite scene"),
            player_bob: animation_load!(ctx.res, PLAYER_BOB),
            main_menu: scene_preload!(ctx.run, MAIN_MENU_SCENE).expect("preload main menu"),
            pause_menu: scene_preload!(ctx.run, PAUSE_MENU_SCENE).expect("preload pause menu"),
            fade: scene_preload!(ctx.run, TRANSITION_FADE_SCENE).expect("preload fade"),
            profiling_overlay: scene_preload!(ctx.run, PROFILING_OVERLAY_SCENE)
                .expect("preload profiling overlay"),
            info_overlay: scene_preload!(ctx.run, INFO_OVERLAY_SCENE)
                .expect("preload info overlay"),
            webcam: scene_preload!(ctx.run, WEBCAM_SCENE).expect("preload webcam demo"),
            fps_tester: scene_preload!(ctx.run, FPS_TESTER_SCENE).expect("preload fps tester"),
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

        if self.active_demo(ctx) == DemoKind::AudioGap {
            if key_pressed!(ctx.ipt, KeyCode::KeyT) {
                let debug = with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
                    state.runtime.audio_debug = !state.runtime.audio_debug;
                    state.runtime.audio_debug
                })
                .unwrap_or(false);
                ctx.run.Audio().set_debug_rays(debug);
                self.sync_info_overlay(ctx);
            }
            if !paused {
                self.update_audio_zone(ctx);
            }
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
        self.activate_demo(ctx, DemoKind::AnimatedSprites);
    }
    fn on_demo_sky_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::Animations);
    }
    fn on_demo_physics_bones_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::PhysicsBones);
    }
    fn on_demo_physics_collisions_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::PhysicsCollisions);
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
    fn on_demo_webcam_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::Webcam);
    }
    fn on_demo_fps_tester_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.activate_demo(ctx, DemoKind::FpsTester);
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
                self.spawn_static_sprite_zone(
                    ctx,
                    Vector2::new(-2400.0, 900.0),
                    16,
                    16,
                    34.0,
                    "static_256",
                );
                self.spawn_static_sprite_zone(
                    ctx,
                    Vector2::new(-1700.0, 900.0),
                    32,
                    32,
                    18.0,
                    "static_1024",
                );
                self.spawn_static_sprite_zone(
                    ctx,
                    Vector2::new(-450.0, 900.0),
                    64,
                    64,
                    10.0,
                    "static_4096",
                );
            }
            DemoKind::Lights => {
                self.spawn_light_zone(ctx, Vector2::new(1450.0, 850.0));
            }
            DemoKind::Water => {
                self.spawn_water_zone(ctx, Vector2::new(1500.0, 120.0));
            }
            DemoKind::AnimatedSprites => {
                self.spawn_animated_sprite_showcase(ctx, Vector2::new(-1450.0, 260.0));
            }
            DemoKind::Animations => {
                self.spawn_animation_player_zone(ctx, Vector2::new(-1750.0, -920.0));
            }
            DemoKind::PhysicsBones => {
                self.spawn_skeletal_zone(ctx, Vector2::new(100.0, -920.0));
            }
            DemoKind::PhysicsCollisions => {
                self.spawn_physics_zone(ctx, Vector2::new(1350.0, -920.0));
            }
            DemoKind::ParticlesGap => {
                self.spawn_particles_zone(ctx, Vector2::new(1300.0, 1250.0));
            }
            DemoKind::AudioGap => {
                self.spawn_audio_zone(ctx, Vector2::new(1950.0, 1250.0));
            }
            DemoKind::Webcam => {
                self.spawn_webcam_scene(ctx);
            }
            DemoKind::FpsTester => {
                self.spawn_fps_tester_scene(ctx);
            }
            DemoKind::SkyGap | DemoKind::BlendGap => {}
            DemoKind::MultiMesh => {
                self.spawn_static_sprite_zone(
                    ctx,
                    Vector2::new(-450.0, 900.0),
                    64,
                    64,
                    10.0,
                    "multimesh_2d",
                );
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
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.ui.main_menu_root = main_menu_root;
            state.ui.pause_menu_root = pause_menu_root;
            state.ui.fade_root = fade_root;
            state.ui.fade_panel = fade_panel;
            state.ui.profiling_overlay_root = profiling_overlay_root;
            state.ui.info_overlay_root = info_overlay_root;
        });
        self.apply_transition_fade(ctx, 0.0, false);
    }

    fn connect_ui_signals(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect_pairs!(
            ctx.run,
            ctx.id,
            [
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
                ("demo_webcam_click", "on_demo_webcam_click"),
                ("demo_fps_tester_click", "on_demo_fps_tester_click"),
                ("pause_resume_click", "on_pause_resume_click"),
                ("pause_restart_click", "on_pause_restart_click"),
                ("pause_hub_click", "on_pause_hub_click"),
            ]
        );
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
            state.runtime.fade_alpha = 0.0;
            state.runtime.fade_active = true;
            state.runtime.fade_phase = FadePhase::FadeIn;
        });
        self.apply_transition_fade(ctx, 0.0, true);
    }

    fn update_fade(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let Some((alpha, active, do_action)) =
            with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
                if !state.runtime.fade_active {
                    return (0.0, false, false);
                }
                let mut do_action = false;
                match state.runtime.fade_phase {
                    FadePhase::Idle => {
                        state.runtime.fade_active = false;
                    }
                    FadePhase::FadeIn => {
                        let step = if FADE_IN_SECONDS <= 0.0001 {
                            1.0
                        } else {
                            dt / FADE_IN_SECONDS
                        };
                        state.runtime.fade_alpha = (state.runtime.fade_alpha + step).min(1.0);
                        if state.runtime.fade_alpha >= 0.999 {
                            state.runtime.fade_alpha = 1.0;
                            state.runtime.fade_phase = FadePhase::FadeOut;
                            do_action = true;
                        }
                    }
                    FadePhase::FadeOut => {
                        let step = if FADE_OUT_SECONDS <= 0.0001 {
                            1.0
                        } else {
                            dt / FADE_OUT_SECONDS
                        };
                        state.runtime.fade_alpha = (state.runtime.fade_alpha - step).max(0.0);
                        if state.runtime.fade_alpha <= 0.001 {
                            state.runtime.fade_alpha = 0.0;
                            state.runtime.fade_active = false;
                            state.runtime.fade_phase = FadePhase::Idle;
                            state.runtime.fade_action = FadeAction::None;
                        }
                    }
                }
                (
                    state.runtime.fade_alpha,
                    state.runtime.fade_active,
                    do_action,
                )
            })
        else {
            return;
        };
        if do_action {
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
        let color = FADE_COLOR.with_alpha(clamped);
        for id in [root, panel] {
            if id.is_nil() {
                continue;
            }
            let _ = with_node_mut!(ctx.run, UiPanel, id, |node| {
                node.style.fill = color;
                node.style.stroke = color;
                node.style.stroke_width = 0.0;
                node.visible = show;
                node.input_enabled = false;
            });
        }
    }

    fn jump_camera_to_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        let camera = find_node!(ctx.run, ctx.id, CAMERA_NODE_NAME).unwrap_or(NodeID::nil());
        if camera.is_nil() {
            return;
        }
        let target = demo_anchor(demo);
        let _ = set_local_pos_2d!(ctx.run, camera, target);
        let _ = with_node_mut!(ctx.run, Camera2D, camera, |cam| {
            cam.zoom = 1.0;
        });
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
                    state.runtime.queued_demo = DemoKind::None;
                    state.runtime.paused = false;
                });
                self.rebuild(ctx, demo);
                self.jump_camera_to_demo(ctx, demo);
                physics_pause!(ctx.run, false);
            }
            FadeAction::ShowHub => {
                with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
                    state.runtime.active_demo = DemoKind::None;
                    state.runtime.queued_demo = DemoKind::None;
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
        let (menu, pause, profiling, info, active_demo, paused) =
            with_state!(ctx.run, Demo2DState, ctx.id, |state| {
                (
                    state.ui.main_menu_root,
                    state.ui.pause_menu_root,
                    state.ui.profiling_overlay_root,
                    state.ui.info_overlay_root,
                    state.runtime.active_demo,
                    state.runtime.paused,
                )
            });
        let in_hub = active_demo == DemoKind::None;
        set_ui_tree_visible(ctx, menu, in_hub);
        set_ui_tree_visible(ctx, pause, paused);
        set_ui_tree_visible(ctx, profiling, true);
        set_ui_tree_visible(ctx, info, true);
        self.sync_info_overlay(ctx);
    }

    fn clear_dynamic(&self, ctx: &mut ScriptContext<'_, API>) {
        ctx.run.Audio().set_debug_rays(false);
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.audio_timer = 0.0;
            state.runtime.audio_debug = false;
        });
        for node in query!(ctx.run, all(tags["demo2d_dynamic"]), in_subtree(ctx.id)) {
            let _ = remove_node!(ctx.run, node);
        }
    }

    fn spawn_fps_tester_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let scene = self.assets(ctx).fps_tester;
        let Ok(root) = scene_load!(ctx.run, scene) else {
            log_error!("[Demo2D] fps tester load fail");
            return;
        };
        reparent!(ctx.run, ctx.id, root);
        tag_add!(ctx.run, root, tags!["demo2d_dynamic"]);
    }

    fn spawn_webcam_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let scene = self.assets(ctx).webcam;
        let Ok(root) = scene_load!(ctx.run, scene) else {
            log_error!("[Demo2D] webcam demo load fail");
            return;
        };
        reparent!(ctx.run, ctx.id, root);
        tag_add!(ctx.run, root, tags!["demo2d_dynamic"]);
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
        let tex = texture_load!(ctx.res, SPRITE_SHEET);
        let logo = texture_load!(ctx.res, PERRO_LOGO);
        for y in 0..rows {
            for x in 0..cols {
                let idx = ((y * cols + x) % 64) as f32;
                let use_logo = (x + y * cols) % 17 == 0;
                let px = origin.x + x as f32 * step;
                let py = origin.y - y as f32 * step;
                let _ = spawn!(
                    ctx.run,
                    Sprite2D,
                    "static_sprite",
                    tags!["demo2d_dynamic"],
                    ctx.id,
                    |sprite| {
                        if use_logo {
                            sprite.texture = logo;
                            sprite.texture_region = Some([0.0, 0.0, 1197.96, 1018.47]);
                            sprite.transform.scale = Vector2::new(0.026, 0.026);
                        } else {
                            sprite.texture = tex;
                            sprite.texture_region = Some([
                                32.0 * (idx % 8.0),
                                32.0 * (idx / 8.0).floor(),
                                32.0,
                                32.0,
                            ]);
                            sprite.transform.scale = Vector2::new(0.55, 0.55);
                        }
                        sprite.transform.position = Vector2::new(px, py);
                    }
                );
            }
        }
    }

    fn spawn_animated_sprite_showcase(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let scene = self.assets(ctx).animated_sprite_scene;
        let defs = [
            ("idle", Vector2::new(-220.0, 40.0), 2.8),
            ("run", Vector2::new(0.0, 40.0), 2.8),
            ("hurt_grid", Vector2::new(220.0, 40.0), 2.8),
        ];
        for (name, offset, scale) in defs {
            let Ok(node) = scene_load!(ctx.run, scene) else {
                continue;
            };
            reparent!(ctx.run, ctx.id, node);
            tag_add!(ctx.run, node, tags!["demo2d_dynamic"]);
            let _ = with_node_mut!(ctx.run, AnimatedSprite2D, node, |sprite| {
                sprite.current_animation = name.into();
                sprite.current_frame = 0;
                sprite.fps_scale = 1.0;
                sprite.playing = true;
                sprite.looping = true;
                sprite.transform.position = origin + offset;
                sprite.transform.scale = Vector2::new(scale, scale);
            });
        }
    }

    fn spawn_light_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        self.spawn_static_sprite_zone(
            ctx,
            origin + Vector2::new(-360.0, 120.0),
            16,
            16,
            22.0,
            "light_bg",
        );

        let disc = texture_load!(ctx.res, LIGHT_DISC);
        for i in 0..8 {
            let x = origin.x - 280.0 + i as f32 * 80.0;
            let y = origin.y + if i % 2 == 0 { 100.0 } else { -120.0 };
            let hue = i as f32 / 8.0;
            let _ = spawn!(
                ctx.run,
                PointLight2D,
                "point_light",
                tags!["demo2d_dynamic"],
                ctx.id,
                |light| {
                    light.color = palette_rgb(hue);
                    light.intensity = 2.1;
                    light.range = 180.0;
                    light.transform.position = Vector2::new(x, y);
                }
            );
            self.spawn_marker_sprite(ctx, disc, Vector2::new(x, y), 1.5);
        }

        for i in 0..2 {
            let _ = spawn!(
                ctx.run,
                SpotLight2D,
                "spot_light",
                tags!["demo2d_dynamic"],
                ctx.id,
                |light| {
                    light.color = if i == 0 {
                        Color::rgb(1.0, 0.9, 0.45)
                    } else {
                        Color::rgb(0.45, 0.95, 1.0)
                    };
                    light.intensity = 2.6;
                    light.range = 260.0;
                    light.inner_angle_radians = 0.25;
                    light.outer_angle_radians = 0.75;
                    light.transform.position =
                        origin + Vector2::new(-100.0 + i as f32 * 220.0, 240.0);
                    light.transform.rotation = if i == 0 { -1.2 } else { -1.9 };
                }
            );
        }

        for i in 0..2 {
            let _ = spawn!(
                ctx.run,
                RayLight2D,
                "ray_light",
                tags!["demo2d_dynamic"],
                ctx.id,
                |light| {
                    light.color = if i == 0 {
                        Color::rgb(1.0, 0.35, 0.5)
                    } else {
                        Color::rgb(0.55, 1.0, 0.45)
                    };
                    light.intensity = 2.0;
                    light.transform.position =
                        origin + Vector2::new(-250.0 + i as f32 * 500.0, -260.0);
                    light.transform.rotation = if i == 0 { 0.4 } else { -0.4 };
                }
            );
        }
    }

    fn spawn_water_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        for pool in 0..3 {
            let water = create_node!(
                ctx.run,
                WaterBody2D,
                "water",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let base = origin + Vector2::new(pool as f32 * 240.0 - 240.0, 0.0);
            let _ = with_node_mut!(ctx.run, WaterBody2D, water, |node| {
                node.transform.position = base;
                node.water.shape = WaterShape::rect(Vector2::new(170.0, 90.0));
                node.water.resolution = [96, 48];
                node.water.render_resolution = [96, 48];
                node.water.flow = Vector2::new(0.3 * (pool as f32 - 1.0), 0.0);
                node.water.wind = Vector2::new(1.0, 0.0);
                node.water.optics.deep_color = [
                    0.02,
                    0.15 + pool as f32 * 0.08,
                    0.26 + pool as f32 * 0.06,
                    0.96,
                ]
                .into();
                node.water.optics.shallow_color = [0.12, 0.48, 0.72, 0.72].into();
                node.water.visual.foam_color = [0.86, 0.96, 1.0, 1.0].into();
                node.water.physics.buoyancy = 2.8;
                node.water.physics.drag = 0.7;
            });

            let floor = create_node!(
                ctx.run,
                StaticBody2D,
                "water_floor",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let _ = with_node_mut!(ctx.run, StaticBody2D, floor, |body| {
                body.transform.position = base + Vector2::new(0.0, -70.0);
            });
            let shape = create_node!(
                ctx.run,
                CollisionShape2D,
                "water_floor_shape",
                tags!["demo2d_dynamic"],
                floor
            );
            let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                s.shape = Shape2D::Quad {
                    width: 180.0,
                    height: 16.0,
                };
            });

            for i in 0..16 {
                let body = create_node!(
                    ctx.run,
                    RigidBody2D,
                    "floater",
                    tags!["demo2d_dynamic"],
                    ctx.id
                );
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
                let shape = create_node!(
                    ctx.run,
                    CollisionShape2D,
                    "floater_shape",
                    tags!["demo2d_dynamic"],
                    body
                );
                let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                    s.shape = Shape2D::Quad {
                        width: 20.0,
                        height: 20.0,
                    };
                });
                let sprite_sheet = texture_load!(ctx.res, SPRITE_SHEET);
                self.spawn_child_sprite(
                    ctx,
                    body,
                    sprite_sheet,
                    32.0 * ((i % 8) as f32),
                    64.0,
                    32.0,
                    32.0,
                    0.65,
                );
            }
        }
    }

    fn spawn_physics_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let ground = create_node!(
            ctx.run,
            StaticBody2D,
            "ground",
            tags!["demo2d_dynamic"],
            ctx.id
        );
        let _ = with_node_mut!(ctx.run, StaticBody2D, ground, |body| {
            body.transform.position = origin + Vector2::new(0.0, -40.0);
        });
        let ground_shape = create_node!(
            ctx.run,
            CollisionShape2D,
            "ground_shape",
            tags!["demo2d_dynamic"],
            ground
        );
        let _ = with_node_mut!(ctx.run, CollisionShape2D, ground_shape, |shape| {
            shape.shape = Shape2D::Quad {
                width: 860.0,
                height: 24.0,
            };
        });

        for i in 0..240 {
            let body = create_node!(
                ctx.run,
                RigidBody2D,
                "crate",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let col = (i % 20) as f32;
            let row = (i / 20) as f32;
            let _ = with_node_mut!(ctx.run, RigidBody2D, body, |rb| {
                rb.transform.position =
                    origin + Vector2::new(-350.0 + col * 36.0, row * 34.0 + 10.0);
                rb.angular_velocity = (i % 3) as f32 * 0.15;
                rb.friction = 0.6;
                rb.restitution = 0.08;
            });
            let shape = create_node!(
                ctx.run,
                CollisionShape2D,
                "crate_shape",
                tags!["demo2d_dynamic"],
                body
            );
            let _ = with_node_mut!(ctx.run, CollisionShape2D, shape, |s| {
                s.shape = if i % 5 == 0 {
                    Shape2D::Circle { radius: 12.0 }
                } else {
                    Shape2D::Quad {
                        width: 24.0,
                        height: 24.0,
                    }
                };
            });
            let sprite_sheet = texture_load!(ctx.res, SPRITE_SHEET);
            self.spawn_child_sprite(
                ctx,
                body,
                sprite_sheet,
                32.0 * ((i % 8) as f32),
                96.0,
                32.0,
                32.0,
                0.7,
            );
        }
    }

    fn spawn_animation_player_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        let clip = self.assets(ctx).player_bob;
        let tex = texture_load!(ctx.res, HERO_SHEET);
        for i in 0..48 {
            let actor = create_node!(
                ctx.run,
                Node2D,
                "actor_root",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let col = (i % 12) as f32;
            let row = (i / 12) as f32;
            let _ = with_base_node_mut!(ctx.run, Node2D, actor, |node| {
                node.transform.position = origin + Vector2::new(-330.0 + col * 62.0, row * 84.0);
            });

            let sprite = create_node!(
                ctx.run,
                Sprite2D,
                "actor_sprite",
                tags!["demo2d_dynamic"],
                actor
            );
            let _ = with_node_mut!(ctx.run, Sprite2D, sprite, |node| {
                node.texture = tex;
                node.texture_region = Some([32.0 * (i % 4) as f32, 0.0, 32.0, 32.0]);
                node.transform.scale = Vector2::new(1.0, 1.0);
            });

            let player = create_node!(
                ctx.run,
                AnimationPlayer,
                "actor_player",
                tags!["demo2d_dynamic"],
                ctx.id
            );
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
            let Ok(root) = scene_load!(ctx.run, scene) else {
                continue;
            };
            reparent!(ctx.run, ctx.id, root);
            tag_add!(ctx.run, root, tags!["demo2d_dynamic"]);
            let col = (i % 4) as f32;
            let row = (i / 4) as f32;
            let pos = origin + Vector2::new(-260.0 + col * 180.0, row * 170.0);
            let _ = set_local_pos_2d!(ctx.run, root, pos);
            let _ = set_local_scale_2d!(ctx.run, root, Vector2::new(1.5, 1.5));
        }
    }

    fn spawn_particles_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        self.spawn_static_sprite_zone(
            ctx,
            origin + Vector2::new(-320.0, 120.0),
            14,
            10,
            26.0,
            "particle_bg",
        );
        let disc = texture_load!(ctx.res, LIGHT_DISC);
        for i in 0..4 {
            let node = create_node!(
                ctx.run,
                ParticleEmitter2D,
                "particles",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let pos = origin
                + match i {
                    0 => Vector2::new(-220.0, 90.0),
                    1 => Vector2::new(-40.0, 0.0),
                    2 => Vector2::new(150.0, 100.0),
                    _ => Vector2::new(310.0, -20.0),
                };
            let profile = match i {
                0 => "inline://preset = spiral\npreset_param_a = 2.6\npreset_param_b = 18.0\nlifetime_min = 1.0\nlifetime_max = 1.4\nspeed_min = 12.0\nspeed_max = 24.0\nsize_min = 6.0\nsize_max = 12.0\ncolor_start = (1.0, 0.6, 0.2, 1.0)\ncolor_end = (0.9, 0.1, 0.0, 0.0)".to_string(),
                1 => "inline://preset = ballistic\nforce = (0.0, -55.0)\nlifetime_min = 0.8\nlifetime_max = 1.1\nspeed_min = 40.0\nspeed_max = 88.0\nspread_radians = 0.5\nsize_min = 4.0\nsize_max = 9.0\ncolor_start = (0.4, 0.9, 1.0, 1.0)\ncolor_end = (0.1, 0.3, 1.0, 0.0)".to_string(),
                2 => "inline://preset = noise_drift\npreset_param_a = 22.0\npreset_param_b = 1.8\nlifetime_min = 1.6\nlifetime_max = 2.3\nspeed_min = 8.0\nspeed_max = 18.0\nsize_min = 8.0\nsize_max = 15.0\ncolor_start = (0.8, 1.0, 0.9, 0.7)\ncolor_end = (0.8, 1.0, 0.9, 0.0)".to_string(),
                _ => "inline://preset = flat_disk\nlifetime_min = 0.6\nlifetime_max = 0.9\nspeed_min = 26.0\nspeed_max = 44.0\nsize_min = 5.0\nsize_max = 10.0\ncolor_start = (1.0, 0.95, 0.5, 1.0)\ncolor_end = (1.0, 0.3, 0.1, 0.0)".to_string(),
            };
            let _ = with_node_mut!(ctx.run, ParticleEmitter2D, node, |emitter| {
                emitter.transform.position = pos;
                emitter.spawn_rate = 180.0 + i as f32 * 70.0;
                emitter.seed = 10 + i as u32;
                emitter.profile = profile.clone().into();
                emitter.prewarm = true;
            });
            self.spawn_marker_sprite(ctx, disc, pos, 1.0 + i as f32 * 0.2);
        }
    }

    fn spawn_audio_zone(&self, ctx: &mut ScriptContext<'_, API>, origin: Vector2) {
        self.spawn_static_sprite_zone(
            ctx,
            origin + Vector2::new(-280.0, 100.0),
            12,
            8,
            28.0,
            "audio_bg",
        );
        let disc = texture_load!(ctx.res, LIGHT_DISC);
        ctx.run.Audio().set_debug_rays(true);
        with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.audio_timer = 0.0;
            state.runtime.audio_debug = true;
        });
        for i in 0..3 {
            let pos =
                origin + Vector2::new(-180.0 + i as f32 * 180.0, 40.0 + (i % 2) as f32 * 80.0);
            let mask = create_node!(
                ctx.run,
                AudioMask2D,
                "audio_mask",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let _ = with_node_mut!(ctx.run, AudioMask2D, mask, |node| {
                node.transform.position = pos;
                node.material.transmission = 0.12 + i as f32 * 0.1;
                node.material.low_pass_strength = 0.52 + i as f32 * 0.12;
                node.material.reflection = 0.16 + i as f32 * 0.05;
                node.material.thickness_multiplier = 0.75 + i as f32 * 0.2;
            });
            self.spawn_audio_quad_shape(ctx, mask, 70.0, 24.0);
            let zone = create_node!(
                ctx.run,
                AudioEffectZone2D,
                "audio_zone",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let _ = with_node_mut!(ctx.run, AudioEffectZone2D, zone, |node| {
                node.transform.position = pos + Vector2::new(0.0, -70.0);
                node.bounce = i % 2 == 0;
                node.effects = vec![AudioEffect {
                    reverb_send: 0.25 + i as f32 * 0.15,
                    echo: 0.05 + i as f32 * 0.08,
                    dampening: 0.1 + i as f32 * 0.15,
                }];
            });
            self.spawn_audio_circle_shape(ctx, zone, 64.0 + i as f32 * 18.0);
            let speaker = self.spawn_marker_sprite(ctx, disc, pos, 1.8);
            tag_add!(ctx.run, speaker, tags!["demo2d_audio_speaker"]);
            self.spawn_ring(ctx, pos, 56.0 + i as f32 * 18.0, 18 + i * 6);
        }
        self.sync_info_overlay(ctx);
    }

    fn spawn_audio_quad_shape(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        parent: NodeID,
        width: f32,
        height: f32,
    ) {
        let node = create_node!(
            ctx.run,
            CollisionShape2D,
            "audio_shape",
            tags!["demo2d_dynamic"],
            parent
        );
        let _ = with_node_mut!(ctx.run, CollisionShape2D, node, |node| {
            node.shape = Shape2D::Quad { width, height };
        });
    }

    fn spawn_audio_circle_shape(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        parent: NodeID,
        radius: f32,
    ) {
        let node = create_node!(
            ctx.run,
            CollisionShape2D,
            "audio_shape",
            tags!["demo2d_dynamic"],
            parent
        );
        let _ = with_node_mut!(ctx.run, CollisionShape2D, node, |node| {
            node.shape = Shape2D::Circle { radius };
        });
    }

    fn update_audio_zone(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let play = with_state_mut!(ctx.run, Demo2DState, ctx.id, |state| {
            state.runtime.audio_timer -= dt;
            if state.runtime.audio_timer <= 0.0 {
                state.runtime.audio_timer = 0.38;
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
        if !play {
            return;
        }

        let opts = MidiNoteOptions {
            velocity: 76,
            sustain: Duration::from_millis(520),
            program: program::Piano::Electric1,
            volume: 0.42,
            ..MidiNoteOptions::default()
        };
        let spatial = SpatialAudioOptions {
            range: 520.0,
            audio_layer: BitMask::ALL,
            enable_propagation: true,
            direction_2d: AudioDirection::Omni,
            direction_3d: AudioDirection::Omni,
        };
        for (i, speaker) in query!(
            ctx.run,
            all(tags["demo2d_audio_speaker"]),
            in_subtree(ctx.id)
        )
        .into_iter()
        .enumerate()
        {
            let note = match i % 3 {
                0 => Note::C4,
                1 => Note::E4,
                _ => Note::G4,
            };
            let opts = MidiNoteOptions {
                velocity: 62 + i as u8 * 10,
                ..opts
            };
            let _ = ctx
                .run
                .Audio()
                .midi()
                .play_note_attached(note, speaker, opts, spatial);
        }
    }

    fn spawn_ring(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        center: Vector2,
        radius: f32,
        count: i32,
    ) {
        let disc = texture_load!(ctx.res, LIGHT_DISC);
        for i in 0..count {
            let t = i as f32 / count as f32;
            let a = t * std::f32::consts::TAU;
            let pos = center + Vector2::new(a.cos() * radius, a.sin() * radius);
            let node = create_node!(
                ctx.run,
                Sprite2D,
                "audio_dot",
                tags!["demo2d_dynamic"],
                ctx.id
            );
            let _ = with_node_mut!(ctx.run, Sprite2D, node, |sprite| {
                sprite.texture = disc;
                sprite.transform.position = pos;
                sprite.transform.scale = Vector2::new(0.18, 0.18);
            });
        }
    }

    fn spawn_marker_sprite(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        tex: TextureID,
        pos: Vector2,
        scale: f32,
    ) -> NodeID {
        let node = create_node!(
            ctx.run,
            Sprite2D,
            "light_marker",
            tags!["demo2d_dynamic"],
            ctx.id
        );
        let _ = with_node_mut!(ctx.run, Sprite2D, node, |sprite| {
            sprite.texture = tex;
            sprite.transform.position = pos;
            sprite.transform.scale = Vector2::new(scale, scale);
        });
        node
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
        let sprite = create_node!(
            ctx.run,
            Sprite2D,
            "child_sprite",
            tags!["demo2d_dynamic"],
            parent
        );
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
        with_state!(ctx.run, Demo2DState, ctx.id, |state| state
            .runtime
            .active_demo)
    }

    fn sync_info_overlay(&self, ctx: &mut ScriptContext<'_, API>) {
        let (overlay, active_demo, audio_debug) =
            with_state!(ctx.run, Demo2DState, ctx.id, |state| {
                (
                    state.ui.info_overlay_root,
                    state.runtime.active_demo,
                    state.runtime.audio_debug,
                )
            });
        if overlay.is_nil() {
            return;
        }
        let title = match active_demo {
            DemoKind::None => "Demo2D".to_string(),
            DemoKind::MeshMaterials => "Sprite Stress".to_string(),
            DemoKind::Lights => "Lights".to_string(),
            DemoKind::Water => "Water".to_string(),
            DemoKind::AnimatedSprites => "Animated Sprites".to_string(),
            DemoKind::Animations => "Animations".to_string(),
            DemoKind::PhysicsBones => "Physics Bones".to_string(),
            DemoKind::PhysicsCollisions => "Physics Collisions".to_string(),
            DemoKind::SkyGap => "Reserved".to_string(),
            DemoKind::BlendGap => "Reserved".to_string(),
            DemoKind::MultiMesh => "Sprite Batch".to_string(),
            DemoKind::ParticlesGap => "Particles".to_string(),
            DemoKind::AudioGap => "Audio".to_string(),
            DemoKind::Webcam => "Webcam Stream".to_string(),
            DemoKind::FpsTester => "FPS Tester".to_string(),
        };
        let active_anim_sprites = query!(
            ctx.run,
            all(node_type[AnimatedSprite2D]),
            in_subtree(ctx.id)
        )
        .len();
        let body = match active_demo {
            DemoKind::None => "pick lane frm hub".to_string(),
            DemoKind::MeshMaterials => "sprites 256/1024/4096".to_string(),
            DemoKind::Lights => "point 8 | spot 2 | ray 2".to_string(),
            DemoKind::Water => "water 3 | floaters 48".to_string(),
            DemoKind::AnimatedSprites => format!("active animated sprites {}", active_anim_sprites),
            DemoKind::Animations => "transform clips | players 48".to_string(),
            DemoKind::PhysicsBones => "rigs 12 | bone chains 12".to_string(),
            DemoKind::PhysicsCollisions => "rigid 240".to_string(),
            DemoKind::ParticlesGap => "emitters 4 | mixed cpu particles".to_string(),
            DemoKind::AudioGap => format!(
                "speakers 3 | masks 3 | fx zones 3 | rays {} | T toggle",
                if audio_debug { "on" } else { "off" }
            ),
            DemoKind::Webcam => "Webcam node -> CameraStream2D".to_string(),
            DemoKind::FpsTester => "cap btns | render tick vs cap tick".to_string(),
            DemoKind::SkyGap | DemoKind::BlendGap => "gap lane".to_string(),
            DemoKind::MultiMesh => "dense retained sprite batch".to_string(),
        };
        let _ = call_method!(ctx.run, overlay, func!("set_content"), params![title, body]);
    }
});

fn palette_rgb(t: f32) -> Color {
    let a = std::f32::consts::TAU * t;
    Color::rgb(
        0.5 + 0.5 * a.cos(),
        0.5 + 0.5 * (a + 2.094).cos(),
        0.5 + 0.5 * (a + 4.188).cos(),
    )
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
    set_tree_visible!(ctx.run, root, visible);
    for id in descendants!(ctx.run, root) {
        let _ = with_node_mut!(ctx.run, UiButton, id, |node| {
            node.input_enabled = visible;
        });
    }
    let _ = force_rerender!(ctx.run, root);
}

fn demo_anchor(demo: DemoKind) -> Vector2 {
    match demo {
        DemoKind::None | DemoKind::MeshMaterials => Vector2::new(-1450.0, 650.0),
        DemoKind::Lights => Vector2::new(1450.0, 850.0),
        DemoKind::Water => Vector2::new(1500.0, 120.0),
        DemoKind::AnimatedSprites => Vector2::new(-1450.0, 260.0),
        DemoKind::Animations => Vector2::new(-1750.0, -920.0),
        DemoKind::PhysicsBones => Vector2::new(0.0, -780.0),
        DemoKind::PhysicsCollisions => Vector2::new(1350.0, -520.0),
        DemoKind::SkyGap => Vector2::new(0.0, 1250.0),
        DemoKind::BlendGap => Vector2::new(650.0, 1250.0),
        DemoKind::MultiMesh => Vector2::new(-450.0, 900.0),
        DemoKind::ParticlesGap => Vector2::new(1300.0, 1250.0),
        DemoKind::AudioGap => Vector2::new(1950.0, 1250.0),
        DemoKind::Webcam => Vector2::new(0.0, 0.0),
        DemoKind::FpsTester => Vector2::new(0.0, 0.0),
    }
}
