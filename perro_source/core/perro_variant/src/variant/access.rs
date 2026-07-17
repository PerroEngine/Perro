use super::*;

// -------------------- Accessors (extensible pattern) --------------------

impl Variant {
    #[inline]
    pub fn kind(&self) -> VariantKind {
        match self {
            Variant::Null => VariantKind::Null,
            Variant::Bool(_) => VariantKind::Bool,
            Variant::Number(_) => VariantKind::Number,
            Variant::String(_) => VariantKind::String,
            Variant::Bytes(_) => VariantKind::Bytes,
            Variant::ID(_) => VariantKind::ID,
            Variant::EngineStruct(_) => VariantKind::EngineStruct,
            Variant::Array(_) => VariantKind::Array,
            Variant::Object(_) => VariantKind::Object,
        }
    }

    #[deprecated(note = "use Variant::kind()")]
    #[inline]
    pub fn get_kind(&self) -> VariantKind {
        self.kind()
    }

    #[inline]
    pub fn kind_name(&self) -> &'static str {
        self.kind().as_str()
    }

    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Variant::Bool(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_number(&self) -> Option<Number> {
        match *self {
            Variant::Number(n) => Some(n),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i8(&self) -> Option<i8> {
        match *self {
            Variant::Number(Number::I8(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i16(&self) -> Option<i16> {
        match *self {
            Variant::Number(Number::I16(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i32(&self) -> Option<i32> {
        match *self {
            Variant::Number(Number::I32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Variant::Number(Number::I64(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_i128(&self) -> Option<i128> {
        match *self {
            Variant::Number(Number::I128(v)) => Some(v.get()),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u8(&self) -> Option<u8> {
        match *self {
            Variant::Number(Number::U8(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u16(&self) -> Option<u16> {
        match *self {
            Variant::Number(Number::U16(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u32(&self) -> Option<u32> {
        match *self {
            Variant::Number(Number::U32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Variant::Number(Number::U64(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_u128(&self) -> Option<u128> {
        match *self {
            Variant::Number(Number::U128(v)) => Some(v.get()),
            _ => None,
        }
    }

    #[inline]
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Variant::Number(Number::F32(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Variant::Number(Number::F64(v)) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variant::String(s) => Some(s),
            _ => None,
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Variant::Bytes(b) => Some(b),
            _ => None,
        }
    }

    #[inline]
    pub fn as_node(&self) -> Option<NodeID> {
        match *self {
            Variant::ID(IDs::Node(id)) => Some(id),
            _ => None,
        }
    }

    /// Node id if this holds one, else [`NodeID::nil`]. Convenience for the
    /// common "read a node-ref var, default to nil" pattern.
    #[inline]
    pub fn as_node_or_nil(&self) -> NodeID {
        self.as_node().unwrap_or(NodeID::nil())
    }

    #[inline]
    pub fn as_texture(&self) -> Option<TextureID> {
        match *self {
            Variant::ID(IDs::Texture(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_material(&self) -> Option<MaterialID> {
        match *self {
            Variant::ID(IDs::Material(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_mesh(&self) -> Option<MeshID> {
        match *self {
            Variant::ID(IDs::Mesh(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_animation(&self) -> Option<AnimationID> {
        match *self {
            Variant::ID(IDs::Animation(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_light(&self) -> Option<LightID> {
        match *self {
            Variant::ID(IDs::Light(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_signal(&self) -> Option<SignalID> {
        match *self {
            Variant::ID(IDs::Signal(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_audio_bus(&self) -> Option<AudioBusID> {
        match *self {
            Variant::ID(IDs::AudioBus(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_tag(&self) -> Option<TagID> {
        match *self {
            Variant::ID(IDs::Tag(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_preloaded_scene(&self) -> Option<PreloadedSceneID> {
        match *self {
            Variant::ID(IDs::PreloadedScene(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_id(&self) -> Option<IDs> {
        match *self {
            Variant::ID(id) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec2(&self) -> Option<Vector2> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec3(&self) -> Option<Vector3> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_vec4(&self) -> Option<Vector4> {
        match self {
            Variant::EngineStruct(EngineStruct::Vector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec2(&self) -> Option<IVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec3(&self) -> Option<IVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_ivec4(&self) -> Option<IVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::IVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec2(&self) -> Option<UVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec3(&self) -> Option<UVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_uvec4(&self) -> Option<UVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::UVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec2(&self) -> Option<UnitVector2> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec3(&self) -> Option<UnitVector3> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector3(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_unit_vec4(&self) -> Option<UnitVector4> {
        match self {
            Variant::EngineStruct(EngineStruct::UnitVector4(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix2(&self) -> Option<Matrix2> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix2(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix3(&self) -> Option<Matrix3> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix3(v)) => Some(**v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix4(&self) -> Option<Matrix4> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix4(v)) => Some(**v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_matrix2x2(&self) -> Option<Matrix<2, 2, f32>> {
        self.as_matrix2().map(Matrix::<2, 2>::from)
    }

    #[inline]
    pub fn as_matrix3x3(&self) -> Option<Matrix<3, 3, f32>> {
        self.as_matrix3().map(Matrix::<3, 3>::from)
    }

    #[inline]
    pub fn as_matrix4x4(&self) -> Option<Matrix<4, 4, f32>> {
        self.as_matrix4().map(Matrix::<4, 4>::from)
    }

    #[inline]
    pub fn matrix_shape(&self) -> Option<MatrixShape> {
        match self {
            Variant::EngineStruct(EngineStruct::Matrix2(_)) => {
                Some(MatrixShape::new(2, 2, MatrixCellType::F32))
            }
            Variant::EngineStruct(EngineStruct::Matrix3(_)) => {
                Some(MatrixShape::new(3, 3, MatrixCellType::F32))
            }
            Variant::EngineStruct(EngineStruct::Matrix4(_)) => {
                Some(MatrixShape::new(4, 4, MatrixCellType::F32))
            }
            Variant::Object(obj) => obj.get("rows")?.matrix_shape(),
            Variant::Array(rows) => {
                let first_row = rows.first()?.as_array()?;
                let cols = first_row.len();
                let mut cell_type = first_row
                    .first()
                    .and_then(matrix_cell_type_for_variant)
                    .unwrap_or(MatrixCellType::Null);

                for row in rows {
                    let row = row.as_array()?;
                    if row.len() != cols {
                        return None;
                    }

                    for cell in row {
                        let next_type = matrix_cell_type_for_variant(cell)?;
                        if cell_type != next_type {
                            cell_type = MatrixCellType::Mixed;
                        }
                    }
                }

                Some(MatrixShape::new(rows.len(), cols, cell_type))
            }
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform2(&self) -> Option<Transform2D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform2D(t)) => Some(**t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform3(&self) -> Option<Transform3D> {
        match self {
            Variant::EngineStruct(EngineStruct::Transform3D(t)) => Some(**t),
            _ => None,
        }
    }

    #[inline]
    pub fn as_transform2d(&self) -> Option<Transform2D> {
        self.as_transform2()
    }

    #[inline]
    pub fn as_transform3d(&self) -> Option<Transform3D> {
        self.as_transform3()
    }

    #[inline]
    pub fn as_quat(&self) -> Option<Quaternion> {
        match self {
            Variant::EngineStruct(EngineStruct::Quaternion(q)) => Some(*q),
            _ => None,
        }
    }

    #[inline]
    pub fn as_post_process_set(&self) -> Option<&PostProcessSet> {
        match self {
            Variant::EngineStruct(EngineStruct::PostProcessSet(v)) => Some(v.as_ref()),
            _ => None,
        }
    }

    #[inline]
    pub fn as_visual_accessibility_settings(&self) -> Option<VisualAccessibilitySettings> {
        match self {
            Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v)) => Some(*v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array(&self) -> Option<&[Variant]> {
        match self {
            Variant::Array(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Variant>> {
        match self {
            Variant::Array(v) => Some(v),
            _ => None,
        }
    }

    #[inline]
    pub fn as_object(&self) -> Option<&BTreeMap<Arc<str>, Variant>> {
        match self {
            Variant::Object(m) => Some(m),
            _ => None,
        }
    }

    #[inline]
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<Arc<str>, Variant>> {
        match self {
            Variant::Object(m) => Some(m),
            _ => None,
        }
    }
}

pub(super) fn matrix_cell_type_for_variant(value: &Variant) -> Option<MatrixCellType> {
    Some(match value {
        Variant::Null => MatrixCellType::Null,
        Variant::Bool(_) => MatrixCellType::Bool,
        Variant::Number(Number::I8(_)) => MatrixCellType::I8,
        Variant::Number(Number::I16(_)) => MatrixCellType::I16,
        Variant::Number(Number::I32(_)) => MatrixCellType::I32,
        Variant::Number(Number::I64(_)) => MatrixCellType::I64,
        Variant::Number(Number::I128(_)) => MatrixCellType::I128,
        Variant::Number(Number::U8(_)) => MatrixCellType::U8,
        Variant::Number(Number::U16(_)) => MatrixCellType::U16,
        Variant::Number(Number::U32(_)) => MatrixCellType::U32,
        Variant::Number(Number::U64(_)) => MatrixCellType::U64,
        Variant::Number(Number::U128(_)) => MatrixCellType::U128,
        Variant::Number(Number::F32(_)) => MatrixCellType::F32,
        Variant::Number(Number::F64(_)) => MatrixCellType::F64,
        Variant::String(_) => MatrixCellType::String,
        Variant::Bytes(_) => MatrixCellType::Bytes,
        Variant::ID(_) => MatrixCellType::ID,
        Variant::EngineStruct(EngineStruct::Vector2(_)) => MatrixCellType::Vector2,
        Variant::EngineStruct(EngineStruct::Vector3(_)) => MatrixCellType::Vector3,
        Variant::EngineStruct(EngineStruct::Vector4(_)) => MatrixCellType::Vector4,
        Variant::EngineStruct(EngineStruct::IVector2(_)) => MatrixCellType::IVector2,
        Variant::EngineStruct(EngineStruct::IVector3(_)) => MatrixCellType::IVector3,
        Variant::EngineStruct(EngineStruct::IVector4(_)) => MatrixCellType::IVector4,
        Variant::EngineStruct(EngineStruct::UVector2(_)) => MatrixCellType::UVector2,
        Variant::EngineStruct(EngineStruct::UVector3(_)) => MatrixCellType::UVector3,
        Variant::EngineStruct(EngineStruct::UVector4(_)) => MatrixCellType::UVector4,
        Variant::EngineStruct(EngineStruct::UnitVector2(_)) => MatrixCellType::UnitVector2,
        Variant::EngineStruct(EngineStruct::UnitVector3(_)) => MatrixCellType::UnitVector3,
        Variant::EngineStruct(EngineStruct::UnitVector4(_)) => MatrixCellType::UnitVector4,
        Variant::EngineStruct(EngineStruct::Matrix2(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(2, 2, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Matrix3(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(3, 3, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Matrix4(_)) => {
            MatrixCellType::Matrix(Box::new(MatrixShape::new(4, 4, MatrixCellType::F32)))
        }
        Variant::EngineStruct(EngineStruct::Transform2D(_)) => MatrixCellType::Transform2D,
        Variant::EngineStruct(EngineStruct::Transform3D(_)) => MatrixCellType::Transform3D,
        Variant::EngineStruct(EngineStruct::Quaternion(_)) => MatrixCellType::Quaternion,
        Variant::EngineStruct(EngineStruct::PostProcessSet(_)) => MatrixCellType::PostProcessSet,
        Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(_)) => {
            MatrixCellType::VisualAccessibilitySettings
        }
        Variant::Array(_) => value
            .matrix_shape()
            .map(|shape| MatrixCellType::Matrix(Box::new(shape)))
            .unwrap_or(MatrixCellType::Array),
        Variant::Object(obj) => obj
            .get("rows")
            .and_then(Variant::matrix_shape)
            .map(|shape| MatrixCellType::Matrix(Box::new(shape)))
            .unwrap_or(MatrixCellType::Object),
    })
}
