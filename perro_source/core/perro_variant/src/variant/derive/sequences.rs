use super::super::*;

impl<T, const N: usize> DeriveVariant for [T; N]
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        if items.len() != N {
            return None;
        }
        let mut out = Vec::with_capacity(N);
        for item in items {
            out.push(T::from_variant(item)?);
        }
        let boxed: Box<[T]> = out.into_boxed_slice();
        let boxed: Box<[T; N]> = boxed.try_into().ok()?;
        Some(*boxed)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        if items.len() != N {
            return None;
        }
        let mut out = Vec::with_capacity(N);
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        let boxed: Box<[T]> = out.into_boxed_slice();
        let boxed: Box<[T; N]> = boxed.try_into().ok()?;
        Some(*boxed)
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

impl<T> DeriveVariant for Box<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out.into_boxed_slice())
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(out.into_boxed_slice())
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(
            Vec::from(self)
                .into_iter()
                .map(DeriveVariant::into_variant)
                .collect(),
        )
    }
}

impl<T> DeriveVariant for Arc<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(Arc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(Arc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }
}

impl<T> DeriveVariant for Rc<[T]>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(Rc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(Rc::from(out.into_boxed_slice()))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }
}

impl<T> DeriveVariant for Vec<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
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

impl<T> DeriveVariant for VecDeque<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = VecDeque::with_capacity(items.len());
        for item in items {
            out.push_back(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = VecDeque::with_capacity(items.len());
        for item in items {
            out.push_back(T::from_owned_variant(item)?);
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

impl<T> DeriveVariant for LinkedList<T>
where
    T: DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = LinkedList::new();
        for item in items {
            out.push_back(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = LinkedList::new();
        for item in items {
            out.push_back(T::from_owned_variant(item)?);
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

impl<T> DeriveVariant for BinaryHeap<T>
where
    T: Ord + DeriveVariant,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        let items = value.as_array()?;
        let mut out = BinaryHeap::with_capacity(items.len());
        for item in items {
            out.push(T::from_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        let items = match value {
            Variant::Array(items) => items,
            _ => return None,
        };
        let mut out = BinaryHeap::with_capacity(items.len());
        for item in items {
            out.push(T::from_owned_variant(item)?);
        }
        Some(out)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::Array(self.iter().map(DeriveVariant::to_variant).collect())
    }

    #[inline]
    fn into_variant(self) -> Variant {
        Variant::Array(
            self.into_vec()
                .into_iter()
                .map(DeriveVariant::into_variant)
                .collect(),
        )
    }
}

macro_rules! impl_tuple_derive_variant {
    ($len:expr; $($ty:ident: $idx:tt),+ $(,)?) => {
        impl<$($ty),+> DeriveVariant for ($($ty,)+)
        where
            $($ty: DeriveVariant),+
        {
            #[inline]
            fn from_variant(value: &Variant) -> Option<Self> {
                let items = value.as_array()?;
                if items.len() != $len {
                    return None;
                }
                Some(($(<$ty as DeriveVariant>::from_variant(items.get($idx)?)?,)+))
            }

            #[inline]
            fn from_owned_variant(value: Variant) -> Option<Self> {
                let items = match value {
                    Variant::Array(items) => items,
                    _ => return None,
                };
                if items.len() != $len {
                    return None;
                }
                let mut items = items.into_iter();
                Some(($(<$ty as DeriveVariant>::from_owned_variant(items.next()?)?,)+))
            }

            #[inline]
            fn to_variant(&self) -> Variant {
                Variant::Array(vec![$(self.$idx.to_variant()),+])
            }

            #[inline]
            fn into_variant(self) -> Variant {
                Variant::Array(vec![$(self.$idx.into_variant()),+])
            }
        }

        impl<$($ty),+> VariantSchema for ($($ty,)+) {}
    };
}

impl_tuple_derive_variant!(2; A: 0, B: 1);
impl_tuple_derive_variant!(3; A: 0, B: 1, C: 2);
impl_tuple_derive_variant!(4; A: 0, B: 1, C: 2, D: 3);
impl_tuple_derive_variant!(5; A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_derive_variant!(6; A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);

pub(super) trait VariantObjectKey: Sized {
    fn from_arc_key(key: Arc<str>) -> Self;
    fn to_arc_key(&self) -> Arc<str>;
    fn into_arc_key(self) -> Arc<str>;
}
