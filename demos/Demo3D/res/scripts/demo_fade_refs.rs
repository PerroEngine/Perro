use perro_api::prelude::*;

type SelfNodeType = UiPanel;

#[State]
struct DemoFadeRefsState {
    #[default = NodeID::nil()]
    pub transition_fade_panel: NodeID,
}

lifecycle!({});
