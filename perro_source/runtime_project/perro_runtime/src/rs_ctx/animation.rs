use super::core::RuntimeResourceApi;
use perro_animation::AnimationClip;
use perro_ids::AnimationID;
use perro_resource_context::sub_apis::AnimationAPI;
use std::borrow::Cow;
use std::sync::Arc;

impl AnimationAPI for RuntimeResourceApi {
    fn load_animation_source(&self, source: &str) -> AnimationID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.load_animation_source_hashed(hash, None)
        } else {
            self.load_animation_source_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn reserve_animation_source(&self, source: &str) -> AnimationID {
        if let Some(hash) = perro_ids::parse_hashed_source_uri(source) {
            self.reserve_animation_source_hashed(hash, None)
        } else {
            self.reserve_animation_source_hashed(perro_ids::string_to_u64(source), Some(source))
        }
    }

    fn load_animation_source_hashed(&self, source_hash: u64, source: Option<&str>) -> AnimationID {
        if source.is_some_and(|v| v.trim().is_empty()) {
            return AnimationID::nil();
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        if let Some(id) = state.animation_by_source.get(&source_hash).copied() {
            return id;
        }

        let clip = self
            .load_animation_source_data(source_hash, source.map(str::trim))
            .unwrap_or_else(|| Arc::new(AnimationClip::default()));
        let id = state.allocate_animation_id();
        state.animation_by_source.insert(source_hash, id);
        state.animation_data_by_id.insert(id, clip);
        id
    }

    fn reserve_animation_source_hashed(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> AnimationID {
        self.load_animation_source_hashed(source_hash, source)
    }

    fn drop_animation_source(&self, id: AnimationID) -> bool {
        if id.is_nil() {
            return false;
        }

        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_by_source.retain(|_, existing| *existing != id);
        state.animation_data_by_id.remove(&id);
        let _ = state.free_animation_id(id);
        true
    }

    fn get_animation(&self, id: AnimationID) -> Option<Arc<AnimationClip>> {
        if id.is_nil() {
            return None;
        }

        let state = self.state.lock().expect("resource api mutex poisoned");
        state.animation_data_by_id.get(&id).cloned()
    }
}

impl RuntimeResourceApi {
    fn load_animation_source_data(
        &self,
        source_hash: u64,
        source: Option<&str>,
    ) -> Option<Arc<AnimationClip>> {
        if let Some(lookup) = self.static_animation_lookup
            && let Some(clip) = lookup(source_hash)
        {
            return Some(Arc::new(clip.clone()));
        }

        let Some(source) = source else {
            return None;
        };
        if source.ends_with(".panim")
            && let Ok(bytes) = perro_io::load_asset(source)
            && let Ok(text) = std::str::from_utf8(&bytes)
            && let Ok(clip) = perro_animation::parse_panim(text)
        {
            return Some(Arc::new(clip));
        }

        Some(Arc::new(AnimationClip {
            name: Cow::Borrowed("Animation"),
            fps: 60.0,
            total_frames: 1,
            objects: Cow::Borrowed(&[]),
            object_tracks: Cow::Borrowed(&[]),
            frame_events: Cow::Borrowed(&[]),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_project::StaticAnimationLookup;
    use perro_animation::{
        AnimationBoneSelector, AnimationBoneTarget, AnimationClip, AnimationEvent,
        AnimationEventScope, AnimationObject, AnimationObjectKey, AnimationObjectTrack,
        AnimationParam, AnimationTrackValue,
    };
    use std::path::PathBuf;
    use std::sync::{Arc, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_PANIM_SRC: &str = r#"
[Animation]
name = "StaticEquivalence"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (0,0,0)
}
[/Frame0]
"#;

    const TEST_PANIM_NAME: &str = "StaticEquivalence";

    fn unique_temp_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("perro_runtime_{label}_{ts}"))
    }

    fn write_test_panim_file() -> String {
        let dir = unique_temp_dir("animation");
        std::fs::create_dir_all(&dir).expect("failed to create temp dir");
        let path = dir.join("clip.panim");
        std::fs::write(&path, TEST_PANIM_SRC).expect("failed to write test panim");
        path.to_string_lossy().to_string()
    }

    fn new_api_with_lookup(lookup: Option<StaticAnimationLookup>) -> Arc<RuntimeResourceApi> {
        RuntimeResourceApi::new(None, None, None, lookup, None, None)
    }

    fn static_clip_lookup(path_hash: u64) -> Option<&'static AnimationClip> {
        if path_hash != string_to_u64("res://test/clip.panim") {
            return None;
        }
        static CLIP: OnceLock<AnimationClip> = OnceLock::new();
        Some(CLIP.get_or_init(|| {
            perro_animation::parse_panim(TEST_PANIM_SRC).expect("test panim must parse")
        }))
    }

    fn assert_f32_eq(actual: f32, expected: f32) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }

    fn assert_vec2_eq(actual: [f32; 2], expected: [f32; 2]) {
        assert_f32_eq(actual[0], expected[0]);
        assert_f32_eq(actual[1], expected[1]);
    }

    fn assert_vec3_eq(actual: [f32; 3], expected: [f32; 3]) {
        assert_f32_eq(actual[0], expected[0]);
        assert_f32_eq(actual[1], expected[1]);
        assert_f32_eq(actual[2], expected[2]);
    }

    fn assert_vec4_eq(actual: [f32; 4], expected: [f32; 4]) {
        assert_f32_eq(actual[0], expected[0]);
        assert_f32_eq(actual[1], expected[1]);
        assert_f32_eq(actual[2], expected[2]);
        assert_f32_eq(actual[3], expected[3]);
    }

    fn assert_bone_target_eq(
        actual: &Option<AnimationBoneTarget>,
        expected: &Option<AnimationBoneTarget>,
    ) {
        match (actual, expected) {
            (None, None) => {}
            (Some(a), Some(b)) => match (&a.selector, &b.selector) {
                (AnimationBoneSelector::Index(ai), AnimationBoneSelector::Index(bi)) => {
                    assert_eq!(ai, bi);
                }
                (AnimationBoneSelector::Name(an), AnimationBoneSelector::Name(bn)) => {
                    assert_eq!(an, bn);
                }
                _ => panic!("bone selector mismatch"),
            },
            _ => panic!("bone target mismatch"),
        }
    }

    fn assert_track_value_eq(actual: &AnimationTrackValue, expected: &AnimationTrackValue) {
        match (actual, expected) {
            (AnimationTrackValue::Bool(a), AnimationTrackValue::Bool(b)) => assert_eq!(a, b),
            (AnimationTrackValue::I32(a), AnimationTrackValue::I32(b)) => assert_eq!(a, b),
            (AnimationTrackValue::U32(a), AnimationTrackValue::U32(b)) => assert_eq!(a, b),
            (AnimationTrackValue::F32(a), AnimationTrackValue::F32(b)) => assert_f32_eq(*a, *b),
            (AnimationTrackValue::Vec2(a), AnimationTrackValue::Vec2(b)) => assert_vec2_eq(*a, *b),
            (AnimationTrackValue::Vec3(a), AnimationTrackValue::Vec3(b)) => assert_vec3_eq(*a, *b),
            (AnimationTrackValue::Vec4(a), AnimationTrackValue::Vec4(b)) => assert_vec4_eq(*a, *b),
            (AnimationTrackValue::AssetPath(a), AnimationTrackValue::AssetPath(b)) => {
                assert_eq!(a, b)
            }
            (AnimationTrackValue::Transform2D(a), AnimationTrackValue::Transform2D(b)) => {
                assert_eq!(a, b)
            }
            (AnimationTrackValue::Transform3D(a), AnimationTrackValue::Transform3D(b)) => {
                assert_eq!(a, b)
            }
            _ => panic!("track value variant mismatch"),
        }
    }

    fn assert_param_eq(actual: &AnimationParam, expected: &AnimationParam) {
        match (actual, expected) {
            (AnimationParam::Bool(a), AnimationParam::Bool(b)) => assert_eq!(a, b),
            (AnimationParam::I32(a), AnimationParam::I32(b)) => assert_eq!(a, b),
            (AnimationParam::U32(a), AnimationParam::U32(b)) => assert_eq!(a, b),
            (AnimationParam::F32(a), AnimationParam::F32(b)) => assert_f32_eq(*a, *b),
            (AnimationParam::Vec2(a), AnimationParam::Vec2(b)) => assert_vec2_eq(*a, *b),
            (AnimationParam::Vec3(a), AnimationParam::Vec3(b)) => assert_vec3_eq(*a, *b),
            (AnimationParam::Vec4(a), AnimationParam::Vec4(b)) => assert_vec4_eq(*a, *b),
            (AnimationParam::String(a), AnimationParam::String(b)) => assert_eq!(a, b),
            (AnimationParam::Transform2D(a), AnimationParam::Transform2D(b)) => assert_eq!(a, b),
            (AnimationParam::Transform3D(a), AnimationParam::Transform3D(b)) => assert_eq!(a, b),
            (AnimationParam::ObjectNode(a), AnimationParam::ObjectNode(b)) => assert_eq!(a, b),
            (
                AnimationParam::ObjectField {
                    object: ao,
                    field: af,
                },
                AnimationParam::ObjectField {
                    object: bo,
                    field: bf,
                },
            ) => {
                assert_eq!(ao, bo);
                assert_eq!(af, bf);
            }
            _ => panic!("event param variant mismatch"),
        }
    }

    fn assert_event_scope_eq(actual: &AnimationEventScope, expected: &AnimationEventScope) {
        match (actual, expected) {
            (AnimationEventScope::Global, AnimationEventScope::Global) => {}
            (AnimationEventScope::Object(a), AnimationEventScope::Object(b)) => assert_eq!(a, b),
            _ => panic!("event scope mismatch"),
        }
    }

    fn assert_event_eq(actual: &AnimationEvent, expected: &AnimationEvent) {
        match (actual, expected) {
            (
                AnimationEvent::EmitSignal {
                    name: an,
                    params: ap,
                },
                AnimationEvent::EmitSignal {
                    name: bn,
                    params: bp,
                },
            ) => {
                assert_eq!(an, bn);
                assert_eq!(ap.len(), bp.len());
                for (a, b) in ap.iter().zip(bp.iter()) {
                    assert_param_eq(a, b);
                }
            }
            (
                AnimationEvent::SetVar {
                    name: an,
                    value: av,
                },
                AnimationEvent::SetVar {
                    name: bn,
                    value: bv,
                },
            ) => {
                assert_eq!(an, bn);
                assert_param_eq(av, bv);
            }
            (
                AnimationEvent::CallMethod {
                    name: an,
                    params: ap,
                },
                AnimationEvent::CallMethod {
                    name: bn,
                    params: bp,
                },
            ) => {
                assert_eq!(an, bn);
                assert_eq!(ap.len(), bp.len());
                for (a, b) in ap.iter().zip(bp.iter()) {
                    assert_param_eq(a, b);
                }
            }
            _ => panic!("event variant mismatch"),
        }
    }

    fn assert_object_eq(actual: &AnimationObject, expected: &AnimationObject) {
        assert_eq!(actual.name, expected.name);
        assert_eq!(actual.node_type, expected.node_type);
    }

    fn assert_object_key_eq(actual: &AnimationObjectKey, expected: &AnimationObjectKey) {
        assert_eq!(actual.frame, expected.frame);
        assert_eq!(actual.interpolation, expected.interpolation);
        assert_eq!(actual.ease, expected.ease);
        assert_track_value_eq(&actual.value, &expected.value);
    }

    fn assert_object_track_eq(actual: &AnimationObjectTrack, expected: &AnimationObjectTrack) {
        assert_eq!(actual.object, expected.object);
        assert_eq!(actual.field, expected.field);
        assert_bone_target_eq(&actual.bone_target, &expected.bone_target);
        assert_eq!(actual.interpolation, expected.interpolation);
        assert_eq!(actual.ease, expected.ease);
        assert_eq!(actual.keys.len(), expected.keys.len());
        for (a, b) in actual.keys.iter().zip(expected.keys.iter()) {
            assert_object_key_eq(a, b);
        }
    }

    fn assert_clip_deep_eq(actual: &AnimationClip, expected: &AnimationClip) {
        assert_eq!(actual.name, expected.name);
        assert_f32_eq(actual.fps, expected.fps);
        assert_eq!(actual.total_frames, expected.total_frames);

        assert_eq!(actual.objects.len(), expected.objects.len());
        for (a, b) in actual.objects.iter().zip(expected.objects.iter()) {
            assert_object_eq(a, b);
        }

        assert_eq!(actual.object_tracks.len(), expected.object_tracks.len());
        for (a, b) in actual
            .object_tracks
            .iter()
            .zip(expected.object_tracks.iter())
        {
            assert_object_track_eq(a, b);
        }

        assert_eq!(actual.frame_events.len(), expected.frame_events.len());
        for (a, b) in actual.frame_events.iter().zip(expected.frame_events.iter()) {
            assert_eq!(a.frame, b.frame);
            assert_event_scope_eq(&a.scope, &b.scope);
            assert_event_eq(&a.event, &b.event);
        }
    }

    #[test]
    fn animation_loader_static_lookup_matches_file_parse() {
        let source = write_test_panim_file();

        let disk_api = new_api_with_lookup(None);
        let disk_id = disk_api.load_animation_source(&source);
        let disk_clip = disk_api
            .get_animation(disk_id)
            .expect("disk-loaded clip should exist");

        let static_api = new_api_with_lookup(Some(static_clip_lookup));
        let static_id = static_api.load_animation_source(&source);
        let static_clip = static_api
            .get_animation(static_id)
            .expect("statically-loaded clip should exist");

        assert_eq!(disk_clip.name.as_ref(), TEST_PANIM_NAME);
        assert_clip_deep_eq(&disk_clip, &static_clip);
    }

    #[test]
    fn animation_loader_falls_back_to_file_when_static_lookup_misses() {
        fn empty_lookup(_path_hash: u64) -> Option<&'static AnimationClip> {
            None
        }

        let source = write_test_panim_file();
        let api = new_api_with_lookup(Some(empty_lookup));
        let id = api.load_animation_source(&source);
        let clip = api.get_animation(id).expect("fallback clip should exist");

        assert_eq!(clip.name.as_ref(), TEST_PANIM_NAME);
    }
}
