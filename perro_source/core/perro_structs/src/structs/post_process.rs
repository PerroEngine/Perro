use std::borrow::Cow;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum PostProcessEffect {
    Blur {
        strength: f32,
    },
    Pixelate {
        size: f32,
    },
    Warp {
        waves: f32,
        strength: f32,
    },
    Vignette {
        strength: f32,
        radius: f32,
        softness: f32,
    },
    Crt {
        scanline_strength: f32,
        curvature: f32,
        chromatic: f32,
        vignette: f32,
    },
    ColorFilter {
        color: [f32; 3],
        strength: f32,
    },
    ReverseFilter {
        color: [f32; 3],
        strength: f32,
        softness: f32,
    },
    Bloom {
        strength: f32,
        threshold: f32,
        radius: f32,
    },
    Saturate {
        amount: f32,
    },
    BlackWhite {
        amount: f32,
    },
    Custom {
        shader_path: Cow<'static, str>,
        params: Vec<CustomPostParam>,
    },
}

pub type CustomPostParamValue = crate::ConstParamValue;

#[derive(Clone, Debug, PartialEq)]
pub struct CustomPostParam {
    pub name: Option<Cow<'static, str>>,
    pub value: CustomPostParamValue,
}

impl CustomPostParam {
    #[inline]
    pub fn named(name: impl Into<Cow<'static, str>>, value: CustomPostParamValue) -> Self {
        Self {
            name: Some(name.into()),
            value,
        }
    }

    #[inline]
    pub fn unnamed(value: CustomPostParamValue) -> Self {
        Self { name: None, value }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostProcessEntry {
    pub name: Option<Cow<'static, str>>,
    pub effect: PostProcessEffect,
}

impl PostProcessEntry {
    #[inline]
    pub fn named(name: impl Into<Cow<'static, str>>, effect: PostProcessEffect) -> Self {
        Self {
            name: Some(name.into()),
            effect,
        }
    }

    #[inline]
    pub fn unnamed(effect: PostProcessEffect) -> Self {
        Self { name: None, effect }
    }
}

#[derive(Clone, PartialEq)]
pub struct PostProcessSet {
    entries: Vec<PostProcessEntry>,
}

impl fmt::Debug for PostProcessSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostProcessSet")
            .field("entries", &self.entries)
            .finish()
    }
}

impl Default for PostProcessSet {
    fn default() -> Self {
        Self::new()
    }
}

impl PostProcessSet {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn from_effects(effects: Vec<PostProcessEffect>) -> Self {
        Self {
            entries: effects.into_iter().map(PostProcessEntry::unnamed).collect(),
        }
    }

    pub fn from_entries(entries: Vec<PostProcessEntry>) -> Self {
        Self { entries }
    }

    pub fn from_pairs(
        effects: Vec<PostProcessEffect>,
        mut names: Vec<Option<Cow<'static, str>>>,
    ) -> Self {
        if names.len() < effects.len() {
            names.resize_with(effects.len(), || None);
        } else if names.len() > effects.len() {
            names.truncate(effects.len());
        }
        Self {
            entries: effects
                .into_iter()
                .zip(names)
                .map(|(effect, name)| PostProcessEntry { name, effect })
                .collect(),
        }
    }

    pub fn entries(&self) -> &[PostProcessEntry] {
        &self.entries
    }

    pub fn entries_mut(&mut self) -> &mut [PostProcessEntry] {
        &mut self.entries
    }

    pub fn effects(&self) -> impl Iterator<Item = &PostProcessEffect> {
        self.entries.iter().map(|entry| &entry.effect)
    }

    pub fn effects_mut(&mut self) -> impl Iterator<Item = &mut PostProcessEffect> {
        self.entries.iter_mut().map(|entry| &mut entry.effect)
    }

    pub fn to_effects_vec(&self) -> Vec<PostProcessEffect> {
        self.effects().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = Option<&str>> {
        self.entries.iter().map(|entry| entry.name.as_deref())
    }

    pub fn get(&self, name: &str) -> Option<&PostProcessEffect> {
        self.entries
            .iter()
            .find(|entry| entry.name.as_deref() == Some(name))
            .map(|entry| &entry.effect)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut PostProcessEffect> {
        self.entries
            .iter_mut()
            .find(|entry| entry.name.as_deref() == Some(name))
            .map(|entry| &mut entry.effect)
    }

    pub fn add(&mut self, name: impl Into<Cow<'static, str>>, effect: PostProcessEffect) {
        let name = name.into();
        if let Some(idx) = self
            .entries
            .iter()
            .position(|entry| entry.name.as_deref() == Some(name.as_ref()))
        {
            self.entries[idx].effect = effect;
        } else {
            self.entries.push(PostProcessEntry::named(name, effect));
        }
    }

    pub fn add_unnamed(&mut self, effect: PostProcessEffect) {
        self.entries.push(PostProcessEntry::unnamed(effect));
    }

    pub fn remove(&mut self, name: &str) -> Option<PostProcessEffect> {
        let idx = self
            .entries
            .iter()
            .position(|entry| entry.name.as_deref() == Some(name))?;
        self.remove_index(idx)
    }

    pub fn remove_index(&mut self, index: usize) -> Option<PostProcessEffect> {
        if index >= self.entries.len() {
            return None;
        }
        Some(self.entries.remove(index).effect)
    }

    pub fn rename(&mut self, old: &str, new: impl Into<Cow<'static, str>>) -> bool {
        let idx = self
            .entries
            .iter()
            .position(|entry| entry.name.as_deref() == Some(old));
        let Some(idx) = idx else { return false };
        self.entries[idx].name = Some(new.into());
        true
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl From<Vec<PostProcessEffect>> for PostProcessSet {
    fn from(effects: Vec<PostProcessEffect>) -> Self {
        Self::from_effects(effects)
    }
}
