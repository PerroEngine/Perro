use super::super::*;

impl<T> DeriveVariant for Option<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if matches!(value, Variant::Null) {
            Some(None)
        } else {
            T::from_variant(value).map(Some)
        }
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        if matches!(value, Variant::Null) {
            Some(None)
        } else {
            T::from_scene_variant(value, resolver).map(Some)
        }
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        if matches!(value, Variant::Null) {
            Some(None)
        } else {
            T::from_owned_variant(value).map(Some)
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        match self {
            Some(v) => v.to_variant(),
            None => Variant::Null,
        }
    }

    #[inline]
    fn into_variant(self) -> Variant {
        match self {
            Some(v) => v.into_variant(),
            None => Variant::Null,
        }
    }
}

/// Interned `Result` tag; clone is one atomic increment, no allocation.
fn result_tag(ok: bool) -> Arc<str> {
    static OK: std::sync::LazyLock<Arc<str>> = std::sync::LazyLock::new(|| Arc::from("Ok"));
    static ERR: std::sync::LazyLock<Arc<str>> = std::sync::LazyLock::new(|| Arc::from("Err"));
    Arc::clone(if ok { &OK } else { &ERR })
}

impl<T, E> DeriveVariant for Result<T, E>
where
    T: DeriveVariant,
    E: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        // Compact form: `["Ok"|"Err", payload]`.
        let (tag, data) = if let Some(arr) = value.as_array() {
            if arr.len() != 2 {
                return None;
            }
            (arr[0].as_str()?, &arr[1])
        } else {
            // Legacy `{__variant, __data}` object form.
            let obj = value.as_object()?;
            (obj.get("__variant")?.as_str()?, obj.get("__data")?)
        };
        match tag {
            "Ok" => T::from_variant(data).map(Ok),
            "Err" => E::from_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let (tag, data) = match value {
            Variant::Array(arr) => {
                if arr.len() != 2 {
                    return None;
                }
                let mut it = arr.into_iter();
                let tag = match it.next()? {
                    Variant::String(tag) => tag,
                    _ => return None,
                };
                (tag, it.next()?)
            }
            Variant::Object(mut obj) => {
                let tag = match obj.remove("__variant")? {
                    Variant::String(tag) => tag,
                    _ => return None,
                };
                (tag, obj.remove("__data")?)
            }
            _ => return None,
        };
        match tag.as_ref() {
            "Ok" => T::from_owned_variant(data).map(Ok),
            "Err" => E::from_owned_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        let (tag, data) = if let Some(arr) = value.as_array() {
            if arr.len() != 2 {
                return None;
            }
            (arr[0].as_str()?, &arr[1])
        } else {
            let obj = value.as_object()?;
            (obj.get("__variant")?.as_str()?, obj.get("__data")?)
        };
        match tag {
            "Ok" => T::from_scene_variant(data, resolver).map(Ok),
            "Err" => E::from_scene_variant(data, resolver).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let (ok, data) = match self {
            Ok(value) => (true, value.to_variant()),
            Err(err) => (false, err.to_variant()),
        };
        Variant::Array(vec![Variant::String(result_tag(ok)), data])
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let (ok, data) = match self {
            Ok(value) => (true, value.into_variant()),
            Err(err) => (false, err.into_variant()),
        };
        Variant::Array(vec![Variant::String(result_tag(ok)), data])
    }
}

impl<T> DeriveVariant for Box<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Box::new)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Box::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Box::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        (*self).into_variant()
    }
}

impl<T> DeriveVariant for Arc<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Arc::new)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Arc::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Arc::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.as_ref().to_variant()
    }
}

impl<T> DeriveVariant for Rc<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Rc::new)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Rc::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Rc::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.as_ref().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.as_ref().to_variant()
    }
}

impl<T> DeriveVariant for Cell<T>
where
    T: Copy + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Cell::new)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Cell::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Cell::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.get().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_inner().into_variant()
    }
}

macro_rules! impl_atomic_derive_variant {
    ($atomic:ty, $inner:ty) => {
        impl DeriveVariant for $atomic {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_variant(value).map(<$atomic>::new)
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                <$inner as DeriveVariant>::from_owned_variant(value).map(<$atomic>::new)
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                self.load(Ordering::SeqCst).to_variant()
            }

            #[inline]
            fn into_variant(self) -> Variant {
                self.into_inner().into_variant()
            }
        }
    };
}

impl_atomic_derive_variant!(AtomicBool, bool);
impl_atomic_derive_variant!(AtomicI32, i32);
impl_atomic_derive_variant!(AtomicI64, i64);
impl_atomic_derive_variant!(AtomicU32, u32);
impl_atomic_derive_variant!(AtomicU64, u64);
impl_atomic_derive_variant!(AtomicUsize, usize);

impl<T> DeriveVariant for RefCell<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(RefCell::new)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(RefCell::new)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(RefCell::new)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.borrow().to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.into_inner().into_variant()
    }
}

impl<T> DeriveVariant for Wrapping<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Wrapping)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Wrapping)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Wrapping)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}

impl<T> DeriveVariant for Saturating<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Saturating)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Saturating)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Saturating)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}

impl<T> DeriveVariant for Reverse<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        T::from_variant(value).map(Reverse)
    }
    #[inline]
    fn from_scene_variant(
        value: &Variant,
        resolver: &mut dyn SceneVariantResolver,
    ) -> Option<Self> {
        T::from_scene_variant(value, resolver).map(Reverse)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        T::from_owned_variant(value).map(Reverse)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        self.0.to_variant()
    }

    #[inline]
    fn into_variant(self) -> Variant {
        self.0.into_variant()
    }
}
