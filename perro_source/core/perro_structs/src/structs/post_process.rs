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
        params: Cow<'static, [CustomPostParam]>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CustomPostParamValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

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

#[derive(Clone, PartialEq)]
pub struct PostProcessSet {
    effects: Cow<'static, [PostProcessEffect]>,
    names: Cow<'static, [Option<Cow<'static, str>>]>,
}

impl fmt::Debug for PostProcessSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostProcessSet")
            .field("effects", &self.effects)
            .field("names", &self.names)
            .finish()
    }
}

impl Default for PostProcessSet {
    fn default() -> Self {
        Self::new()
    }
}

impl PostProcessSet {
    pub const fn new() -> Self {
        Self {
            effects: Cow::Borrowed(&[]),
            names: Cow::Borrowed(&[]),
        }
    }

    pub fn from_effects(effects: Vec<PostProcessEffect>) -> Self {
        let len = effects.len();
        Self {
            effects: Cow::Owned(effects),
            names: Cow::Owned(vec![None; len]),
        }
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
            effects: Cow::Owned(effects),
            names: Cow::Owned(names),
        }
    }

    pub fn as_slice(&self) -> &[PostProcessEffect] {
        self.effects.as_ref()
    }

    pub fn as_slice_mut(&mut self) -> &mut [PostProcessEffect] {
        self.effects.to_mut().as_mut_slice()
    }

    pub fn len(&self) -> usize {
        self.effects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = Option<&str>> {
        self.names.iter().map(|n| n.as_deref())
    }

    pub fn get(&self, name: &str) -> Option<&PostProcessEffect> {
        let idx = self.names.iter().position(|n| n.as_deref() == Some(name))?;
        self.effects.get(idx)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut PostProcessEffect> {
        let idx = self.names.iter().position(|n| n.as_deref() == Some(name))?;
        self.effects.to_mut().get_mut(idx)
    }

    pub fn add(&mut self, name: impl Into<Cow<'static, str>>, effect: PostProcessEffect) {
        let name = name.into();
        self.sync_lengths();
        if let Some(idx) = self
            .names
            .iter()
            .position(|n| n.as_deref() == Some(name.as_ref()))
        {
            self.effects.to_mut()[idx] = effect;
        } else {
            self.effects.to_mut().push(effect);
            self.names.to_mut().push(Some(name));
        }
    }

    pub fn add_unnamed(&mut self, effect: PostProcessEffect) {
        self.sync_lengths();
        self.effects.to_mut().push(effect);
        self.names.to_mut().push(None);
    }

    pub fn remove(&mut self, name: &str) -> Option<PostProcessEffect> {
        let idx = self.names.iter().position(|n| n.as_deref() == Some(name))?;
        let names = self.names.to_mut();
        let effects = self.effects.to_mut();
        if idx >= effects.len() || idx >= names.len() {
            return None;
        }
        names.remove(idx);
        Some(effects.remove(idx))
    }

    pub fn rename(&mut self, old: &str, new: impl Into<Cow<'static, str>>) -> bool {
        let idx = self.names.iter().position(|n| n.as_deref() == Some(old));
        let Some(idx) = idx else { return false };
        self.names.to_mut()[idx] = Some(new.into());
        true
    }

    pub fn clear(&mut self) {
        self.effects = Cow::Borrowed(&[]);
        self.names = Cow::Borrowed(&[]);
    }

    fn sync_lengths(&mut self) {
        let effect_len = self.effects.len();
        let names = self.names.to_mut();
        if names.len() < effect_len {
            names.resize_with(effect_len, || None);
        } else if names.len() > effect_len {
            names.truncate(effect_len);
        }
    }
}

impl From<Vec<PostProcessEffect>> for PostProcessSet {
    fn from(effects: Vec<PostProcessEffect>) -> Self {
        Self::from_effects(effects)
    }
}

impl AsRef<[PostProcessEffect]> for PostProcessSet {
    fn as_ref(&self) -> &[PostProcessEffect] {
        self.as_slice()
    }
}

impl AsMut<[PostProcessEffect]> for PostProcessSet {
    fn as_mut(&mut self) -> &mut [PostProcessEffect] {
        self.as_slice_mut()
    }
}
