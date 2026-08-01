use perro_api::prelude::*;

#[State]
struct DemoFadeRefsState {
    #[default = NodeID::nil()]
    pub transition_fade_panel: NodeID,
}

lifecycle!({});
