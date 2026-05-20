pub mod api;
pub mod res_path;
pub mod sub_apis;

pub use api::ResourceWindow;
pub use perro_csv::{
    CSVQuery, CSVQueryResult, CSVQueryRow, Csv, CsvBuf, CsvCell, CsvCompare, CsvLogic, CsvOrder,
    CsvRow, CsvRowIndex,
};
#[doc(hidden)]
pub use perro_ids::string_to_u64 as __perro_string_to_u64;
pub use perro_render_bridge::{Material3D, Mesh3D, MeshSurfaceRange, RuntimeMeshVertex};
pub use perro_scene::{Scene, SceneDoc, SceneWrite};
pub use res_path::{ResPath, ResPathBuf, ResPathError, ResPathKind, ResPathSource};

pub mod prelude {
    pub use crate::api::{ResourceAPI, ResourceWindow, ViewportAPI};
    pub use crate::res_path::{ResPath, ResPathBuf, ResPathError, ResPathKind, ResPathSource};
    pub use crate::sub_apis::{
        AnimationAPI, AnimationModule, Audio, Audio2D, Audio2DModule, Audio3D, Audio3DModule,
        AudioAPI, AudioBusID, AudioDirection, AudioModule, AudioPan, AudioPlayConfig, CsvAPI,
        CsvModule, Draw2DAPI, Draw2DModule, Locale, LocalizationAPI, LocalizationModule,
        MaterialAPI, MaterialModule, MeshAPI, MeshModule, MidiChannel, MidiModule, MidiNoteHandle,
        MidiNoteOptions, MidiProgram, MidiSong, MidiSound, MidiSpatialPos, MidiSpatialPosition,
        Note, PannedAudio, PostProcessingAPI, SceneDocAPI, SceneDocModule, SkeletonAPI,
        SkeletonModule, SpatialAudioOptions, TextureAPI, TextureModule, VisualAccessibilityAPI,
        program,
    };
    pub use crate::{
        animation_drop, animation_is_loaded, animation_load, animation_reserve,
        animation_tree_drop, animation_tree_is_loaded, audio_bus, audio_bus_pause,
        audio_bus_resume, audio_bus_set_speed, audio_bus_set_volume, audio_bus_stop, audio_drop,
        audio_is_loaded, audio_length_millis, audio_length_seconds, audio_load, audio_play,
        audio_reserve, audio_set_master_volume, audio_stop, audio_stop_all, audio_stop_source,
        csv_load, csv_save, disable_colorblind_filter, draw, enable_colorblind_filter,
        get_viewport_size, locale, locale_get_current, locale_in, locale_set, material_create,
        material_drop, material_get_data, material_is_loaded, material_load, material_reserve,
        material_write, mesh_create, mesh_drop, mesh_get_data, mesh_is_loaded, mesh_load,
        mesh_reserve, mesh_write, midi_load_soundfont, midi_play, midi_play_at, midi_release,
        midi_soundfont_is_loaded, midi_start, midi_start_at, post_processing_add,
        post_processing_clear, post_processing_remove, post_processing_set, res_path, res_path_buf,
        scene_load_doc, scene_save_doc, skeleton_load_bones, texture_drop, texture_is_loaded,
        texture_load, texture_reserve,
    };
    pub use perro_csv::{
        CSVQuery, CSVQueryResult, CSVQueryRow, Csv, CsvBuf, CsvCell, CsvCompare, CsvLogic,
        CsvOrder, CsvRow, CsvRowIndex,
    };
    pub use perro_ids::prelude::{
        AnimationID, AnimationTreeID, LightID, MaterialID, MeshID, NodeID, ScriptMemberID,
        SignalID, TagID, TextureID, UIElementID,
    };
    pub use perro_render_bridge::{Material3D, Mesh3D, MeshSurfaceRange, RuntimeMeshVertex};
    pub use perro_scene::{Scene, SceneDoc, SceneWrite};
    pub use perro_structs::{Vector2, Vector3};
}
