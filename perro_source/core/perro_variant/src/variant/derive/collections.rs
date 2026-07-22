use super::super::*;
use super::sequences::VariantObjectKey;

impl VariantObjectKey for Arc<str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        key
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::clone(self)
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        self
    }
}

impl VariantObjectKey for String {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        key.to_string()
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_str())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(self)
    }
}

impl VariantObjectKey for Box<str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        Box::from(key.as_ref())
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_ref())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(String::from(self))
    }
}

impl VariantObjectKey for Cow<'static, str> {
    #[inline]
    fn from_arc_key(key: Arc<str>) -> Self {
        Cow::Owned(key.to_string())
    }

    #[inline]
    fn to_arc_key(&self) -> Arc<str> {
        Arc::from(self.as_ref())
    }

    #[inline]
    fn into_arc_key(self) -> Arc<str> {
        Arc::from(self.into_owned())
    }
}

impl<K, T> DeriveVariant for BTreeMap<K, T>
where
    K: Ord + VariantObjectKey,
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(K::from_arc_key(Arc::clone(k)), T::from_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(
                K::from_arc_key(Arc::clone(k)),
                T::from_scene_variant(v, resolver)?,
            );
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let object = match value {
            Variant::Object(object) => object,
            _ => return None,
        };
        let mut out = BTreeMap::new();
        for (k, v) in object {
            out.insert(K::from_arc_key(k), T::from_owned_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.to_arc_key(), v.to_variant());
        }
        Variant::Object(out)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.into_arc_key(), v.into_variant());
        }
        Variant::Object(out)
    }
}

impl<K, T> DeriveVariant for HashMap<K, T>
where
    K: Eq + Hash + VariantObjectKey,
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = HashMap::with_capacity(object.len());
        for (k, v) in object {
            out.insert(K::from_arc_key(Arc::clone(k)), T::from_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        let object = value.as_object()?;
        let mut out = HashMap::with_capacity(object.len());
        for (k, v) in object {
            out.insert(
                K::from_arc_key(Arc::clone(k)),
                T::from_scene_variant(v, resolver)?,
            );
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let object = match value {
            Variant::Object(object) => object,
            _ => return None,
        };
        let mut out = HashMap::with_capacity(object.len());
        for (k, v) in object {
            out.insert(K::from_arc_key(k), T::from_owned_variant(v)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.to_arc_key(), v.to_variant());
        }
        Variant::Object(out)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut out = BTreeMap::new();
        for (k, v) in self {
            out.insert(k.into_arc_key(), v.into_variant());
        }
        Variant::Object(out)
    }
}

impl<T> DeriveVariant for BTreeSet<T>
where
    T: Ord + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = BTreeSet::new();
        for item in items {
            out.insert(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        value
            .as_array()?
            .iter()
            .map(|item| T::from_scene_variant(item, resolver))
            .collect()
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = BTreeSet::new();
        for item in items {
            out.insert(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for HashSet<T>
where
    T: Eq + Hash + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = HashSet::with_capacity(items.len());
        for item in items {
            out.insert(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        value
            .as_array()?
            .iter()
            .map(|item| T::from_scene_variant(item, resolver))
            .collect()
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = HashSet::with_capacity(items.len());
        for item in items {
            out.insert(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(self.into_iter().map(DeriveVariant::into_variant).collect())
    }
}

impl<T> DeriveVariant for Range<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        let start = T::from_variant(&items[0])?;
        let end = T::from_variant(&items[1])?;
        Some(start..end)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        Some(
            T::from_scene_variant(&items[0], resolver)?
                ..T::from_scene_variant(&items[1], resolver)?,
        )
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != 2 {
            return None;
        }
        let mut items = items.into_iter();
        let start = T::from_owned_variant(items.next()?)?;
        let end = T::from_owned_variant(items.next()?)?;
        Some(start..end)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(vec![self.start.to_variant(), self.end.to_variant()])
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(vec![self.start.into_variant(), self.end.into_variant()])
    }
}

impl<T> DeriveVariant for RangeInclusive<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        let start = T::from_variant(&items[0])?;
        let end = T::from_variant(&items[1])?;
        Some(start..=end)
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != 2 {
            return None;
        }
        Some(
            T::from_scene_variant(&items[0], resolver)?
                ..=T::from_scene_variant(&items[1], resolver)?,
        )
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != 2 {
            return None;
        }
        let mut items = items.into_iter();
        let start = T::from_owned_variant(items.next()?)?;
        let end = T::from_owned_variant(items.next()?)?;
        Some(start..=end)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(vec![self.start().to_variant(), self.end().to_variant()])
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let (start, end) = self.into_inner();
        Variant::Array(vec![start.into_variant(), end.into_variant()])
    }
}

impl DeriveVariant for Duration {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(secs) = value.as_f64() {
            return Duration::try_from_secs_f64(secs).ok();
        }
        if let Some(secs) = value.as_u64() {
            return Some(Duration::from_secs(secs));
        }
        let obj = value.as_object()?;
        let secs = obj.get("secs")?.as_u64()?;
        let nanos = obj.get("nanos")?.as_u32()?;
        (nanos < 1_000_000_000).then_some(Duration::new(secs, nanos))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut out = BTreeMap::new();
        out.insert(Arc::from("secs"), Variant::from(self.as_secs()));
        out.insert(Arc::from("nanos"), Variant::from(self.subsec_nanos()));
        Variant::Object(out)
    }
}

impl DeriveVariant for SystemTime {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let duration = Duration::from_variant(value)?;
        UNIX_EPOCH.checked_add(duration)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let duration = Duration::from_owned_variant(value)?;
        UNIX_EPOCH.checked_add(duration)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.duration_since(UNIX_EPOCH)
            .map(|duration| duration.to_variant())
            .unwrap_or(Variant::Null)
    }
}
