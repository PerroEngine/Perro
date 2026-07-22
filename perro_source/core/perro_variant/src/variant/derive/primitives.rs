use super::super::*;

// -------------------- Constructors --------------------

impl Variant {
    #[inline]
    pub const fn null() -> Self {
        Variant::Null
    }
    #[inline]
    pub const fn is_null(&self) -> bool {
        matches!(self, Variant::Null)
    }

    #[inline]
    pub fn string<S: AsRef<str>>(s: S) -> Self {
        Variant::String(Arc::<str>::from(s.as_ref()))
    }

    #[inline]
    pub fn bytes<B: AsRef<[u8]>>(b: B) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(b.as_ref()))
    }

    #[inline]
    pub fn object() -> Self {
        Variant::Object(BTreeMap::new())
    }

    #[inline]
    pub fn array() -> Self {
        Variant::Array(Vec::new())
    }

    /// Decode this [`Variant`] into typed value via [`DeriveVariant`].
    ///
    /// Returns `Err` when shape/type does not match `T`.
    ///
    /// # Example
    /// ```rust
    /// use perro_variant::Variant;
    ///
    /// let value = Variant::from(42_i32);
    /// let parsed = value.parse::<i32>();
    /// assert_eq!(parsed, Ok(42));
    /// ```
    #[inline]
    pub fn parse<T>(&self) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        T::from_variant(self).ok_or_else(|| VariantParseError {
            target: std::any::type_name::<T>(),
            actual: self.kind_name(),
        })
    }

    /// Decode scene-authored data with resource-path coercion enabled.
    #[inline]
    pub fn parse_scene<T>(
        &self,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        T::from_scene_variant(self, resolver).ok_or_else(|| VariantParseError {
            target: std::any::type_name::<T>(),
            actual: self.kind_name(),
        })
    }

    /// Decode this [`Variant`] into typed value, returning `None` on mismatch.
    #[inline]
    pub fn as_type<T>(&self) -> Option<T>
    where
        T: DeriveVariant,
    {
        T::from_variant(self)
    }

    /// Check whether this [`Variant`] can decode into `T`.
    #[inline]
    pub fn is_type<T>(&self) -> bool
    where
        T: DeriveVariant,
    {
        T::from_variant(self).is_some()
    }

    /// Decode this [`Variant`] into typed value while consuming the variant.
    ///
    /// This avoids clone work for container-heavy values when the caller no
    /// longer needs the intermediate variant.
    #[inline]
    pub fn into_parse<T>(self) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        let actual = self.kind_name();
        T::from_owned_variant(self).ok_or_else(|| VariantParseError {
            target: std::any::type_name::<T>(),
            actual,
        })
    }

    /// Consume + decode scene-authored data with resource-path coercion.
    #[inline]
    pub fn into_parse_scene<T>(
        self,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Result<T, VariantParseError>
    where
        T: DeriveVariant,
    {
        let actual = self.kind_name();
        T::from_scene_variant(&self, resolver).ok_or_else(|| VariantParseError {
            target: std::any::type_name::<T>(),
            actual,
        })
    }

    /// Decode this [`Variant`] into typed value while consuming it, returning `None` on mismatch.
    #[inline]
    pub fn into_type<T>(self) -> Option<T>
    where
        T: DeriveVariant,
    {
        T::from_owned_variant(self)
    }
}

impl DeriveVariant for Variant {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        Some(value.clone())
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        Some(value)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.clone()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self
    }
}

impl DeriveVariant for bool {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_bool()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for () {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        matches!(value, Variant::Null).then_some(())
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Null
    }
}

macro_rules! impl_statefield_signed {
    ($ty:ty, $pat:pat => $expr:expr) => {
        impl DeriveVariant for $ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                match value.as_number() {
                    Some($pat) => Some($expr),
                    _ => None,
                }
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

macro_rules! impl_statefield_unsigned {
    ($ty:ty, $pat:pat => $expr:expr) => {
        impl DeriveVariant for $ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                match value.as_number() {
                    Some($pat) => Some($expr),
                    _ => None,
                }
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

impl_statefield_signed!(i8, Number::I8(v) => v);
impl_statefield_signed!(i16, Number::I16(v) => v);
impl_statefield_signed!(i32, Number::I32(v) => v);
impl_statefield_signed!(i64, Number::I64(v) => v);
impl_statefield_signed!(i128, Number::I128(v) => v.get());

impl_statefield_unsigned!(u8, Number::U8(v) => v);
impl_statefield_unsigned!(u16, Number::U16(v) => v);
impl_statefield_unsigned!(u32, Number::U32(v) => v);
impl_statefield_unsigned!(u64, Number::U64(v) => v);
impl_statefield_unsigned!(u128, Number::U128(v) => v.get());

macro_rules! impl_nonzero_derive_variant {
    ($nonzero:ty, $inner:ty) => {
        impl DeriveVariant for $nonzero {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_variant(value).and_then(Self::new)
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_owned_variant(value).and_then(Self::new)
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                <$inner as DeriveVariant>::to_variant(&self.get())
            }
        }
    };
}

impl_nonzero_derive_variant!(NonZeroI8, i8);
impl_nonzero_derive_variant!(NonZeroI16, i16);
impl_nonzero_derive_variant!(NonZeroI32, i32);
impl_nonzero_derive_variant!(NonZeroI64, i64);
impl_nonzero_derive_variant!(NonZeroI128, i128);
impl_nonzero_derive_variant!(NonZeroIsize, isize);
impl_nonzero_derive_variant!(NonZeroU8, u8);
impl_nonzero_derive_variant!(NonZeroU16, u16);
impl_nonzero_derive_variant!(NonZeroU32, u32);
impl_nonzero_derive_variant!(NonZeroU64, u64);
impl_nonzero_derive_variant!(NonZeroU128, u128);
impl_nonzero_derive_variant!(NonZeroUsize, usize);

impl DeriveVariant for isize {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        match value.as_number() {
            Some(Number::I64(v)) => isize::try_from(v).ok(),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        i64::try_from(*self)
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for usize {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        match value.as_number() {
            Some(Number::U64(v)) => usize::try_from(v).ok(),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        u64::try_from(*self)
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for f32 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Unit {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f32().map(Self::new).or_else(|| {
            value
                .as_number()
                .and_then(|value| value.as_u64_lossy())
                .and_then(|value| u8::try_from(value).ok())
                .map(Self::from_u8)
        })
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.to_f32())
    }
}

impl DeriveVariant for f64 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_f64()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for String {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(ToString::to_string)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        value.as_str().map(ToString::to_string)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.clone())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for char {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let mut chars = value.as_str()?.chars();
        let out = chars.next()?;
        chars.next().is_none().then_some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.to_string())
    }
}

impl DeriveVariant for Arc<str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(Arc::<str>::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(Arc::clone(self))
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self)
    }
}

impl DeriveVariant for Box<str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(Box::<str>::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(Box::<str>::from(v.as_ref())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_ref())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(String::from(self))
    }
}

impl DeriveVariant for Cow<'static, str> {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(|value| Cow::Owned(value.to_string()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(Cow::Owned(v.to_string())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(self.as_ref())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::from(self.into_owned())
    }
}

impl DeriveVariant for PathBuf {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_str().map(PathBuf::from)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::String(v) => Some(PathBuf::from(v.as_ref())),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.to_str().map(Variant::from).unwrap_or(Variant::Null)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_os_string()
            .into_string()
            .map(Variant::from)
            .unwrap_or(Variant::Null)
    }
}

impl DeriveVariant for NodeID {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_node()
            .or_else(|| {
                value
                    .as_number()
                    .and_then(|n| n.as_i64_lossy())
                    .and_then(|n| u64::try_from(n).ok())
                    .map(NodeID::from_u64)
            })
            .or_else(|| value.as_str().and_then(|s| NodeID::parse_str(s).ok()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for TextureID {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_texture()
            .or_else(|| {
                value
                    .as_number()
                    .and_then(|n| n.as_i64_lossy())
                    .and_then(|n| u64::try_from(n).ok())
                    .map(TextureID::from_u64)
            })
            .or_else(|| value.as_str().and_then(|s| TextureID::parse_str(s).ok()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        Self::from_variant(value).or_else(|| {
            resolve_scene_asset(
                value,
                resolver,
                SceneAssetKind::Texture,
                Variant::as_texture,
            )
        })
    }
}

macro_rules! impl_statefield_plain_id {
    ($id_ty:ty) => {
        impl DeriveVariant for $id_ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                value
                    .as_number()
                    .and_then(|n| n.as_u64_lossy())
                    .map(<$id_ty>::from_u64)
                    .or_else(|| value.as_str().and_then(|s| s.parse::<$id_ty>().ok()))
                    .or_else(|| value.as_id().map(IDs::as_u64).map(<$id_ty>::from_u64))
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

impl_statefield_plain_id!(LightID);
impl_statefield_plain_id!(SignalID);
impl_statefield_plain_id!(AudioBusID);
impl_statefield_plain_id!(TagID);
impl_statefield_plain_id!(PreloadedSceneID);

macro_rules! impl_scene_asset_id {
    ($id_ty:ty, $kind:ident, $accessor:ident) => {
        impl DeriveVariant for $id_ty {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                value
                    .$accessor()
                    .or_else(|| {
                        value
                            .as_number()
                            .and_then(|n| n.as_u64_lossy())
                            .map(<$id_ty>::from_u64)
                    })
                    .or_else(|| value.as_str().and_then(|s| s.parse::<$id_ty>().ok()))
                    .or_else(|| value.as_id().map(IDs::as_u64).map(<$id_ty>::from_u64))
            }

            #[inline]
            fn from_scene_variant(
                value: &Variant,
                resolver: &mut dyn SceneVariantResolver,
            ) -> Option<Self> {
                Self::from_variant(value).or_else(|| {
                    resolve_scene_asset(value, resolver, SceneAssetKind::$kind, Variant::$accessor)
                })
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::from(*self)
            }
        }
    };
}

fn resolve_scene_asset<T>(
    value: &Variant,
    resolver: &mut dyn SceneVariantResolver,
    kind: SceneAssetKind,
    extract: fn(&Variant) -> Option<T>,
) -> Option<T> {
    let path = value.as_str()?;
    if !(path.starts_with("res://") || path.starts_with("dlc://") || path.starts_with("user://")) {
        return None;
    }
    resolver
        .resolve_asset(kind, path)
        .as_ref()
        .and_then(extract)
}

impl_scene_asset_id!(AnimationTreeID, AnimationTree, as_animation_tree);
impl_scene_asset_id!(NavMeshID, NavMesh, as_nav_mesh);
impl_scene_asset_id!(SoundFontID, SoundFont, as_sound_font);
impl_scene_asset_id!(MaterialID, Material, as_material);
impl_scene_asset_id!(MeshID, Mesh, as_mesh);
impl_scene_asset_id!(AnimationID, Animation, as_animation);
