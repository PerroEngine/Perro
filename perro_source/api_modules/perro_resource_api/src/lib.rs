//! Public resource scripting API.
//!
//! This crate exposes script access to loaded resources and asset documents:
//! textures, audio, CSV tables, meshes, materials, GLTF metadata, skeletons,
//! animations, animation trees, 2D draw data, localization, scene documents,
//! viewport data, post-processing, and visual accessibility.

pub mod api;
pub mod load_error;
pub mod res_path;
pub mod sub_apis;

// ---- Resource facade ----

pub use api::ResourceWindow;
pub use load_error::{LoadError, LoadResult};

// ---- Shared data types ----

pub use perro_csv::{
    CSVQuery, CSVQueryResult, CSVQueryRow, Csv, CsvBuf, CsvCell, CsvCompare, CsvLogic, CsvOrder,
    CsvRow, CsvRowIndex,
};
#[doc(hidden)]
pub use perro_ids::string_to_u64 as __perro_string_to_u64;
pub use perro_render_bridge::{
    CustomMaterial3D, CustomMaterialImage3D, CustomMaterialLighting3D, CustomMaterialParam3D,
    CustomMaterialParamValue3D, Material3D, Mesh3D, MeshSurfaceRange, RuntimeMeshVertex,
};
pub use perro_scene::{Scene, SceneDoc, SceneWrite};
pub use res_path::{ResPath, ResPathBuf, ResPathError, ResPathKind, ResPathSource};

/// Common imports for scripts that use resource APIs.
pub mod prelude {
    // Facade traits and module accessors.
    pub use crate::api::{ResourceAPI, ResourceWindow, ViewportAPI};
    pub use crate::load_error::{LoadError, LoadResult};
    pub use crate::res_path::{ResPath, ResPathBuf, ResPathError, ResPathKind, ResPathSource};

    // Resource domain APIs.
    pub use crate::sub_apis::{
        AnimationAPI, AnimationModule, Audio, Audio2D, Audio2DModule, Audio3D, Audio3DModule,
        AudioAPI, AudioBusID, AudioClip, AudioDirection, AudioModule, AudioPan, AudioPlayConfig,
        CsvAPI, CsvModule, Draw2DAPI, Draw2DModule, GlbModule, GltfAPI, GltfInfo, IntoLocale,
        Locale, LocalizationAPI, LocalizationModule, MaterialAPI, MaterialModule,
        MaterialReserveArg, MeshAPI, MeshModule, MeshReserveArg, MicAPI, MicClip,
        MicDenoiseSettings, MicModule, MicSettings, MidiChannel, MidiModule, MidiNoteHandle,
        MidiNoteOptions, MidiProgram, MidiSong, MidiSound, MidiSpatialPos, MidiSpatialPosition,
        NavMesh3D, NavMeshLink3D, NavMeshResource3D, NavMeshTriangle3D, NavMeshValidationError,
        Note, PannedAudio, PostProcessingAPI, SceneDocAPI, SceneDocModule, SkeletonAPI,
        SkeletonModule, SpatialAudioOptions, TextureAPI, TextureModule, TextureReserveArg,
        VideoAPI, VideoModule, VideoUpdate, VisualAccessibilityAPI, WebcamAPI, WebcamConfig,
        WebcamDevice, WebcamFrame, WebcamModule, program,
    };

    // Convenience macros.
    pub use crate::{
        animation_count, animation_create_from_bytes, animation_drop, animation_is_loaded,
        animation_load, animation_reserve, animation_tree_create_from_bytes, animation_tree_drop,
        animation_tree_is_loaded, audio_bus, audio_bus_pause, audio_bus_resume,
        audio_bus_set_speed, audio_bus_set_volume, audio_bus_stop, audio_create_from_bytes,
        audio_drop, audio_is_loaded, audio_length_millis, audio_length_seconds, audio_load,
        audio_play, audio_play_clip, audio_reserve, audio_set_master_volume, audio_stop,
        audio_stop_all, audio_stop_source, csv_load, csv_load_bytes, csv_save,
        disable_colorblind_filter, draw, enable_colorblind_filter, get_viewport_size, glb_inspect,
        locale, locale_get_current, locale_in, locale_set, material_count, material_create,
        material_create_from_bytes, material_drop, material_get_data, material_is_loaded,
        material_load, material_reserve, material_write, mesh_count, mesh_create,
        mesh_create_from_bytes, mesh_drop, mesh_get_data, mesh_is_loaded, mesh_load, mesh_reserve,
        mesh_write, mic_clip, mic_frame, mic_frame_bytes, mic_get_bytes, mic_get_clip,
        mic_is_listening, mic_pack, mic_record, mic_save_wav, mic_start, mic_start_listening,
        mic_start_stream, mic_start_with, mic_stop, mic_stop_listening, mic_stop_stream,
        mic_stream_bytes, mic_stream_clip, mic_unpack, midi_load_soundfont,
        midi_load_soundfont_from_bytes, midi_play, midi_play_at, midi_release,
        midi_soundfont_is_loaded, midi_start, midi_start_at, navmesh_create,
        navmesh_create_from_bytes, navmesh_load, node_count, post_processing_add,
        post_processing_clear, post_processing_remove, post_processing_set, res_path, res_path_buf,
        scene_count, scene_load_doc, scene_save_doc, skeleton_count, skeleton_load_bones,
        skeleton_load_bones_2d_from_bytes, skeleton_load_bones_3d_from_bytes, texture_count,
        texture_create_from_bytes, texture_create_from_rgba, texture_drop, texture_is_loaded,
        texture_load, texture_reserve, texture_write_rgba, texture_write_rgba_region,
        video_release_node, video_update_node, webcam_default, webcam_devices, webcam_frame_rgba,
        webcam_open, webcam_open_device, webcam_texture,
    };

    // Shared data types.
    pub use perro_csv::{
        CSVQuery, CSVQueryResult, CSVQueryRow, Csv, CsvBuf, CsvCell, CsvCompare, CsvLogic,
        CsvOrder, CsvRow, CsvRowIndex,
    };
    pub use perro_ids::prelude::{
        AnimationID, AnimationTreeID, LightID, MaterialID, MeshID, NavMeshID, NodeID,
        ScriptMemberID, SignalID, TagID, TextureID, WebcamID,
    };
    pub use perro_render_bridge::{
        CustomMaterial3D, CustomMaterialImage3D, CustomMaterialLighting3D, CustomMaterialParam3D,
        CustomMaterialParamValue3D, Material3D, Mesh3D, MeshSurfaceRange, RuntimeMeshVertex,
    };
    pub use perro_scene::{Scene, SceneDoc, SceneWrite};
    pub use perro_structs::{Vector2, Vector3};
}
