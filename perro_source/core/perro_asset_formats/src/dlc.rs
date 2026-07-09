//! Stable C ABI types for DLC asset discovery.

use std::mem::size_of;

pub const REGISTRY_ABI_VERSION: u32 = 1;
pub const REGISTRY_API_SYMBOL: &[u8] = b"perro_dlc_pack_registry_api\0";
pub const NO_ENGINE_ABI_FINGERPRINT: [u8; 32] = [0; 32];

/// Stable asset type ID.
///
/// Keep this as an integer wrapper. A C ABI enum makes unknown future values
/// undefined behavior in old loaders.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct DlcAssetKind(u32);

impl DlcAssetKind {
    pub const UNKNOWN: Self = Self(0);
    pub const SCENE: Self = Self(1);
    pub const MATERIAL: Self = Self(2);
    pub const UI_STYLE: Self = Self(3);
    pub const TILE_SET: Self = Self(4);
    pub const PARTICLE: Self = Self(5);
    pub const ANIMATION: Self = Self(6);
    pub const ANIMATION_TREE: Self = Self(7);
    pub const MESH: Self = Self(8);
    pub const COLLISION_TRIMESH: Self = Self(9);
    pub const SKELETON: Self = Self(10);
    pub const TEXTURE: Self = Self(11);
    pub const SHADER: Self = Self(12);
    pub const AUDIO: Self = Self(13);
    pub const CSV: Self = Self(14);
    pub const LOCALIZATION: Self = Self(15);
    pub const FILE: Self = Self(16);

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn name(self) -> Option<&'static str> {
        match self {
            Self::SCENE => Some("scene"),
            Self::MATERIAL => Some("material"),
            Self::UI_STYLE => Some("ui_style"),
            Self::TILE_SET => Some("tile_set"),
            Self::PARTICLE => Some("particle"),
            Self::ANIMATION => Some("animation"),
            Self::ANIMATION_TREE => Some("animation_tree"),
            Self::MESH => Some("mesh"),
            Self::COLLISION_TRIMESH => Some("collision_trimesh"),
            Self::SKELETON => Some("skeleton"),
            Self::TEXTURE => Some("texture"),
            Self::SHADER => Some("shader"),
            Self::AUDIO => Some("audio"),
            Self::CSV => Some("csv"),
            Self::LOCALIZATION => Some("localization"),
            Self::FILE => Some("file"),
            _ => None,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct DlcAssetFlags(u32);

impl DlcAssetFlags {
    pub const NONE: Self = Self(0);
    /// Path names a pipeline-generated sub-asset, such as a GLTF mesh key.
    pub const SYNTHESIZED: Self = Self(1 << 0);

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }
}

/// Stable asset access mode ID.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct DlcAssetAccess(u32);

impl DlcAssetAccess {
    pub const UNKNOWN: Self = Self(0);
    /// Read through the ABI byte lookup function.
    pub const BYTES: Self = Self(1);
    /// Read through engine-private typed lookup only after fingerprint match.
    pub const ENGINE_LOCAL: Self = Self(2);

    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Borrowed registry metadata.
///
/// `path_ptr..path_len` holds a UTF-8 canonical asset URI. The pack owns this
/// memory. It stays valid until the loaded pack library unloads.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DlcRegistryEntryV1 {
    pub kind: DlcAssetKind,
    pub flags: DlcAssetFlags,
    pub access: DlcAssetAccess,
    /// Producers set zero. Loaders ignore this field.
    pub reserved: u32,
    pub path_hash: u64,
    pub path_ptr: *const u8,
    pub path_len: usize,
}

pub type DlcRegistryLenFnV1 = unsafe extern "C" fn() -> usize;
pub type DlcRegistryGetFnV1 =
    unsafe extern "C" fn(index: usize, entry_out: *mut DlcRegistryEntryV1) -> bool;
pub type DlcRegistryFindFnV1 = unsafe extern "C" fn(
    kind: DlcAssetKind,
    path_hash: u64,
    entry_out: *mut DlcRegistryEntryV1,
) -> bool;
pub type DlcRegistryLookupBytesFnV1 = unsafe extern "C" fn(
    kind: DlcAssetKind,
    path_hash: u64,
    data_out: *mut *const u8,
    data_len_out: *mut usize,
) -> bool;

/// Versioned pack registry function table.
///
/// `struct_size` lets loaders reject truncated tables and accept v1-compatible
/// tail fields. All function pointers remain valid until pack unload.
///
/// `engine_abi_fingerprint` gates engine-private typed pointers. Producers use
/// all zeroes when no entry uses `ENGINE_LOCAL`. Stable byte lookup never needs
/// an engine fingerprint match.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DlcRegistryApiV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub engine_abi_fingerprint: [u8; 32],
    pub registry_len: DlcRegistryLenFnV1,
    pub registry_get: DlcRegistryGetFnV1,
    pub registry_find: DlcRegistryFindFnV1,
    pub registry_lookup_bytes: DlcRegistryLookupBytesFnV1,
}

impl DlcRegistryApiV1 {
    pub const fn new(
        engine_abi_fingerprint: [u8; 32],
        registry_len: DlcRegistryLenFnV1,
        registry_get: DlcRegistryGetFnV1,
        registry_find: DlcRegistryFindFnV1,
        registry_lookup_bytes: DlcRegistryLookupBytesFnV1,
    ) -> Self {
        Self {
            abi_version: REGISTRY_ABI_VERSION,
            struct_size: size_of::<Self>() as u32,
            engine_abi_fingerprint,
            registry_len,
            registry_get,
            registry_find,
            registry_lookup_bytes,
        }
    }
}

/// Exported `perro_dlc_pack_registry_api` symbol signature.
///
/// Packs return null for unsupported versions. Loaders request an exact major
/// ABI and validate both header fields before reading function pointers.
pub type DlcRegistryApiQueryFn =
    unsafe extern "C" fn(requested_version: u32) -> *const DlcRegistryApiV1;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{needs_drop, offset_of};

    #[test]
    fn asset_kind_ids_stay_stable() {
        let kinds = [
            DlcAssetKind::SCENE,
            DlcAssetKind::MATERIAL,
            DlcAssetKind::UI_STYLE,
            DlcAssetKind::TILE_SET,
            DlcAssetKind::PARTICLE,
            DlcAssetKind::ANIMATION,
            DlcAssetKind::ANIMATION_TREE,
            DlcAssetKind::MESH,
            DlcAssetKind::COLLISION_TRIMESH,
            DlcAssetKind::SKELETON,
            DlcAssetKind::TEXTURE,
            DlcAssetKind::SHADER,
            DlcAssetKind::AUDIO,
            DlcAssetKind::CSV,
            DlcAssetKind::LOCALIZATION,
            DlcAssetKind::FILE,
        ];
        for (index, kind) in kinds.into_iter().enumerate() {
            assert_eq!(kind.raw(), index as u32 + 1);
            assert!(kind.name().is_some());
        }
        assert_eq!(DlcAssetKind::from_raw(99).name(), None);
    }

    #[test]
    fn registry_v1_layout_stays_stable_on_64_bit_targets() {
        if cfg!(target_pointer_width = "64") {
            assert_eq!(size_of::<DlcRegistryEntryV1>(), 40);
            assert_eq!(offset_of!(DlcRegistryEntryV1, kind), 0);
            assert_eq!(offset_of!(DlcRegistryEntryV1, flags), 4);
            assert_eq!(offset_of!(DlcRegistryEntryV1, access), 8);
            assert_eq!(offset_of!(DlcRegistryEntryV1, reserved), 12);
            assert_eq!(offset_of!(DlcRegistryEntryV1, path_hash), 16);
            assert_eq!(offset_of!(DlcRegistryEntryV1, path_ptr), 24);
            assert_eq!(offset_of!(DlcRegistryEntryV1, path_len), 32);

            assert_eq!(size_of::<DlcRegistryApiV1>(), 72);
            assert_eq!(offset_of!(DlcRegistryApiV1, abi_version), 0);
            assert_eq!(offset_of!(DlcRegistryApiV1, struct_size), 4);
            assert_eq!(offset_of!(DlcRegistryApiV1, engine_abi_fingerprint), 8);
            assert_eq!(offset_of!(DlcRegistryApiV1, registry_len), 40);
            assert_eq!(offset_of!(DlcRegistryApiV1, registry_get), 48);
            assert_eq!(offset_of!(DlcRegistryApiV1, registry_find), 56);
            assert_eq!(offset_of!(DlcRegistryApiV1, registry_lookup_bytes), 64);
        }
        assert!(!needs_drop::<DlcRegistryEntryV1>());
        assert!(!needs_drop::<DlcRegistryApiV1>());
    }

    #[test]
    fn asset_flags_preserve_unknown_bits() {
        let flags = DlcAssetFlags::from_raw(DlcAssetFlags::SYNTHESIZED.raw() | (1 << 31));
        assert!(flags.contains(DlcAssetFlags::SYNTHESIZED));
        assert_eq!(flags.raw(), (1 << 31) | 1);
    }

    #[test]
    fn access_modes_split_stable_bytes_from_engine_local_data() {
        assert_eq!(DlcAssetAccess::BYTES.raw(), 1);
        assert_eq!(DlcAssetAccess::ENGINE_LOCAL.raw(), 2);
        assert_eq!(DlcAssetAccess::from_raw(99).raw(), 99);
    }
}
