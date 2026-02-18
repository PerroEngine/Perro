use perro_api::prelude::*;
use perro_core::prelude::*;
use perro_ids::prelude::*;
use perro_modules::prelude::*;
use perro_scripting::prelude::*;

///@State
#[derive(Default)]
pub struct ExampleState {
    speed: f32,
}

///@Script
pub struct ExampleScript;

impl<R: RuntimeAPI + ?Sized> ScriptLifecycle<R> for ExampleScript {
    fn init(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let _origin = Vector2::new(0.0, 0.0);
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed = 240.0;
            });
        LogMod::info("Script initialized!");

        let json = JSONMod::parse(r#"{"name":"perro","ok":true,"count":1223}"#).expect("valid json");
        let json_text = JSONMod::stringify(&json).expect("stringify json");
        let save_path = FileMod::resolve_path_string("user://parsed.json");
        LogMod::info(&format!("user path: {}", save_path));
        FileMod::save_string("user://parsed.json", json_text.as_str()).expect("write json");
        LogMod::info(&format!(
            "exists after save: {}",
            FileMod::exists("user://parsed.json")
        ));

        let loaded_text = FileMod::load_string("user://parsed.json").expect("read json");
        let loaded = JSONMod::parse(loaded_text.as_str()).expect("parse json");
        LogMod::info(JSONMod::stringify(&loaded).expect("stringify json").as_str());
    }

    fn update(&self, api: &mut API<'_, R>, self_id: NodeID) {
        let dt = api.Time().get_delta();
        let _ = api
            .Scripts()
            .with_state_mut::<ExampleState, _, _>(self_id, |state| {
                state.speed += dt;
            });
    }

    fn fixed_update(&self, _api: &mut API<'_, R>, _self_id: NodeID) {}
}
