use perro_api::prelude::*;

type SelfNodeType = Node3D;

const MAIN_MENU_SCENE_PATH: &ResPath = res_path!("res://Menu/MainMenu.scn");
const PAUSE_MENU_SCENE_PATH: &ResPath = res_path!("res://Menu/PauseMenu.scn");
const TRANSITION_FADE_SCENE_PATH: &ResPath = res_path!("res://Menu/TransitionFade.scn");
const MESH_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/mesh_materials.scn");
const LIGHTS_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/lights.scn");
const WATER_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/water.scn");
const WATER_CANNON_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/water_cannon.scn");
const ANIMATION_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/animations.scn");
const SKY_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/sky.scn");
const BLEND_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/mesh_blending.scn");
const MULTIMESH_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/multimesh.scn");
const PARTICLES_DEMO_SCENE_PATH: &ResPath = res_path!("res://scenes/demos/particles.scn");
const POSITIONAL_AUDIO_DEMO_SCENE_PATH: &ResPath =
    res_path!("res://scenes/demos/positional_audio.scn");

const DEMO_MOUNT_NODE_NAME: &str = "DemoMount";
const HUB_CAMERA_NODE_NAME: &str = "HubCamera";
const HUB_MENU_PANEL_NODE_NAME: &str = "hub_menu_panel";
const HUB_MENU_CONTENT_NODE_NAME: &str = "hub_menu_content";
const DEMO_BUTTON_MESH_NODE_NAME: &str = "demo_btn_mesh";
const DEMO_BUTTON_LIGHTS_NODE_NAME: &str = "demo_btn_lights";
const DEMO_BUTTON_WATER_NODE_NAME: &str = "demo_btn_water";
const DEMO_BUTTON_WATER_CANNON_NODE_NAME: &str = "demo_btn_water_cannon";
const DEMO_BUTTON_ANIMATIONS_NODE_NAME: &str = "demo_btn_animations";
const DEMO_BUTTON_SKY_NODE_NAME: &str = "demo_btn_sky";
const DEMO_BUTTON_BLEND_NODE_NAME: &str = "demo_btn_blend";
const DEMO_BUTTON_MULTIMESH_NODE_NAME: &str = "demo_btn_multimesh";
const DEMO_BUTTON_PARTICLES_NODE_NAME: &str = "demo_btn_particles";
const DEMO_BUTTON_AUDIO_NODE_NAME: &str = "demo_btn_audio";
const PAUSE_PANEL_NODE_NAME: &str = "pause_panel";
const PAUSE_CONTENT_NODE_NAME: &str = "pause_content";
const PAUSE_BUTTON_RESUME_NODE_NAME: &str = "pause_btn_resume";
const PAUSE_BUTTON_RESTART_NODE_NAME: &str = "pause_btn_restart";
const PAUSE_BUTTON_HUB_NODE_NAME: &str = "pause_btn_hub";
const TRANSITION_FADE_PANEL_NODE_NAME: &str = "transition_fade_panel";

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
    WaterCannon,
    Animations,
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
            DemoKind::WaterCannon => Some(WATER_CANNON_DEMO_SCENE_PATH),
            DemoKind::Animations => Some(ANIMATION_DEMO_SCENE_PATH),
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
    pub mesh: PreloadedSceneID,
    pub lights: PreloadedSceneID,
    pub water: PreloadedSceneID,
    pub water_cannon: PreloadedSceneID,
    pub animations: PreloadedSceneID,
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
            mesh: PreloadedSceneID::nil(),
            lights: PreloadedSceneID::nil(),
            water: PreloadedSceneID::nil(),
            water_cannon: PreloadedSceneID::nil(),
            animations: PreloadedSceneID::nil(),
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
    pub demo_mount: NodeID,
    pub hub_camera: NodeID,
    pub active_demo_root: NodeID,
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
            demo_mount: NodeID::nil(),
            hub_camera: NodeID::nil(),
            active_demo_root: NodeID::nil(),
            hub_buttons: vec![NodeID::nil(); 10],
            pause_buttons: vec![NodeID::nil(); 3],
        }
    }
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
}

lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let main_menu = scene_preload!(ctx.run, MAIN_MENU_SCENE_PATH).expect("preload main menu");
        let pause_menu =
            scene_preload!(ctx.run, PAUSE_MENU_SCENE_PATH).expect("preload pause menu");
        let fade = scene_preload!(ctx.run, TRANSITION_FADE_SCENE_PATH).expect("preload fade");
        let mesh = scene_preload!(ctx.run, MESH_DEMO_SCENE_PATH).expect("preload mesh demo");
        let lights = scene_preload!(ctx.run, LIGHTS_DEMO_SCENE_PATH).expect("preload lights demo");
        let water = scene_preload!(ctx.run, WATER_DEMO_SCENE_PATH).expect("preload water demo");
        let water_cannon = scene_preload!(ctx.run, WATER_CANNON_DEMO_SCENE_PATH)
            .expect("preload water cannon demo");
        let animations =
            scene_preload!(ctx.run, ANIMATION_DEMO_SCENE_PATH).expect("preload animation demo");
        let sky = scene_preload!(ctx.run, SKY_DEMO_SCENE_PATH).expect("preload sky demo");
        let blend =
            scene_preload!(ctx.run, BLEND_DEMO_SCENE_PATH).expect("preload mesh blend demo");
        let multimesh =
            scene_preload!(ctx.run, MULTIMESH_DEMO_SCENE_PATH).expect("preload multimesh demo");
        let particles =
            scene_preload!(ctx.run, PARTICLES_DEMO_SCENE_PATH).expect("preload particles demo");
        let positional_audio = scene_preload!(ctx.run, POSITIONAL_AUDIO_DEMO_SCENE_PATH)
            .expect("preload positional audio demo");

        let parent = get_node_parent_id!(ctx.run, ctx.id).unwrap_or(NodeID::nil());
        let demo_mount = get_child!(ctx.run, parent, DEMO_MOUNT_NODE_NAME).unwrap_or(NodeID::nil());
        let hub_camera = get_child!(ctx.run, parent, HUB_CAMERA_NODE_NAME).unwrap_or(NodeID::nil());

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.scenes = DemoScenesState {
                main_menu,
                pause_menu,
                fade,
                mesh,
                lights,
                water,
                water_cannon,
                animations,
                sky,
                blend,
                multimesh,
                particles,
                positional_audio,
            };
            state.refs.demo_mount = demo_mount;
            state.refs.hub_camera = hub_camera;
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
            signal!("demo_water_cannon_click"),
            func!("on_demo_water_cannon_click")
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
        self.set_hub_camera_active(ctx, true);
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

    fn on_demo_water_cannon_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::WaterCannon);
    }

    fn on_demo_animations_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_load_demo(ctx, DemoKind::Animations);
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

    fn on_pause_restart_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_restart_demo(ctx);
    }

    fn on_pause_hub_click(&self, ctx: &mut ScriptContext<'_, API>, _button: NodeID) {
        self.queue_back_to_hub(ctx);
    }

    fn load_main_menu_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            get_node_parent_id!(ctx.run, ctx.id).unwrap_or(NodeID::nil()),
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
            get_child!(ctx.run, content, DEMO_BUTTON_MESH_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_LIGHTS_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_WATER_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_WATER_CANNON_NODE_NAME)
                .unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_ANIMATIONS_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_SKY_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_BLEND_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_MULTIMESH_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_PARTICLES_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, DEMO_BUTTON_AUDIO_NODE_NAME).unwrap_or(NodeID::nil()),
        ];

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.main_menu_root = root;
            state.refs.hub_buttons = buttons;
        });
    }

    fn load_pause_menu_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            get_node_parent_id!(ctx.run, ctx.id).unwrap_or(NodeID::nil()),
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
        let buttons = vec![
            get_child!(ctx.run, content, PAUSE_BUTTON_RESUME_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, PAUSE_BUTTON_RESTART_NODE_NAME).unwrap_or(NodeID::nil()),
            get_child!(ctx.run, content, PAUSE_BUTTON_HUB_NODE_NAME).unwrap_or(NodeID::nil()),
        ];

        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.pause_menu_root = root;
            state.refs.pause_buttons = buttons;
        });
        self.apply_pause_alpha(ctx, 0.0);
    }

    fn load_fade_scene(&self, ctx: &mut ScriptContext<'_, API>) {
        let (parent, scene) = (
            get_node_parent_id!(ctx.run, ctx.id).unwrap_or(NodeID::nil()),
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
                self.set_hub_camera_active(ctx, true);
                mouse_show!(ctx.ipt);
            }
        }
        self.sync_ui(ctx);
    }

    fn load_demo(&self, ctx: &mut ScriptContext<'_, API>, demo: DemoKind) {
        let Some(path) = demo.path() else {
            return;
        };

        let mount = with_state!(ctx.run, DemoManagerState, ctx.id, |state| state
            .refs
            .demo_mount);
        if mount.is_nil() {
            log_error!("[DemoManager] demo mount missing");
            return;
        }

        let root = match scene_load!(ctx.run, path) {
            Ok(id) => id,
            Err(err) => {
                log_error!("[DemoManager] demo load fail: {:?}", err);
                return;
            }
        };

        reparent!(ctx.run, mount, root);
        with_state_mut!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.refs.active_demo_root = root;
            state.runtime.active_demo = demo;
            state.runtime.mode = DemoMode::DemoActive;
            state.runtime.pause_alpha = 0.0;
        });
        self.set_hub_camera_active(ctx, false);
        mouse_capture!(ctx.ipt);
    }

    fn unload_active_demo(&self, ctx: &mut ScriptContext<'_, API>) {
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
        mouse_show!(ctx.ipt);
        self.sync_ui(ctx);
    }

    fn resume_demo(&self, ctx: &mut ScriptContext<'_, API>) {
        let has_demo = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            state.runtime.active_demo != DemoKind::None
        });
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
        let (mode, menu_root, pause_root, pause_alpha) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.mode,
                    state.refs.main_menu_root,
                    state.refs.pause_menu_root,
                    state.runtime.pause_alpha,
                )
            });
        set_ui_tree_visible(ctx, menu_root, mode == DemoMode::Hub);
        set_ui_tree_visible(
            ctx,
            pause_root,
            mode == DemoMode::Paused || pause_alpha > 0.001,
        );
        self.apply_pause_alpha(ctx, pause_alpha);
        self.apply_mode_io(ctx);
    }

    fn apply_mode_io(&self, ctx: &mut ScriptContext<'_, API>) {
        let (mode, fade_active, hub_buttons, pause_buttons, pause_alpha) =
            with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
                (
                    state.runtime.mode,
                    state.runtime.fade_active,
                    state.refs.hub_buttons.clone(),
                    state.refs.pause_buttons.clone(),
                    state.runtime.pause_alpha,
                )
            });

        let hub_enabled = mode == DemoMode::Hub && !fade_active;
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

        if mode == DemoMode::Hub || mode == DemoMode::Paused || fade_active {
            mouse_show!(ctx.ipt);
        } else {
            mouse_capture!(ctx.ipt);
        }
    }

    fn set_hub_camera_active(&self, ctx: &mut ScriptContext<'_, API>, active: bool) {
        let id = with_state!(ctx.run, DemoManagerState, ctx.id, |state| state
            .refs
            .hub_camera);
        if !id.is_nil() {
            with_node_mut!(ctx.run, Camera3D, id, |camera| {
                camera.active = active;
            });
        }
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
        let (mode, root, buttons) = with_state!(ctx.run, DemoManagerState, ctx.id, |state| {
            (
                state.runtime.mode,
                state.refs.pause_menu_root,
                state.refs.pause_buttons.clone(),
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
