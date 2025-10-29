#![allow(improper_ctypes_definitions)]
#![allow(unused)]

use std::any::Any;
use std::collections::HashMap;
use serde_json::{Value, json};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::ops::{Deref, DerefMut};
use rust_decimal::{Decimal, prelude::*};
use num_bigint::BigInt;
use std::str::FromStr;
use std::{rc::Rc, cell::RefCell};

use perro_core::prelude::*;

// ========================================================================
// MainPup - Main Script Structure
// ========================================================================

pub struct MainPupScript {
    node_id: Uuid,
}

// ========================================================================
// MainPup - Creator Function (FFI Entry Point)
// ========================================================================

#[unsafe(no_mangle)]
pub extern "C" fn main_pup_create_script() -> *mut dyn ScriptObject {
    Box::into_raw(Box::new(MainPupScript {
        node_id: Uuid::nil(),
    })) as *mut dyn ScriptObject
}

// ========================================================================
// Supporting Struct Definitions
// ========================================================================

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub health: i32,
    pub max_health: i32,
    pub speed: f32,
    pub score: f64,
    pub position_x: f32,
    pub position_y: f32,
}

impl std::fmt::Display for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "health: {:?}, ", self.health)?;
        write!(f, "max_health: {:?}, ", self.max_health)?;
        write!(f, "speed: {:?}, ", self.speed)?;
        write!(f, "score: {:?}, ", self.score)?;
        write!(f, "position_x: {:?}, ", self.position_x)?;
        write!(f, "position_y: {:?} ", self.position_y)?;
        write!(f, "}}")
    }
}

impl Player {
    pub fn new() -> Self { Self::default() }
}



#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Enemy {
    pub health: i32,
    pub damage: i32,
    pub x: f64,
    pub y: f32,
}

impl std::fmt::Display for Enemy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        write!(f, "health: {:?}, ", self.health)?;
        write!(f, "damage: {:?}, ", self.damage)?;
        write!(f, "x: {:?}, ", self.x)?;
        write!(f, "y: {:?} ", self.y)?;
        write!(f, "}}")
    }
}

impl Enemy {
    pub fn new() -> Self { Self::default() }
}



// ========================================================================
// MainPup - Script Init & Update Implementation
// ========================================================================

impl Script for MainPupScript {
    fn init(&mut self, api: &mut ScriptApi<'_>) {
        let mut player = Player::new();;
        player.health = 100i32;
        player.max_health = 100i32;
        player.speed = 5.0f32;
        player.score = 0.0f64;
        player.position_x = 0.0f32;
        player.position_y = 0.0f32;
        let mut enemy = Enemy::new();;
        enemy.health = 50i32;
        enemy.damage = 10i32;
        enemy.x = 100.0f64;
        enemy.y = 50.0f32;
        api.print(String::from("Game initialized!").as_str());
        api.print(api.JSON.stringify(&player).as_str());
    }

    fn update(&mut self, api: &mut ScriptApi<'_>) {
        let delta = api.delta();
        let mut self_node = api.get_node_clone::<Ui>(&self.node_id);
        api.print(&self_node.name);
        let mut x = delta;
        self_node.name = x.to_string();
        let mut player = Player::new();;
        player.health = 100i32;
        player.speed = 5.0f32;
        player.position_x = 10.0f32;
        player.position_y = 20.0f32;
        player.score = 1000.0f64;
        let mut enemy = Enemy::new();;
        enemy.health = 30i32;
        enemy.damage = 15i32;
        enemy.x = 50.0f64;
        enemy.y = 40.0f32;
        player.position_x = (player.position_x + player.speed);
        player.position_y = (player.position_y + 2.0f32);
        enemy.x = (enemy.x + 1.5f64);
        enemy.y += (0.5f32 + 1.0f32);
        let mut distance_x = (enemy.x + (player.position_x as f64));
        let mut distance_y = (enemy.y + player.position_y);
        let mut total_distance = (distance_x + (distance_y as f64));
        let mut collision_range = 5.0f32;
        player.health = (player.health + enemy.damage);
        player.health += (5i32 + 10i32);
        player.score += ((10.0f64 + 5.0f64) as f64);
        player.score = (player.score + 100.0f64);
        let mut regen_rate = 2i32;
        let mut bonus_regen = 3i32;
        player.health += (regen_rate + bonus_regen);
        let mut base_boost = 1.0f32;
        let mut time_boost = 0.5f32;
        player.speed = (player.speed + (base_boost + time_boost));
        player.speed += (0.25f32 + (0.25f32 + 0.5f32));
        let mut health_diff = (player.max_health + player.health);
        enemy.damage = (enemy.damage + (1i32 + 2i32));
        enemy.damage += 5i32;
        let mut score_multiplier = 2.5f64;
        let mut base_points = 10.0f64;
        player.score += (base_points + score_multiplier);
        player.score = (player.score + ((50.0f64 + 25.0f64) as f64));

        api.merge_nodes(vec![self_node.to_scene_node()]);
    }

}


impl ScriptObject for MainPupScript {
    fn set_node_id(&mut self, id: Uuid) {
        self.node_id = id;
    }

    fn get_node_id(&self) -> Uuid {
        self.node_id
    }

    fn get_var(&self, name: &str) -> Option<Value> {
        match name {
            _ => None,
        }
    }

    fn set_var(&mut self, name: &str, val: Value) -> Option<()> {
        match name {
            _ => None,
        }
    }

    fn apply_exposed(&mut self, hashmap: &HashMap<String, Value>) {
        for (key, _) in hashmap.iter() {
            match key.as_str() {
                _ => {},
            }
        }
    }
}
