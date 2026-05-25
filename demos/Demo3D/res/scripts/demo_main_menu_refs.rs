use perro_api::prelude::*;

type SelfNodeType = UiPanel;

#[State]
struct DemoMainMenuRefsState {
    #[default = NodeID::nil()]
    pub hub_menu_panel: NodeID,
    #[default = NodeID::nil()]
    pub hub_menu_content: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_mesh: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_lights: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_water: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_animations: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_physics_bones: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_physics_collisions: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_sky: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_sky_wispy: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_blend: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_multimesh: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_particles: NodeID,
    #[default = NodeID::nil()]
    pub demo_btn_audio: NodeID,
}

lifecycle!({});
