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

impl<T, E> DeriveVariant for Result<T, E>
where
    T: DeriveVariant,
    E: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let obj = value.as_object()?;
        let tag = obj.get("__variant")?.as_str()?;
        let data = obj.get("__data")?;
        match tag {
            "Ok" => T::from_variant(data).map(Ok),
            "Err" => E::from_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let mut obj = match value {
            Variant::Object(obj) => obj,
            _ => return None,
        };
        let tag = match obj.remove("__variant")? {
            Variant::String(tag) => tag,
            _ => return None,
        };
        let data = obj.remove("__data")?;
        match tag.as_ref() {
            "Ok" => T::from_owned_variant(data).map(Ok),
            "Err" => E::from_owned_variant(data).map(Err),
            _ => None,
        }
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        let mut obj = BTreeMap::new();
        match self {
            Ok(value) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Ok")));
                obj.insert(Arc::from("__data"), value.to_variant());
            }
            Err(err) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Err")));
                obj.insert(Arc::from("__data"), err.to_variant());
            }
        }
        Variant::Object(obj)
    }

    #[inline]
    fn into_variant(self) -> Variant {
        let mut obj = BTreeMap::new();
        match self {
            Ok(value) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Ok")));
                obj.insert(Arc::from("__data"), value.into_variant());
            }
            Err(err) => {
                obj.insert(Arc::from("__variant"), Variant::String(Arc::from("Err")));
                obj.insert(Arc::from("__data"), err.into_variant());
            }
        }
        Variant::Object(obj)
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
