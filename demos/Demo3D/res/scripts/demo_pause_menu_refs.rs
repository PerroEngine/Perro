use perro_api::prelude::*;

type SelfNodeType = UiPanel;

#[State]
struct DemoPauseMenuRefsState {
    #[default = NodeID::nil()]
    pub pause_panel: NodeID,
    #[default = NodeID::nil()]
    pub pause_content: NodeID,
    #[default = NodeID::nil()]
    pub pause_title: NodeID,
    #[default = NodeID::nil()]
    pub pause_sens_label: NodeID,
    #[default = NodeID::nil()]
    pub pause_btn_sens_down: NodeID,
    #[default = NodeID::nil()]
    pub pause_btn_sens_up: NodeID,
    #[default = NodeID::nil()]
    pub pause_btn_resume: NodeID,
    #[default = NodeID::nil()]
    pub pause_btn_restart: NodeID,
    #[default = NodeID::nil()]
    pub pause_btn_hub: NodeID,
}

lifecycle!({});
