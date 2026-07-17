use super::super::*;

impl DeriveVariant for Vector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        Some(Vector2::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Vector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        Some(Vector3::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Vector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_vec4() {
            return Some(v);
        }
        if let Variant::Array(values) = value
            && values.len() == 4
        {
            return Some(Self::new(
                values[0].as_f32()?,
                values[1].as_f32()?,
                values[2].as_f32()?,
                values[3].as_f32()?,
            ));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Vector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_number()?.as_i64_lossy()?;
        let y = obj.get("y")?.as_number()?.as_i64_lossy()?;
        Some(IVector2::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
        ))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_number()?.as_i64_lossy()?;
        let y = obj.get("y")?.as_number()?.as_i64_lossy()?;
        let z = obj.get("z")?.as_number()?.as_i64_lossy()?;
        Some(IVector3::new(
            i32::try_from(x).ok()?,
            i32::try_from(y).ok()?,
            i32::try_from(z).ok()?,
        ))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for IVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_ivec4() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = i32::try_from(obj.get("x")?.as_number()?.as_i64_lossy()?).ok()?;
        let y = i32::try_from(obj.get("y")?.as_number()?.as_i64_lossy()?).ok()?;
        let z = i32::try_from(obj.get("z")?.as_number()?.as_i64_lossy()?).ok()?;
        let w = i32::try_from(obj.get("w")?.as_number()?.as_i64_lossy()?).ok()?;
        Some(IVector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec2() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        Some(UVector2::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec3() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        let z = variant_to_u32(obj.get("z")?)?;
        Some(UVector3::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_uvec4() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = variant_to_u32(obj.get("x")?)?;
        let y = variant_to_u32(obj.get("y")?)?;
        let z = variant_to_u32(obj.get("z")?)?;
        let w = variant_to_u32(obj.get("w")?)?;
        Some(UVector4::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec2() {
            return Some(v);
        }
        if let Some(v) = value.as_vec2() {
            return Some(Self::from(v));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        Some(Self::new(x, y))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec3() {
            return Some(v);
        }
        if let Some(v) = value.as_vec3() {
            return Some(Self::from(v));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        Some(Self::new(x, y, z))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for UnitVector4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_unit_vec4() {
            return Some(v);
        }
        if let Some(v) = value.as_vec4() {
            return Some(Self::new(v.to_array()));
        }
        if let Variant::Array(values) = value
            && values.len() == 4
        {
            return Some(Self::new([
                values[0].as_f32()?,
                values[1].as_f32()?,
                values[2].as_f32()?,
                values[3].as_f32()?,
            ]));
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Self::new([x, y, z, w]))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix2 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix2()
            .or_else(|| parse_matrix_rows::<2>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix3 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix3()
            .or_else(|| parse_matrix_rows::<3>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Matrix4 {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value
            .as_matrix4()
            .or_else(|| parse_matrix_rows::<4>(value).map(Self::from_rows))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl<const ROWS: usize, const COLS: usize, T> DeriveVariant for Matrix<ROWS, COLS, T>
where
    T: VariantMatrixCell,
{
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        // Cell types handle their own lossy numeric coercion (see the
        // `VariantMatrixCell` impls for `f32`/`f64`), so a single direct
        // parse covers every rescuable shape. A `serde_json` round-trip
        // fallback used to run here on first-parse miss, but it could never
        // rescue integer cell types (JSON normalizes all integers to
        // `I64`/`U64`, which still fails an exact-match `iN`/`uN` parse)
        // and floats are now rescued directly without serializing at all.
        parse_matrix_rows_generic(value)
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        matrix_to_fast_variant(self).unwrap_or_else(|| matrix_to_variant_array(self))
    }
}

macro_rules! impl_variant_matrix_cell {
    ($($ty:ty),* $(,)?) => {
        $(
            impl VariantMatrixCell for $ty {
                #[inline]
                fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
                    <Self as DeriveVariant>::from_variant(value)
                }

                #[inline]
                fn to_matrix_cell_variant(&self) -> Variant {
                    <Self as DeriveVariant>::to_variant(self)
                }
            }
        )*
    };
}

impl_variant_matrix_cell!(
    bool,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    String,
    Vector2,
    Vector3,
    Vector4,
    IVector2,
    IVector3,
    IVector4,
    UVector2,
    UVector3,
    UVector4,
    UnitVector2,
    UnitVector3,
    UnitVector4,
);

impl VariantMatrixCell for f32 {
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        value.as_number()?.as_f64_lossy().map(|value| value as f32)
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }

    #[inline]
    fn as_matrix_cell_f32(&self) -> Option<f32> {
        Some(*self)
    }
}

impl VariantMatrixCell for f64 {
    // Lossy numeric retry (mirrors the `from_matrix_cell_variant` half of
    // the `f32` impl above): accepts any numeric variant, not just an exact
    // `Number::F64` match. This is what rescues e.g. a matrix cell that was
    // authored/typed as `f32` but the matrix element type is `f64` —
    // previously handled by round-tripping the whole `Variant` through
    // `serde_json`, which only ever widened `F32 -> F64` anyway (integer
    // cell types can never be rescued by a JSON round trip, since ints
    // always come back as `I64`/`U64`).
    //
    // Note: `as_matrix_cell_f32` is intentionally left at the trait default
    // (`None`), not overridden to `Some(*self as f32)` — doing so would
    // route square f64 matrices through the f32-backed `Matrix2/3/4` fast
    // `to_variant()` path and silently lose precision on serialize. This
    // fix only touches deserialization (`from_variant`).
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        value.as_number()?.as_f64_lossy()
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }
}

impl<const ROWS: usize, const COLS: usize, T> VariantMatrixCell for Matrix<ROWS, COLS, T>
where
    T: VariantMatrixCell,
{
    #[inline]
    fn from_matrix_cell_variant(value: &Variant) -> Option<Self> {
        <Self as DeriveVariant>::from_variant(value)
    }

    #[inline]
    fn to_matrix_cell_variant(&self) -> Variant {
        <Self as DeriveVariant>::to_variant(self)
    }
}

impl DeriveVariant for Quaternion {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_quat() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let x = obj.get("x")?.as_f32()?;
        let y = obj.get("y")?.as_f32()?;
        let z = obj.get("z")?.as_f32()?;
        let w = obj.get("w")?.as_f32()?;
        Some(Quaternion::new(x, y, z, w))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Transform2D {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_transform2d() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let position = Vector2::from_variant(obj.get("position")?)?;
        let scale = Vector2::from_variant(obj.get("scale")?)?;
        let rotation = obj.get("rotation")?.as_f32()?;
        Some(Transform2D::new(position, rotation, scale))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for Transform3D {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        if let Some(v) = value.as_transform3d() {
            return Some(v);
        }
        let obj = value.as_object()?;
        let position = Vector3::from_variant(obj.get("position")?)?;
        let scale = Vector3::from_variant(obj.get("scale")?)?;
        let rotation = Quaternion::from_variant(obj.get("rotation")?)?;
        Some(Transform3D::new(position, rotation, scale))
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}

impl DeriveVariant for PostProcessSet {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_post_process_set().cloned()
    }

    #[inline]
    fn from_owned_variant(value: Variant) -> Option<Self> {
        match value {
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => Some(v),
            _ => None,
        }
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

impl DeriveVariant for VisualAccessibilitySettings {
    #[inline]
    fn from_variant(value: &Variant) -> Option<Self> {
        value.as_visual_accessibility_settings()
    }

    #[inline]
    fn to_variant(&self) -> Variant {
        Variant::from(*self)
    }
}
