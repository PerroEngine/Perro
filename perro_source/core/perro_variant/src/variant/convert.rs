use super::*;

// -------------------- From impls (ergonomic construction) --------------------

// Primitives
impl From<bool> for Variant {
    #[inline]
    fn from(v: bool) -> Self {
        Variant::Bool(v)
    }
}
impl From<Number> for Variant {
    #[inline]
    fn from(v: Number) -> Self {
        Variant::Number(v)
    }
}

// Signed ints
impl From<i8> for Variant {
    #[inline]
    fn from(v: i8) -> Self {
        Variant::Number(Number::I8(v))
    }
}
impl From<i16> for Variant {
    #[inline]
    fn from(v: i16) -> Self {
        Variant::Number(Number::I16(v))
    }
}
impl From<i32> for Variant {
    #[inline]
    fn from(v: i32) -> Self {
        Variant::Number(Number::I32(v))
    }
}
impl From<i64> for Variant {
    #[inline]
    fn from(v: i64) -> Self {
        Variant::Number(Number::I64(v))
    }
}
impl From<i128> for Variant {
    #[inline]
    fn from(v: i128) -> Self {
        Variant::Number(Number::I128(PackedI128::new(v)))
    }
}

// Unsigned ints
impl From<u8> for Variant {
    #[inline]
    fn from(v: u8) -> Self {
        Variant::Number(Number::U8(v))
    }
}
impl From<u16> for Variant {
    #[inline]
    fn from(v: u16) -> Self {
        Variant::Number(Number::U16(v))
    }
}
impl From<u32> for Variant {
    #[inline]
    fn from(v: u32) -> Self {
        Variant::Number(Number::U32(v))
    }
}
impl From<u64> for Variant {
    #[inline]
    fn from(v: u64) -> Self {
        Variant::Number(Number::U64(v))
    }
}
impl From<u128> for Variant {
    #[inline]
    fn from(v: u128) -> Self {
        Variant::Number(Number::U128(PackedU128::new(v)))
    }
}

// Floats
impl From<f32> for Variant {
    #[inline]
    fn from(v: f32) -> Self {
        Variant::Number(Number::F32(v))
    }
}
impl From<f64> for Variant {
    #[inline]
    fn from(v: f64) -> Self {
        Variant::Number(Number::F64(v))
    }
}

// Text/Binary
impl From<&str> for Variant {
    #[inline]
    fn from(v: &str) -> Self {
        Variant::String(Arc::<str>::from(v))
    }
}
impl From<String> for Variant {
    #[inline]
    fn from(v: String) -> Self {
        Variant::String(Arc::<str>::from(v))
    }
}
impl From<Arc<str>> for Variant {
    #[inline]
    fn from(v: Arc<str>) -> Self {
        Variant::String(v)
    }
}

impl From<&[u8]> for Variant {
    #[inline]
    fn from(v: &[u8]) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(v))
    }
}
impl From<Vec<u8>> for Variant {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Variant::Bytes(Arc::<[u8]>::from(v.into_boxed_slice()))
    }
}
impl From<Arc<[u8]>> for Variant {
    #[inline]
    fn from(v: Arc<[u8]>) -> Self {
        Variant::Bytes(v)
    }
}

impl<T> From<Arc<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: Arc<T>) -> Self {
        v.to_variant()
    }
}

impl<T> From<Rc<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: Rc<T>) -> Self {
        v.to_variant()
    }
}

impl<T> From<RefCell<T>> for Variant
where
    T: DeriveVariant,
{
    #[inline]
    fn from(v: RefCell<T>) -> Self {
        v.into_variant()
    }
}

// Engine handles
impl From<NodeID> for Variant {
    #[inline]
    fn from(v: NodeID) -> Self {
        Variant::ID(IDs::Node(v))
    }
}
impl From<TextureID> for Variant {
    #[inline]
    fn from(v: TextureID) -> Self {
        Variant::ID(IDs::Texture(v))
    }
}
impl From<MaterialID> for Variant {
    #[inline]
    fn from(v: MaterialID) -> Self {
        Variant::ID(IDs::Material(v))
    }
}
impl From<MeshID> for Variant {
    #[inline]
    fn from(v: MeshID) -> Self {
        Variant::ID(IDs::Mesh(v))
    }
}
impl From<AnimationID> for Variant {
    #[inline]
    fn from(v: AnimationID) -> Self {
        Variant::ID(IDs::Animation(v))
    }
}
impl From<AnimationTreeID> for Variant {
    #[inline]
    fn from(v: AnimationTreeID) -> Self {
        Variant::ID(IDs::AnimationTree(v))
    }
}
impl From<NavMeshID> for Variant {
    #[inline]
    fn from(v: NavMeshID) -> Self {
        Variant::ID(IDs::NavMesh(v))
    }
}
impl From<SoundFontID> for Variant {
    #[inline]
    fn from(v: SoundFontID) -> Self {
        Variant::ID(IDs::SoundFont(v))
    }
}
impl From<LightID> for Variant {
    #[inline]
    fn from(v: LightID) -> Self {
        Variant::ID(IDs::Light(v))
    }
}
impl From<SignalID> for Variant {
    #[inline]
    fn from(v: SignalID) -> Self {
        Variant::ID(IDs::Signal(v))
    }
}
impl From<AudioBusID> for Variant {
    #[inline]
    fn from(v: AudioBusID) -> Self {
        Variant::ID(IDs::AudioBus(v))
    }
}
impl From<TagID> for Variant {
    #[inline]
    fn from(v: TagID) -> Self {
        Variant::ID(IDs::Tag(v))
    }
}
impl From<PreloadedSceneID> for Variant {
    #[inline]
    fn from(v: PreloadedSceneID) -> Self {
        Variant::ID(IDs::PreloadedScene(v))
    }
}

// Math
impl From<Vector2> for Variant {
    #[inline]
    fn from(v: Vector2) -> Self {
        Variant::EngineStruct(EngineStruct::Vector2(v))
    }
}
impl From<Vector3> for Variant {
    #[inline]
    fn from(v: Vector3) -> Self {
        Variant::EngineStruct(EngineStruct::Vector3(v))
    }
}
impl From<Vector4> for Variant {
    #[inline]
    fn from(v: Vector4) -> Self {
        Variant::EngineStruct(EngineStruct::Vector4(v))
    }
}
impl From<IVector2> for Variant {
    #[inline]
    fn from(v: IVector2) -> Self {
        Variant::EngineStruct(EngineStruct::IVector2(v))
    }
}
impl From<IVector3> for Variant {
    #[inline]
    fn from(v: IVector3) -> Self {
        Variant::EngineStruct(EngineStruct::IVector3(v))
    }
}
impl From<IVector4> for Variant {
    #[inline]
    fn from(v: IVector4) -> Self {
        Variant::EngineStruct(EngineStruct::IVector4(v))
    }
}
impl From<UVector2> for Variant {
    #[inline]
    fn from(v: UVector2) -> Self {
        Variant::EngineStruct(EngineStruct::UVector2(v))
    }
}
impl From<UVector3> for Variant {
    #[inline]
    fn from(v: UVector3) -> Self {
        Variant::EngineStruct(EngineStruct::UVector3(v))
    }
}
impl From<UVector4> for Variant {
    #[inline]
    fn from(v: UVector4) -> Self {
        Variant::EngineStruct(EngineStruct::UVector4(v))
    }
}
impl From<UnitVector2> for Variant {
    #[inline]
    fn from(v: UnitVector2) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector2(v))
    }
}
impl From<UnitVector3> for Variant {
    #[inline]
    fn from(v: UnitVector3) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector3(v))
    }
}
impl From<UnitVector4> for Variant {
    #[inline]
    fn from(v: UnitVector4) -> Self {
        Variant::EngineStruct(EngineStruct::UnitVector4(v))
    }
}
impl From<Matrix2> for Variant {
    #[inline]
    fn from(v: Matrix2) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix2(v))
    }
}
impl From<Matrix3> for Variant {
    #[inline]
    fn from(v: Matrix3) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix3(Box::new(v)))
    }
}
impl From<Matrix4> for Variant {
    #[inline]
    fn from(v: Matrix4) -> Self {
        Variant::EngineStruct(EngineStruct::Matrix4(Box::new(v)))
    }
}
impl From<Matrix<2, 2, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<2, 2, f32>) -> Self {
        Variant::from(Matrix2::from(v))
    }
}
impl From<Matrix<3, 3, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<3, 3, f32>) -> Self {
        Variant::from(Matrix3::from(v))
    }
}
impl From<Matrix<4, 4, f32>> for Variant {
    #[inline]
    fn from(v: Matrix<4, 4, f32>) -> Self {
        Variant::from(Matrix4::from(v))
    }
}
impl From<Transform2D> for Variant {
    #[inline]
    fn from(v: Transform2D) -> Self {
        Variant::EngineStruct(EngineStruct::Transform2D(Box::new(v)))
    }
}
impl From<Transform3D> for Variant {
    #[inline]
    fn from(v: Transform3D) -> Self {
        Variant::EngineStruct(EngineStruct::Transform3D(Box::new(v)))
    }
}
impl From<Quaternion> for Variant {
    #[inline]
    fn from(v: Quaternion) -> Self {
        Variant::EngineStruct(EngineStruct::Quaternion(v))
    }
}
impl From<PostProcessSet> for Variant {
    #[inline]
    fn from(v: PostProcessSet) -> Self {
        Variant::EngineStruct(EngineStruct::PostProcessSet(Box::new(v)))
    }
}
impl From<VisualAccessibilitySettings> for Variant {
    #[inline]
    fn from(v: VisualAccessibilitySettings) -> Self {
        Variant::EngineStruct(EngineStruct::VisualAccessibilitySettings(v))
    }
}

// Containers
impl From<Vec<Variant>> for Variant {
    #[inline]
    fn from(v: Vec<Variant>) -> Self {
        Variant::Array(v)
    }
}
impl From<BTreeMap<Arc<str>, Variant>> for Variant {
    #[inline]
    fn from(v: BTreeMap<Arc<str>, Variant>) -> Self {
        Variant::Object(v)
    }
}
