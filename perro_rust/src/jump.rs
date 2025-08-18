#![allow(improper_ctypes_definitions)]
use uuid::Uuid;
use perro_core::{scripting::api::ScriptApi, scripting::script::Script, Sprite2D};

#[unsafe(no_mangle)]
pub extern "C" fn jump_create_script() -> *mut dyn Script {
    Box::into_raw(Box::new(JumpScript {
        node_id: Uuid::nil(), // will be set later
    })) as *mut dyn Script
}

pub struct JumpScript {
    node_id: Uuid,
}

impl Script for JumpScript {
    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let d = api.get_delta();

        let sprite = api.get_node_mut::<Sprite2D>(&self.node_id).unwrap(); 
        sprite.transform.position.x += 1.0 * d;
        
        self.bob(api);
    }

    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }
}

impl JumpScript {
    fn bob(&mut self, api: &mut ScriptApi<'_>) {
         if let Some(sprite) = api.get_node_mut::<Sprite2D>(&self.node_id) {
            sprite.transform.position.y += 0.0005;
        }
    }
}