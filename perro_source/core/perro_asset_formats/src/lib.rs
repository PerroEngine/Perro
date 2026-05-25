//! Shared current binary asset magic, version, and flag constants.

pub mod archive {
    pub const EXTENSION: &str = "perro";
    pub const MAGIC: [u8; 4] = *b"PRA1";
    pub const COMPRESSED_MAGIC: [u8; 4] = *b"PRC1";
    pub const VERSION: u32 = 1;
    pub const FLAG_COMPRESSED: u32 = 1 << 0;
}

pub mod pawdio {
    pub const EXTENSION: &str = "pawdio";
    pub const MAGIC: &[u8; 6] = b"PAWDIO";
    pub const VERSION: u32 = 1;
    pub const FLAG_ZLIB: u32 = 1 << 0;
}

pub mod pmesh {
    pub const EXTENSION: &str = "pmesh";
    pub const MAGIC: &[u8; 5] = b"PMESH";
    pub const VERSION_V1: u32 = 1;
    pub const VERSION_V2: u32 = 2;
    pub const VERSION: u32 = VERSION_V2;
    pub const FLAG_HAS_NORMAL: u32 = 1 << 0;
    pub const FLAG_HAS_UV0: u32 = 1 << 1;
    pub const FLAG_HAS_JOINTS: u32 = 1 << 2;
    pub const FLAG_HAS_WEIGHTS: u32 = 1 << 3;
    pub const FLAG_INDEX_U16: u32 = 1 << 4;
    pub const FLAG_WEIGHTS_UNORM8: u32 = 1 << 5;
    pub const FLAG_HAS_BLEND_SHAPE_NORMALS: u32 = 1 << 6;
    pub const FLAG_PAYLOAD_RAW: u32 = 1 << 31;
}

pub mod pskel {
    pub const EXTENSION: &str = "pskel";
    pub const EXTENSION_2D: &str = "pskel2d";
    pub const EXTENSION_3D: &str = "pskel3d";
    pub const MAGIC: &[u8; 5] = b"PSKEL";
    pub const VERSION: u32 = 1;
    pub const VERSION_2D: u32 = 1;
    pub const FLAG_PAYLOAD_RAW: u32 = 1 << 31;
    pub const BONE_FLAG_HAS_PARENT: u32 = 1 << 0;
    pub const BONE_FLAG_HAS_REST_POS: u32 = 1 << 1;
    pub const BONE_FLAG_HAS_REST_SCALE: u32 = 1 << 2;
    pub const BONE_FLAG_HAS_REST_ROT: u32 = 1 << 3;
    pub const BONE_FLAG_HAS_INV_POS: u32 = 1 << 4;
    pub const BONE_FLAG_HAS_INV_SCALE: u32 = 1 << 5;
    pub const BONE_FLAG_HAS_INV_ROT: u32 = 1 << 6;
}

pub mod ptex {
    pub const EXTENSION: &str = "ptex";
    pub const MAGIC: &[u8; 4] = b"PTEX";
    pub const VERSION: u32 = 1;
    pub const FLAG_FORMAT_MASK: u32 = 0b11;
    pub const FLAG_FORMAT_RGBA8: u32 = 0;
    pub const FLAG_FORMAT_RGB8: u32 = 1;
    pub const FLAG_FORMAT_R8: u32 = 2;
    pub const FLAG_PAYLOAD_RAW: u32 = 1 << 31;
}

pub mod ptset {
    pub const SOURCE_EXTENSION: &str = "ptileset";
    pub const EXTENSION: &str = "ptset";
    pub const MAGIC: &[u8; 5] = b"PTSET";
    pub const VERSION: u32 = 1;
}

pub mod source_ext {
    //! Shared source and generated asset extensions for archive packing and static build.

    pub const RUST_SCRIPT: &str = "rs";
    pub const SCENE: &str = "scn";
    pub const FUR: &str = "fur";
    pub const UI_STYLE: &str = "uistyle";
    pub const MATERIAL: &str = "pmat";
    pub const PARTICLE: &str = "ppart";
    pub const ANIMATION: &str = "panim";
    pub const ANIMATION_TREE: &str = "panimtree";
    pub const SHADER_WGSL: &str = "wgsl";
    pub const GLB: &str = "glb";
    pub const GLTF: &str = "gltf";

    pub const IMAGE: &[&str] = &[
        "png", "jpg", "jpeg", "bmp", "gif", "ico", "tga", "webp", "rgba", "svg",
    ];
    pub const AUDIO: &[&str] = &["mp3", "wav", "ogg", "flac", "aac", "m4a"];
    pub const MIDI: &[&str] = &["mid", "midi"];
    pub const SOUNDFONT: &[&str] = &["sf2"];
    pub const MODEL: &[&str] = &[GLB, GLTF];
    pub const MESH_INPUT: &[&str] = &[crate::pmesh::EXTENSION, GLB, GLTF];
    pub const SKELETON_INPUT: &[&str] = &[
        crate::pskel::EXTENSION,
        crate::pskel::EXTENSION_2D,
        crate::pskel::EXTENSION_3D,
        GLB,
        GLTF,
    ];
    pub const MATERIAL_INPUT: &[&str] = &[MATERIAL, GLB, GLTF];
    pub const STATIC_RESOURCE: &[&str] = &[
        MATERIAL,
        PARTICLE,
        crate::pmesh::EXTENSION,
        crate::pskel::EXTENSION,
        crate::pskel::EXTENSION_2D,
        crate::pskel::EXTENSION_3D,
        crate::ptset::SOURCE_EXTENSION,
        ANIMATION,
        ANIMATION_TREE,
        UI_STYLE,
    ];
    pub const SCENE_FUR: &[&str] = &[SCENE, FUR];
    pub const SHADER: &[&str] = &[SHADER_WGSL];

    pub fn contains(exts: &[&str], ext: &str) -> bool {
        exts.iter()
            .any(|candidate| ext.eq_ignore_ascii_case(candidate))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_versions_are_v1() {
        assert_eq!(super::archive::VERSION, 1);
        assert_eq!(super::pawdio::VERSION, 1);
        assert_eq!(super::pmesh::VERSION, 1);
        assert_eq!(super::pskel::VERSION, 1);
        assert_eq!(super::pskel::VERSION_2D, 1);
        assert_eq!(super::ptex::VERSION, 1);
        assert_eq!(super::ptset::VERSION, 1);
    }

    #[test]
    fn shared_extensions_cover_static_pipeline_inputs() {
        assert!(super::source_ext::contains(
            super::source_ext::MESH_INPUT,
            super::pmesh::EXTENSION,
        ));
        assert!(super::source_ext::contains(
            super::source_ext::SKELETON_INPUT,
            super::pskel::EXTENSION_2D,
        ));
        assert!(super::source_ext::contains(super::source_ext::IMAGE, "PNG",));
        assert!(super::source_ext::contains(super::source_ext::AUDIO, "WAV",));
        assert!(super::source_ext::contains(super::source_ext::MIDI, "MID",));
        assert!(super::source_ext::contains(
            super::source_ext::SOUNDFONT,
            "SF2",
        ));
    }
}
