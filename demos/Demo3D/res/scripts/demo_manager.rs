use perro_api::prelude::*;

type SelfNodeType = Node3D;

const MAIN_MENU_SCENE_PATH: &ResPath = res_path!("res://Menu/MainMenu.scn");
const PAUSE_MENU_SCENE_PATH: &ResPath = res_path!("res://Menu/PauseMenu.scn");
const TRANSITION_FADE_SCENE_PATH: &ResPath = res_path!("res://Menu/TransitionFade.scn");
const PROFILING_OVERLAY_SCENE_PATH: &ResPath = res_path!("res://Menu/ProfilingOverlay.scn");
const MESH_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/mesh_materials.scn");
const LIGHTS_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/lights.scn");
const WATER_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/water.scn");
const ANIMATION_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/animations.scn");
const PHYSICS_BONES_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/physics_bones.scn");
const PHYSICS_COLLISIONS_DEMO_SCENE_PATH: &ResPath =
    res_path!("res://scenes/demos/physics_collisions.scn");
const SKY_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/sky.scn");
const BLEND_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/mesh_blending.scn");
const MULTIMESH_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/multimesh.scn");
const PARTICLES_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/particles.scn");
const POSITIONAL_AUDIO_DEMO_SCENE_PATH: &ResPath =
    res_path!("res://scenes/demos/positional_audio.scn");

const DEMO_UI_ROOT_NODE_NAME: &str = "demo_ui_root";
const HUB_MENU_PANEL_NODE_NAME: &str = "hub_menu_panel";
const HUB_MENU_CONTENT_NODE_NAME: &str = "hub_menu_content";
const DEMO_BUTTON_MESH_NODE_NAME: &str = "demo_btn_mesh";
const DEMO_BUTTON_LIGHTS_NODE_NAME: &str = "demo_btn_lights";
const DEMO_BUTTON_WATER_NODE_NAME: &str = "demo_btn_water";
const DEMO_BUTTON_ANIMATIONS_NODE_NAME: &str = "demo_btn_animations";
const DEMO_BUTTON_PHYSICS_BONES_NODE_NAME: &str = "demo_btn_physics_bones";
const DEMO_BUTTON_PHYSICS_COLLISIONS_NODE_NAME: &str = "demo_btn_physics_collisions";
const DEMO_BUTTON_SKY_NODE_NAME: &str = "demo_btn_sky";
const DEMO_BUTTON_BLEND_NODE_NAME: &str = "demo_btn_blend";
const DEMO_BUTTON_MULTIMESH_NODE_NAME: &str = "demo_btn_multimesh";
const DEMO_BUTTON_PARTICLES_NODE_NAME: &str = "demo_btn_particles";
const DEMO_BUTTON_AUDIO_NODE_NAME: &str = "demo_btn_audio";
const PAUSE_PANEL_NODE_NAME: &str = "pause_panel";
const PAUSE_CONTENT_NODE_NAME: &str = "pause_content";
const PAUSE_TITLE_NODE_NAME: &str = "pause_title";
const PAUSE_SENS_ROW_NODE_NAME: &str = "pause_sens_row";
const PAUSE_SENS_LABEL_NODE_NAME: &str = "pause_sens_label";
const DEMO_CAMERA_NODE_NAME: &str = "DemoCamera";
const PAUSE_BUTTON_SENS_DOWN_NODE_NAME: &str = "pause_btn_sens_down";
const PAUSE_BUTTON_SENS_UP_NODE_NAME: &str = "pause_btn_sens_up";
const PAUSE_BUTTON_RESUME_NODE_NAME: &str = "pause_btn_resume";
const PAUSE_BUTTON_RESTART_NODE_NAME: &str = "pause_btn_restart";
const PAUSE_BUTTON_HUB_NODE_NAME: &str = "pause_btn_hub";
const TRANSITION_FADE_PANEL_NODE_NAME: &str = "transition_fade_panel";

const DEFAULT_MOUSE_SENSITIVITY: f32 = 0.00012;
const MIN_MOUSE_SENSITIVITY: f32 = 0.00004;
const MAX_MOUSE_SENSITIVITY: f32 = 0.00030;
const MOUSE_SENSITIVITY_STEP: f32 = 0.00002;
const DEFAULT_FREECAM_SPEED: f32 = 8.0;
const MIN_FREECAM_SPEED: f32 = 1.0;
const MAX_FREECAM_SPEED: f32 = 48.0;
const FREECAM_SPEED_STEP: f32 = 1.5;
const FADE_IN_SECONDS: f32 = 0.35;
const FADE_HOLD_SECONDS: f32 = 0.20;
const FADE_OUT_SECONDS: f32 = 0.35;
const PAUSE_FADE_IN_SECONDS: f32 = 0.14;
const PAUSE_FADE_OUT_SECONDS: f32 = 0.12;
const PAUSE_BG_MAX_ALPHA: f32 = 0.75;
const PAUSE_PANEL_ALPHA: f32 = 0.92;

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
    Sky,
    MeshBlending,
    MultiMesh,
    Particles,
    PositionalAudio,
}

impl DemoKind {
    fn path(self) -> Option<&'static ResPath> {
        match self {
            DemoKind::None => None,
            DemoKind::MeshMaterials => Some(MESH_DEMO_SCENE_PATH),
            DemoKind::Lights => Some(LIGHTS_DEMO_SCENE_PATH),
            DemoKind::Water => Some(WATER_DEMO_SCENE_PATH),
            DemoKind::Animations => Some(ANIMATION_DEMO_SCENE_PATH),
            DemoKind::PhysicsBones => Some(PHYSICS_BONES_DEMO_SCENE_PATH),
            DemoKind::PhysicsCollisions => Some(PHYSICS_COLLISIONS_DEMO_SCENE_PATH),
            DemoKind::Sky => Some(SKY_DEMO_SCENE_PATH),
            DemoKind::MeshBlending => Some(BLEND_DEMO_SCENE_PATH),
            DemoKind::MultiMesh => Some(MULTIMESH_DEMO_SCENE_PATH),
            DemoKind::Particles => Some(PARTICLES_DEMO_SCENE_PATH),
            DemoKind::PositionalAudio => Some(POSITIONAL_AUDIO_DEMO_SCENE_PATH),
        }
    }
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum DemoMode {
    #[default]
    Hub,
    DemoActive,
    Paused,
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum FadePhase {
    #[default]
    Idle,
    FadeIn,
    Hold,
    FadeOut,
}

#[derive(Variant, Clone, Copy, PartialEq, Eq, Default)]
enum FadeAction {
    #[default]
    None,
    LoadDemo,
    RestartDemo,
    BackToHub,
}

#[derive(Variant, Clone, Copy)]
struct DemoScenesState {
    pub main_menu: PreloadedSceneID,
    pub pause_menu: PreloadedSceneID,
    pub fade: PreloadedSceneID,
    pub profiling_overlay: PreloadedSceneID,
    pub mesh: PreloadedSceneID,
    pub lights: PreloadedSceneID,
    pub water: PreloadedSceneID,
    pub animations: PreloadedSceneID,
    pub physics_bones: PreloadedSceneID,
    pub physics_collisions: PreloadedSceneID,
    pub sky: PreloadedSceneID,
    pub blend: PreloadedSceneID,
    pub multimesh: PreloadedSceneID,
    pub particles: PreloadedSceneID,
    pub positional_audio: PreloadedSceneID,
}

impl Default for DemoScenesState {
    fn default() -> Self {
        Self {
            main_menu: PreloadedSceneID::nil(),
            pause_menu: PreloadedSceneID::nil(),
            fade: PreloadedSceneID::nil(),
            profiling_overlay: PreloadedSceneID::nil(),
            mesh: PreloadedSceneID::nil(),
            lights: PreloadedSceneID::nil(),
            water: PreloadedSceneID::nil(),
            animations: PreloadedSceneID::nil(),
            physics_bones: PreloadedSceneID::nil(),
            physics_collisions: PreloadedSceneID::nil(),
            sky: PreloadedSceneID::nil(),
            blend: PreloadedSceneID::nil(),
            multimesh: PreloadedSceneID::nil(),
            particles: PreloadedSceneID::nil(),
            positional_audio: PreloadedSceneID::nil(),
        }
    }
}

#[derive(Variant, Clone)]
struct DemoRefsState {
    pub main_menu_root: NodeID,
    pub pause_menu_root: NodeID,
    pub fade_root: NodeID,
    pub fade_panel: NodeID,
    pub profiling_overlay_root: NodeID,
    pub active_demo_root: NodeID,
    pub pause_sens_label: NodeID,
    pub hub_buttons: Vec<NodeID>,
    pub pause_buttons: Vec<NodeID>,
}

impl Default for DemoRefsState {
    fn default() -> Self {
        Self {
            main_menu_root: NodeID::nil(),
            pause_menu_root: NodeID::nil(),
            fade_root: NodeID::nil(),
            fade_panel: NodeID::nil(),
            profiling_overlay_root: NodeID::nil(),
            active_demo_root: NodeID::nil(),
            pause_sens_label: NodeID::nil(),
            hub_buttons: vec![NodeID::nil(); 11],
            pause_buttons: vec![NodeID::nil(); 5],
        }
    }
}

#[derive(Variant, Clone, Copy)]
struct PausedAnimPlayerState {
    pub node: NodeID,
    pub paused: bool,
}

#[derive(Variant, Clone, Copy)]
struct PausedAnimTreeState {
    pub node: NodeID,
    pub paused: bool,
}

#[derive(Variant, Clone, Copy)]
struct PausedScriptState {
    pub node: NodeID,
    pub update: bool,
    pub fixed_update: bool,
}

#[derive(Variant, Clone, Copy)]
struct DemoRuntimeState {
    pub mode: DemoMode,
    pub active_demo: DemoKind,
    pub queued_demo: DemoKind,
    pub fade_action: FadeAction,
    pub fade_phase: FadePhase,
    pub fade_alpha: f32,
    pub fade_hold: f32,
    pub fade_active: bool,
    pub fade_action_done: bool,
    pub pause_alpha: f32,
    pub mouse_sensitivity: f32,
    pub freecam_speed: f32,
    pub pause_applied: bool,
    pub physics_was_paused: bool,
}

impl Default for DemoRuntimeState {
    fn default() -> Self {
        Self {
            mode: DemoMode::Hub,
            active_demo: DemoKind::None,
            queued_demo: DemoKind::None,
            fade_action: FadeAction::None,
            fade_phase: FadePhase::Idle,
            fade_alpha: 0.0,
            fade_hold: 0.0,
            fade_active: false,
            fade_action_done: true,
            pause_alpha: 0.0,
            mouse_sensitivity: DEFAULT_MOUSE_SENSITIVITY,
            freecam_speed: DEFAULT_FREECAM_SPEED,
            pause_applied: false,
            physics_was_paused: false,
        }
    }
}

#[State]
struct DemoManagerState {
    #[default = DemoScenesState::default()]
    pub scenes: DemoScenesState,
    #[default = DemoRefsState::default()]
    pub refs: DemoRefsState,
    #[default = DemoRuntimeState::default()]
    pub runtime: DemoRuntimeState,
    pub paused_anim_players: Vec<PausedAnimPlayerState>,
    pub paused_anim_trees: Vec<PausedAnimTreeState>,
    pub paused_scripts: Vec<PausedScriptState>,
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let main_menu = scene_preload!(ctx.run, MAIN_MENU_SCENE_PATH).expect("preload main menu");
        let pause_menu =
            scene_preload!(ctx.run, PAUSE_MENU_SCENE_PATH).expect("preload pause menu");
        let fade = scene_preload!(ctx.run, TRANSITION_FADE_SCENE_PATH).expect("preload fade");
        let profiling_overlay = scene_preload!(ctx.run, PROFILING_OVERLAY_SCENE_PATH)
            .expect("preload profiling overlay");
        let mesh = scene_preload!(ctx.run, MESH_DEMO_SCENE_PATH).expect("preload mesh demo");
        let lights = scene_preload!(ctx.run, LIGHTS_DEMO_SCENE_PATH).expect("preload lights demo");
        let water = scene_preload!(ctx.run, WATER_DEMO_SCENE_PATH).expect("preload water demo");
        let animations =
            scene_preload!(ctx.run, ANIMATION_DEMO_SCENE_PATH).expect("preload animation demo");
        let physics_bones = scene_preload!(ctx.run, PHYSICS_BONES_DEMO_SCENE_PATH)
            .expect("preload physics bones demo");
        let physics_collisions = scene_preload!(ctx.run, PHYSICS_COLLISIONS_DEMO_SCENE_PATH)
            .expect("preload physics collisions demo");
        let sky = scene_preload!(ctx.run, SKY_DEMO_SCENE_PATH).expect("preload sky demo");
        let blend =
            scene_preload!(ctx.run, BLEND_DEMO_SCENE_PATH).expect("preload mesh blend demo");
        let multimesh =
            scene_preload!(ctx.run, MULTIMESH_DEMO_SCENE_PATH).expect("preload multimesh demo");
        let particles =
            scene_preload!(ctx.run, PARTICLES_DEMO_SCENE_PATH).expect("preload particles demo");
        let positional_audio = scene_preload!(ctx.run, POSITIONAL_AUDIO_DEMO_SCENE_PATH)
            .expect("preload positional audio demo");

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.scenes = DemoScenesState {
                main_menu,
                pause_menu,
                fade,
                profiling_overlay,
                mesh,
                lights,
                water,
                animations,
                physics_bones,
                physics_collisions,
                sky,
                blend,
                multimesh,
                particles,
                positional_audio,
            };
            state.runtime = DemoRuntimeState::default();
        });

        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_mesh_click"),
            func!("on_demo_mesh_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_lights_click"),
            func!("on_demo_lights_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_water_click"),
            func!("on_demo_water_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_animations_click"),
            func!("on_demo_animations_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_physics_bones_click"),
            func!("on_demo_physics_bones_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_physics_collisions_click"),
            func!("on_demo_physics_collisions_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_sky_click"),
            func!("on_demo_sky_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_blend_click"),
            func!("on_demo_blend_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_multimesh_click"),
            func!("on_demo_multimesh_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_particles_click"),
            func!("on_demo_particles_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("demo_audio_click"),
            func!("on_demo_audio_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("pause_sens_down_click"),
            func!("on_pause_sens_down_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("pause_sens_up_click"),
            func!("on_pause_sens_up_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("pause_resume_click"),
            func!("on_pause_resume_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("pause_restart_click"),
            func!("on_pause_restart_click")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("pause_hub_click"),
            func!("on_pause_hub_click")
        );

        self.load_main_menu_scene(ctx);
        self.load_pause_menu_scene(ctx);
        self.load_fade_scene(ctx);
        self.load_profiling_overlay_scene(ctx);
        self.apply_mode_io(ctx);
        self.sync_ui(ctx);
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        self.update_fade(ctx, dt);
        self.update_pause_fade(ctx, dt);

        let (mode, fade_active) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.runtime.mode, state.runtime.fade_active)
        });

        if fade_active {
            self.apply_mode_io(ctx);
            return;
        }

        if mode == DemoMode::DemoActive {
            self.adjust_freecam_speed_from_scroll(ctx);
        }

        if key_pressed!(ctx.ipt, KeyCode::Escape) {
            match mode {
                DemoMode::DemoActive => {
                    self.open_pause(ctx);
                }
                DemoMode::Paused => {
                    self.resume_demo(ctx);
                }
                DemoMode::Hub => {}
            }
        }

        self.apply_mode_io(ctx);
    }
});

methods!({
    fn on_demo_mesh_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::MeshMaterials);
    }

    fn on_demo_lights_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Lights);
    }

    fn on_demo_water_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Water);
    }

    fn on_demo_animations_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Animations);
    }

    fn on_demo_physics_bones_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::PhysicsBones);
    }

    fn on_demo_physics_collisions_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::PhysicsCollisions);
    }

    fn on_demo_sky_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Sky);
    }

    fn on_demo_blend_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::MeshBlending);
    }

    fn on_demo_multimesh_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::MultiMesh);
    }

    fn on_demo_particles_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Particles);
    }

    fn on_demo_audio_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::PositionalAudio);
    }

    fn on_pause_resume_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.resume_demo(ctx);
    }

    fn on_pause_sens_down_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.adjust_mouse_sensitivity(ctx, -MOUSE_SENSITIVITY_STEP);
    }

    fn on_pause_sens_up_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.adjust_mouse_sensitivity(ctx, MOUSE_SENSITIVITY_STEP);
    }

    fn on_pause_restart_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_restart_demo(ctx);
    }

    fn on_pause_hub_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_back_to_hub(ctx);
    }

    fn load_main_menu_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            scene_ui_parent(ctx, ctx.id),
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| state
                .scenes
                .main_menu),
        );
        if parent.is_nil() || scene.is_nil() {
            return;
        }

        let root = match scene_load!(ctx.run, scene) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] main menu load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, parent, root);

        let panel = get_child!(ctx.run, root, HUB_MENU_PANEL_NODE_NAME).unwrap_or(root);
        let content =
            get_child!(ctx.run, panel, HUB_MENU_CONTENT_NODE_NAME).unwrap_or(NodeID::nil());
        let buttons = vec![
            find_descendant_by_name(ctx, content, DEMO_BUTTON_MESH_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_LIGHTS_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_WATER_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_ANIMATIONS_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_PHYSICS_BONES_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_PHYSICS_COLLISIONS_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_SKY_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_BLEND_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_MULTIMESH_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_PARTICLES_NODE_NAME),
            find_descendant_by_name(ctx, content, DEMO_BUTTON_AUDIO_NODE_NAME),
        ];

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.main_menu_root = root;
            state.refs.hub_buttons = buttons;
        });
    }

    fn load_pause_menu_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            scene_ui_parent(ctx, ctx.id),
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| state
                .scenes
                .pause_menu),
        );
        if parent.is_nil() || scene.is_nil() {
            return;
        }

        let root = match scene_load!(ctx.run, scene) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] pause menu load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, parent, root);

        let panel = get_child!(ctx.run, root, PAUSE_PANEL_NODE_NAME).unwrap_or(root);
        let content = get_child!(ctx.run, panel, PAUSE_CONTENT_NODE_NAME).unwrap_or(NodeID::nil());
        let sens_row =
            get_child!(ctx.run, content, PAUSE_SENS_ROW_NODE_NAME).unwrap_or(NodeID::nil());
        let sens_label =
            get_child!(ctx.run, sens_row, PAUSE_SENS_LABEL_NODE_NAME).unwrap_or(NodeID::nil());
        let buttons = vec![
            get_child!(ctx.run, sens_row, PAUSE_BUTTON_SENS_DOWN_NODE_NAME)
                .unwrap_or(NodeID::nil()),
            get_child!(ctx.run, sens_row, PAUSE_BUTTON_SENS_UP_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, PAUSE_BUTTON_RESUME_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, PAUSE_BUTTON_RESTART_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, PAUSE_BUTTON_HUB_NODE_NAME).unwrap_or(NodeID::nil()),
        ];

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.pause_menu_root = root;
            state.refs.pause_sens_label = sens_label;
            state.refs.pause_buttons = buttons;
        });
        self.sync_mouse_sensitivity_label(ctx);
        self.apply_pause_alpha(ctx, 0.0);
    }

    fn load_fade_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            scene_ui_parent(ctx, ctx.id),
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| state.scenes.fade),
        );
        if parent.is_nil() || scene.is_nil() {
            return;
        }

        let root = match scene_load!(ctx.run, scene) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] fade load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, parent, root);
        let panel = get_child!(ctx.run, root, TRANSITION_FADE_PANEL_NODE_NAME).unwrap_or(root);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.fade_root = root;
            state.refs.fade_panel = panel;
        });
        self.apply_transition_fade(ctx, 0.0, false);
    }

    fn load_profiling_overlay_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            scene_ui_parent(ctx, ctx.id),
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| state
                .scenes
                .profiling_overlay),
        );
        if parent.is_nil() || scene.is_nil() {
            return;
        }

        let root = match scene_load!(ctx.run, scene) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] profiling overlay load fail: {:?}", err);
                return;
            }
        };
        reparent!(ctx.run, parent, root);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.profiling_overlay_root = root;
        });
        set_ui_tree_visible(ctx, root, true);
    }

    fn queue_load_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        if demo == DemoKind::None {
            return;
        }
        let fade_active = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.fade_active
        });
        if fade_active {
            return;
        }
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.queued_demo = demo;
            state.runtime.fade_action = FadeAction::LoadDemo;
            state.runtime.fade_action_done = false;
        });
        self.start_fade(ctx);
    }

    fn queue_restart_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let active_demo = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.active_demo
        });
        if active_demo == DemoKind::None {
            self.resume_demo(ctx);
            return;
        }
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.queued_demo = active_demo;
            state.runtime.fade_action = FadeAction::RestartDemo;
            state.runtime.fade_action_done = false;
        });
        self.start_fade(ctx);
    }

    fn queue_back_to_hub(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.queued_demo = DemoKind::None;
            state.runtime.fade_action = FadeAction::BackToHub;
            state.runtime.fade_action_done = false;
        });
        self.start_fade(ctx);
    }

    fn start_fade(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.fade_alpha = 0.0;
            state.runtime.fade_hold = FADE_HOLD_SECONDS;
            state.runtime.fade_phase = FadePhase::FadeIn;
            state.runtime.fade_active = true;
        });
        mouse_show!(ctx.ipt);
        self.sync_ui(ctx);
        self.apply_mode_io(ctx);
        self.apply_transition_fade(ctx, 0.0, true);
    }

    fn update_fade(&self, ctx: &mut ScriptContext<'_, API>, dt: f32) {
        let mut run_action = false;
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            if !state.runtime.fade_active {
                return;
            }

            match state.runtime.fade_phase {
                FadePhase::Idle => {}
                FadePhase::FadeIn => {
                    let step = if FADE_IN_SECONDS <= 0.0001 {
                        1.0
                    } else {
                        dt / FADE_IN_SECONDS
                    };
                    state.runtime.fade_alpha = (state.runtime.fade_alpha + step).min(1.0);
                    if state.runtime.fade_alpha >= 1.0 {
                        if !state.runtime.fade_action_done {
                            state.runtime.fade_action_done = true;
                            run_action = true;
                        }
                        state.runtime.fade_phase = FadePhase::Hold;
                    }
                }
                FadePhase::Hold => {
                    state.runtime.fade_hold = (state.runtime.fade_hold - dt).max(0.0);
                    if state.runtime.fade_hold <= 0.0 {
                        state.runtime.fade_phase = FadePhase::FadeOut;
                    }
                }
                FadePhase::FadeOut => {
                    let step = if FADE_OUT_SECONDS <= 0.0001 {
                        1.0
                    } else {
                        dt / FADE_OUT_SECONDS
                    };
                    state.runtime.fade_alpha = (state.runtime.fade_alpha - step).max(0.0);
                    if state.runtime.fade_alpha <= 0.0 {
                        state.runtime.fade_phase = FadePhase::Idle;
                        state.runtime.fade_active = false;
                        state.runtime.fade_action = FadeAction::None;
                    }
                }
            }
        });

        if run_action {
            self.execute_fade_action(ctx);
        }

        let (alpha, active) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.runtime.fade_alpha, state.runtime.fade_active)
        });
        self.apply_transition_fade(ctx, alpha, active);
    }

    fn execute_fade_action(&self, ctx: &mut ScriptContext<'_, API>) {
        let (action, demo) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.runtime.fade_action, state.runtime.queued_demo)
        });

        match action {
            FadeAction::None => {}
            FadeAction::LoadDemo | FadeAction::RestartDemo => {
                self.unload_active_demo(ctx);
                self.load_demo(ctx, demo);
            }
            FadeAction::BackToHub => {
                self.unload_active_demo(ctx);
                with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
                    state.runtime.mode = DemoMode::Hub;
                    state.runtime.active_demo = DemoKind::None;
                    state.runtime.queued_demo = DemoKind::None;
                });
                mouse_show!(ctx.ipt);
            }
        }
        self.sync_ui(ctx);
    }

    fn load_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        let Some(path) = demo.path() else {
            return;
        };

        if ctx.id.is_nil() {
            log_error!("[DemoManager] manager root missing");
            return;
        }

        let root = match scene_load!(ctx.run, path) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] demo load fail: {:?}", err);
                return;
            }
        };

        let scene_root = get_node_parent_id!(ctx.run, root).unwrap_or(root);
        let root_to_attach = if scene_root.is_nil() {
            root
        } else {
            scene_root
        };

        reparent!(ctx.run, ctx.id, root_to_attach);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.active_demo_root = root_to_attach;
            state.runtime.active_demo = demo;
            state.runtime.mode = DemoMode::DemoActive;
            state.runtime.pause_alpha = 0.0;
        });
        self.apply_mouse_sensitivity_to_active_demo(ctx);
        self.apply_freecam_speed_to_active_demo(ctx);
        mouse_capture!(ctx.ipt);
    }

    fn unload_active_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        self.apply_demo_pause(ctx, false);
        let root = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.active_demo_root
        });
        if !root.is_nil() {
            remove_node!(ctx.run, root);
        }
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.active_demo_root = NodeID::nil();
        });
    }

    fn open_pause(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            if state.runtime.mode == DemoMode::DemoActive {
                state.runtime.mode = DemoMode::Paused;
            }
        });
        self.apply_demo_pause(ctx, true);
        mouse_show!(ctx.ipt);
        self.sync_ui(ctx);
    }

    fn resume_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let has_demo = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.active_demo != DemoKind::None
        });
        self.apply_demo_pause(ctx, false);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.mode = if has_demo {
                DemoMode::DemoActive
            } else {
                DemoMode::Hub
            };
        });
        if has_demo {
            mouse_capture!(ctx.ipt);
        } else {
            mouse_show!(ctx.ipt);
        }
        self.sync_ui(ctx);
    }

    fn apply_demo_pause(&self, ctx: &mut ScriptContext<'_, API>, paused: bool) {
        let (root, already_applied) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.refs.active_demo_root, state.runtime.pause_applied)
        });

        if paused {
            if already_applied || root.is_nil() {
                return;
            }

            let physics_was_paused = physics_is_paused!(ctx.run);
            physics_pause!(ctx.run, true);

            let mut scripts = Vec::new();
            for node in subtree_nodes(ctx, root) {
                let update = script_set_update_enabled!(ctx.run, node, false);
                let fixed_update = script_set_fixed_update_enabled!(ctx.run, node, false);
                if update || fixed_update {
                    scripts.push(PausedScriptState {
                        node,
                        update,
                        fixed_update,
                    });
                }
            }

            let mut anim_players = Vec::new();
            for node in query!(ctx.run, all(node_type[AnimationPlayer]), in_subtree(root)) {
                if let Some(prev) = with_node_mut!(ctx.run, AnimationPlayer, node, |player| {
                    let prev = player.paused;
                    player.paused = true;
                    prev
                }) {
                    anim_players.push(PausedAnimPlayerState { node, paused: prev });
                }
            }

            let mut anim_trees = Vec::new();
            for node in query!(ctx.run, all(node_type[AnimationTree]), in_subtree(root)) {
                if let Some(prev) = with_node_mut!(ctx.run, AnimationTree, node, |tree| {
                    let prev = tree.paused;
                    tree.paused = true;
                    prev
                }) {
                    anim_trees.push(PausedAnimTreeState { node, paused: prev });
                }
            }

            with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
                state.runtime.pause_applied = true;
                state.runtime.physics_was_paused = physics_was_paused;
                state.paused_anim_players = anim_players;
                state.paused_anim_trees = anim_trees;
                state.paused_scripts = scripts;
            });
            return;
        }

        if !already_applied {
            return;
        }

        let (physics_was_paused, anim_players, anim_trees, scripts) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.physics_was_paused,
                    state.paused_anim_players.clone(),
                    state.paused_anim_trees.clone(),
                    state.paused_scripts.clone(),
                )
            });

        for saved in scripts {
            if saved.update {
                let _ = script_set_update_enabled!(ctx.run, saved.node, true);
            }
            if saved.fixed_update {
                let _ = script_set_fixed_update_enabled!(ctx.run, saved.node, true);
            }
        }
        for saved in anim_players {
            let _ = with_node_mut!(ctx.run, AnimationPlayer, saved.node, |player| {
                player.paused = saved.paused;
            });
        }
        for saved in anim_trees {
            let _ = with_node_mut!(ctx.run, AnimationTree, saved.node, |tree| {
                tree.paused = saved.paused;
            });
        }
        physics_pause!(ctx.run, physics_was_paused);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.pause_applied = false;
            state.runtime.physics_was_paused = false;
            state.paused_anim_players.clear();
            state.paused_anim_trees.clear();
            state.paused_scripts.clear();
        });
    }

    fn adjust_mouse_sensitivity(&self, ctx: &mut ScriptContext<'_, API>, delta: f32) {
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.mouse_sensitivity = (state.runtime.mouse_sensitivity + delta)
                .clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY);
        });
        self.sync_mouse_sensitivity_label(ctx);
        self.apply_mouse_sensitivity_to_active_demo(ctx);
    }

    fn adjust_freecam_speed_from_scroll(&self, ctx: &mut ScriptContext<'_, API>) {
        let wheel = mouse_wheel!(ctx.ipt).y;
        if wheel.abs() <= 0.001 {
            return;
        }

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.freecam_speed = (state.runtime.freecam_speed + wheel * FREECAM_SPEED_STEP)
                .clamp(MIN_FREECAM_SPEED, MAX_FREECAM_SPEED);
        });
        self.apply_freecam_speed_to_active_demo(ctx);
    }

    fn update_pause_fade(&self, ctx: &mut ScriptContext<'_, API>, dt: f32) {
        let alpha = with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            let target = if state.runtime.mode == DemoMode::Paused {
                PAUSE_BG_MAX_ALPHA
            } else {
                0.0
            };
            let fade_seconds = if target > state.runtime.pause_alpha {
                PAUSE_FADE_IN_SECONDS
            } else {
                PAUSE_FADE_OUT_SECONDS
            };
            let step = if fade_seconds <= 0.0001 {
                PAUSE_BG_MAX_ALPHA
            } else {
                (PAUSE_BG_MAX_ALPHA * dt) / fade_seconds
            };

            if state.runtime.pause_alpha < target {
                state.runtime.pause_alpha = (state.runtime.pause_alpha + step).min(target);
            } else if state.runtime.pause_alpha > target {
                state.runtime.pause_alpha = (state.runtime.pause_alpha - step).max(target);
            }
            state.runtime.pause_alpha
        })
        .unwrap_or(0.0);
        self.apply_pause_alpha(ctx, alpha);
    }

    fn sync_ui(&self, ctx: &mut ScriptContext<'_, API>) {
        let (mode, fade_action, menu_root, pause_root, profiling_overlay_root, pause_alpha) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.mode,
                    state.runtime.fade_action,
                    state.refs.main_menu_root,
                    state.refs.pause_menu_root,
                    state.refs.profiling_overlay_root,
                    state.runtime.pause_alpha,
                )
            });
        let show_hub_menu = mode == DemoMode::Hub
            && !matches!(fade_action, FadeAction::LoadDemo | FadeAction::RestartDemo);
        set_ui_tree_visible(ctx, menu_root, show_hub_menu);
        set_ui_tree_visible(ctx, profiling_overlay_root, true);
        set_ui_tree_visible(
            ctx,
            pause_root,
            mode == DemoMode::Paused || pause_alpha > 0.001,
        );
        self.apply_pause_alpha(ctx, pause_alpha);
        self.apply_mode_io(ctx);
    }

    fn apply_mode_io(&self, ctx: &mut ScriptContext<'_, API>) {
        let (mode, fade_active, fade_action, hub_buttons, pause_buttons, pause_alpha) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.mode,
                    state.runtime.fade_active,
                    state.runtime.fade_action,
                    state.refs.hub_buttons.clone(),
                    state.refs.pause_buttons.clone(),
                    state.runtime.pause_alpha,
                )
            });

        let hub_visible = mode == DemoMode::Hub
            && !matches!(fade_action, FadeAction::LoadDemo | FadeAction::RestartDemo);
        let pause_visible = mode == DemoMode::Paused || pause_alpha > 0.001;
        let hub_enabled = hub_visible && !fade_active;
        let pause_enabled = mode == DemoMode::Paused
            && !fade_active
            && (pause_alpha / PAUSE_BG_MAX_ALPHA).clamp(0.0, 1.0) >= 0.85;

        for id in hub_buttons {
            if !id.is_nil() {
                with_node_mut!(ctx.run, UiButton, id, |button| {
                    button.input_enabled = hub_enabled;
                });
            }
        }

        for id in pause_buttons {
            if !id.is_nil() {
                with_node_mut!(ctx.run, UiButton, id, |button| {
                    button.input_enabled = pause_enabled;
                });
            }
        }

        if hub_visible || pause_visible {
            self.apply_freecam_input_enabled_to_active_demo(ctx, false);
            mouse_show!(ctx.ipt);
        } else {
            self.apply_freecam_input_enabled_to_active_demo(ctx, true);
            mouse_capture!(ctx.ipt);
        }
    }

    fn sync_mouse_sensitivity_label(&self, ctx: &mut ScriptContext<'_, API>) {
        let (label, sensitivity) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.refs.pause_sens_label, state.runtime.mouse_sensitivity)
        });
        if label.is_nil() {
            return;
        }
        let scale = sensitivity / DEFAULT_MOUSE_SENSITIVITY;
        with_node_mut!(ctx.run, UiLabel, label, |node| {
            node.text = format!("Mouse Sens {:.2}x", scale).into();
        });
    }

    fn apply_mouse_sensitivity_to_active_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let (root, sensitivity) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (
                state.refs.active_demo_root,
                state
                    .runtime
                    .mouse_sensitivity
                    .clamp(MIN_MOUSE_SENSITIVITY, MAX_MOUSE_SENSITIVITY),
            )
        });
        if root.is_nil() {
            return;
        }

        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            set_var!(
                ctx.run,
                id,
                var!("mouse_sensitivity"),
                variant!(sensitivity)
            );
            if let Some(children) = get_node_children_ids!(ctx.run, id) {
                for child in children {
                    if !child.is_nil() {
                        stack.push(child);
                    }
                }
            }
        }
    }

    fn apply_freecam_input_enabled_to_active_demo(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        enabled: bool,
    ) {
        let root = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.active_demo_root
        });
        if root.is_nil() {
            return;
        }

        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            set_var!(ctx.run, id, var!("input_enabled"), variant!(enabled));
            if let Some(children) = get_node_children_ids!(ctx.run, id) {
                for child in children {
                    if !child.is_nil() {
                        stack.push(child);
                    }
                }
            }
        }
    }

    fn apply_freecam_speed_to_active_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let (root, speed) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (
                state.refs.active_demo_root,
                state
                    .runtime
                    .freecam_speed
                    .clamp(MIN_FREECAM_SPEED, MAX_FREECAM_SPEED),
            )
        });
        if root.is_nil() {
            return;
        }

        let camera = find_descendant_by_name(ctx, root, DEMO_CAMERA_NODE_NAME);
        if camera.is_nil() {
            return;
        }

        set_var!(ctx.run, camera, var!("speed"), variant!(speed));
    }

    fn apply_transition_fade(&self, ctx: &mut ScriptContext<'_, API>, alpha: f32, visible: bool) {
        let (root, panel) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (state.refs.fade_root, state.refs.fade_panel)
        });
        let clamped = if visible { alpha.clamp(0.0, 1.0) } else { 0.0 };
        let show = visible && clamped > 0.001;
        let color = color_with_alpha("#000000", clamped);
        for id in [root, panel] {
            if id.is_nil() {
                continue;
            }
            with_node_mut!(ctx.run, UiPanel, id, |node| {
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

    fn apply_pause_alpha(&self, ctx: &mut ScriptContext<'_, API>, alpha: f32) {
        let (mode, root, buttons, sens_label) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.mode,
                    state.refs.pause_menu_root,
                    state.refs.pause_buttons.clone(),
                    state.refs.pause_sens_label,
                )
            });
        if root.is_nil() {
            return;
        }

        let show = mode == DemoMode::Paused || alpha > 0.001;
        let t = (alpha / PAUSE_BG_MAX_ALPHA).clamp(0.0, 1.0);
        let bg = color_with_alpha("#070A0F", alpha);

        with_node_mut!(ctx.run, UiPanel, root, |panel| {
            panel.visible = show;
            if let Some(color) = bg {
                panel.style.fill = color;
                panel.style.stroke = color;
                panel.style.stroke_width = 0.0;
            }
        });

        let panel = get_child!(ctx.run, root, PAUSE_PANEL_NODE_NAME).unwrap_or(NodeID::nil());
        if !panel.is_nil() {
            with_node_mut!(ctx.run, UiPanel, panel, |node| {
                node.visible = show;
                if let Some(color) = color_with_alpha("#0B1018", PAUSE_PANEL_ALPHA * t) {
                    node.style.fill = color;
                }
                if let Some(color) = color_with_alpha("#D0E0EF", t) {
                    node.style.stroke = color;
                }
                node.style.stroke_width = if show { 1.0 } else { 0.0 };
            });
        }

        let content = if panel.is_nil() {
            NodeID::nil()
        } else {
            get_child!(ctx.run, panel, PAUSE_CONTENT_NODE_NAME).unwrap_or(NodeID::nil())
        };
        if !content.is_nil() {
            set_ui_tree_visible(ctx, content, show);
            let title =
                get_child!(ctx.run, content, PAUSE_TITLE_NODE_NAME).unwrap_or(NodeID::nil());
            if !title.is_nil() {
                with_node_mut!(ctx.run, UiLabel, title, |label| {
                    label.visible = show;
                    if let Some(color) = color_with_alpha("#FFFFFF", t) {
                        label.color = color;
                    }
                });
            }
            if !sens_label.is_nil() {
                with_node_mut!(ctx.run, UiLabel, sens_label, |label| {
                    label.visible = show;
                    if let Some(color) = color_with_alpha("#FFFFFF", t) {
                        label.color = color;
                    }
                });
            }
        }

        for button in buttons {
            if button.is_nil() {
                continue;
            }
            set_button_alpha(ctx, button, t, show);
        }
    }
});

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
        if let Some(children) = get_node_children_ids!(ctx.run, id) {
            for child in children {
                if !child.is_nil() {
                    stack.push(child);
                }
            }
        }
    }
}

fn scene_ui_parent<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    manager: NodeID,
) -> NodeID {
    let scene_root = get_node_parent_id!(ctx.run, manager).unwrap_or(manager);
    get_child!(ctx.run, scene_root, DEMO_UI_ROOT_NODE_NAME).unwrap_or(scene_root)
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
        if let Some(child) = get_child!(ctx.run, id, name) {
            return child;
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

fn subtree_nodes<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    root: NodeID,
) -> Vec<NodeID> {
    if root.is_nil() {
        return Vec::new();
    }

    let mut nodes = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        nodes.push(id);
        if let Some(children) = get_node_children_ids!(ctx.run, id) {
            for child in children {
                if !child.is_nil() {
                    stack.push(child);
                }
            }
        }
    }
    nodes
}

fn set_button_alpha<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    button: NodeID,
    alpha: f32,
    visible: bool,
) {
    with_node_mut!(ctx.run, UiButton, button, |node| {
        node.visible = visible;
        if let Some(color) = color_with_alpha("#101820", 0.94 * alpha) {
            node.style.fill = color;
        }
        if let Some(color) = color_with_alpha("#88AADD", alpha) {
            node.style.stroke = color;
        }
    });

    if let Some(children) = get_node_children_ids!(ctx.run, button) {
        for child in children {
            with_node_mut!(ctx.run, UiLabel, child, |label| {
                label.visible = visible;
                if let Some(color) = color_with_alpha("#FFFFFF", alpha) {
                    label.color = color;
                }
            });
        }
    }
}

fn color_with_alpha(base: &str, alpha: f32) -> Option<Color> {
    let byte = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_hex(&format!("{base}{byte:02X}"))
}

fn demo_mode_name(mode: DemoMode) -> &'static str {
    match mode {
        DemoMode::Hub => "hub",
        DemoMode::DemoActive => "demo_active",
        DemoMode::Paused => "paused",
    }
}

fn fade_action_name(action: FadeAction) -> &'static str {
    match action {
        FadeAction::None => "none",
        FadeAction::LoadDemo => "load_demo",
        FadeAction::RestartDemo => "restart_demo",
        FadeAction::BackToHub => "back_to_hub",
    }
}
