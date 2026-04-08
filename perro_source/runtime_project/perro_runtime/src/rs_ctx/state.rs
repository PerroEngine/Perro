use perro_animation::AnimationClip;
use perro_ids::{AnimationID, MaterialID, MeshID, TextureID};
use perro_project::LocalizationConfig;
use perro_resource_context::sub_apis::Locale;
use perro_render_bridge::{RenderCommand, RenderRequestID};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Default)]
struct LocalSlotArena {
    generations: Vec<u32>,
    occupied: Vec<bool>,
    free_slots: Vec<usize>,
}

impl LocalSlotArena {
    fn allocate_parts(&mut self) -> (u32, u32) {
        if let Some(slot) = self.free_slots.pop() {
            self.generations[slot] = self.generations[slot].wrapping_add(1);
            self.occupied[slot] = true;
            return ((slot + 1) as u32, self.generations[slot]);
        }

        let slot = self.generations.len();
        self.generations.push(0);
        self.occupied.push(true);
        ((slot + 1) as u32, 0)
    }

    fn free_parts(&mut self, index: u32, generation: u32) -> bool {
        if index == 0 {
            return false;
        }
        let slot = index as usize - 1;
        if slot >= self.generations.len() || slot >= self.occupied.len() {
            return false;
        }
        if !self.occupied[slot] || self.generations[slot] != generation {
            return false;
        }
        self.occupied[slot] = false;
        self.free_slots.push(slot);
        true
    }
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeLocalizationState {
    pub(super) source_csv: Option<String>,
    pub(super) key_column: String,
    pub(super) current_locale: Locale,
    pub(super) current_locale_code: &'static str,
    pub(super) active_by_key: HashMap<&'static str, &'static str>,
    pub(super) active_by_hash: HashMap<u64, &'static str>,
}

impl RuntimeLocalizationState {
    pub(super) fn new(config: Option<&LocalizationConfig>) -> Self {
        if let Some(config) = config {
            let locale_code = Box::leak(
                config
                    .default_locale
                    .trim()
                    .to_ascii_lowercase()
                    .into_boxed_str(),
            );
            return Self {
                source_csv: Some(config.source_csv.clone()),
                key_column: config.key_column.clone(),
                current_locale: locale_from_code(locale_code),
                current_locale_code: locale_code,
                active_by_key: HashMap::new(),
                active_by_hash: HashMap::new(),
            };
        }
        Self::default()
    }
}

impl Default for RuntimeLocalizationState {
    fn default() -> Self {
        Self {
            source_csv: None,
            key_column: "key".to_string(),
            current_locale: Locale::EN,
            current_locale_code: "en",
            active_by_key: HashMap::new(),
            active_by_hash: HashMap::new(),
        }
    }
}

fn locale_from_code(code: &'static str) -> Locale {
    match code {
        "zh" => Locale::ZH,
        "en" => Locale::EN,
        "ru" => Locale::RU,
        "es" => Locale::ES,
        "pt" => Locale::PT,
        "de" => Locale::DE,
        "ja" => Locale::JA,
        "fr" => Locale::FR,
        "ko" => Locale::KO,
        "pl" => Locale::PL,
        "tr" => Locale::TR,
        "it" => Locale::IT,
        "nl" => Locale::NL,
        "vi" => Locale::VI,
        "id" => Locale::ID,
        "ar" => Locale::AR,
        "hi" => Locale::HI,
        "bn" => Locale::BN,
        "ur" => Locale::UR,
        "fa" => Locale::FA,
        "sw" => Locale::SW,
        _ => Locale::Custom(code),
    }
}

#[derive(Default)]
pub(super) struct RuntimeResourceState {
    pub(super) next_request: u64,
    texture_slots: LocalSlotArena,
    mesh_slots: LocalSlotArena,
    material_slots: LocalSlotArena,
    animation_slots: LocalSlotArena,
    pub(super) queued_commands: Vec<RenderCommand>,
    pub(super) texture_by_source: HashMap<String, TextureID>,
    pub(super) texture_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) texture_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) texture_pending_id_by_request: HashMap<RenderRequestID, TextureID>,
    pub(super) texture_reserve_pending: HashSet<String>,
    pub(super) texture_drop_pending: HashSet<String>,
    pub(super) mesh_by_source: HashMap<String, MeshID>,
    pub(super) mesh_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) mesh_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) mesh_pending_id_by_request: HashMap<RenderRequestID, MeshID>,
    pub(super) mesh_reserve_pending: HashSet<String>,
    pub(super) mesh_drop_pending: HashSet<String>,
    pub(super) material_by_source: HashMap<String, MaterialID>,
    pub(super) material_pending_by_source: HashMap<String, RenderRequestID>,
    pub(super) material_pending_source_by_request: HashMap<RenderRequestID, String>,
    pub(super) material_pending_id_by_request: HashMap<RenderRequestID, MaterialID>,
    pub(super) material_reserve_pending: HashSet<String>,
    pub(super) material_drop_pending: HashSet<String>,
    pub(super) animation_by_source: HashMap<String, AnimationID>,
    pub(super) animation_data_by_id: HashMap<AnimationID, Arc<AnimationClip>>,
}

impl RuntimeResourceState {
    const REQUEST_BASE: u64 = 0x1000_0000_0000_0000;

    pub(super) fn new() -> Self {
        Self {
            next_request: Self::REQUEST_BASE,
            ..Self::default()
        }
    }

    pub(super) fn allocate_request(&mut self) -> RenderRequestID {
        let request = RenderRequestID::new(self.next_request);
        self.next_request = self.next_request.wrapping_add(1);
        request
    }

    pub(super) fn allocate_texture_id(&mut self) -> TextureID {
        let (index, generation) = self.texture_slots.allocate_parts();
        TextureID::from_parts(index, generation)
    }

    pub(super) fn allocate_mesh_id(&mut self) -> MeshID {
        let (index, generation) = self.mesh_slots.allocate_parts();
        MeshID::from_parts(index, generation)
    }

    pub(super) fn allocate_material_id(&mut self) -> MaterialID {
        let (index, generation) = self.material_slots.allocate_parts();
        MaterialID::from_parts(index, generation)
    }

    pub(super) fn free_texture_id(&mut self, id: TextureID) -> bool {
        self.texture_slots.free_parts(id.index(), id.generation())
    }

    pub(super) fn free_mesh_id(&mut self, id: MeshID) -> bool {
        self.mesh_slots.free_parts(id.index(), id.generation())
    }

    pub(super) fn free_material_id(&mut self, id: MaterialID) -> bool {
        self.material_slots.free_parts(id.index(), id.generation())
    }

    pub(super) fn allocate_animation_id(&mut self) -> AnimationID {
        let (index, generation) = self.animation_slots.allocate_parts();
        AnimationID::from_parts(index, generation)
    }

    pub(super) fn free_animation_id(&mut self, id: AnimationID) -> bool {
        self.animation_slots.free_parts(id.index(), id.generation())
    }
}
