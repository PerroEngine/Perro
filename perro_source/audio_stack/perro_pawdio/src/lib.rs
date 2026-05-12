mod codec;
mod controller;
mod internal;
mod math;
pub mod midi;
mod player;
mod types;

pub use controller::{AudioController, AudioSourceHandle};
pub use midi::{
    MidiChannel, MidiNoteHandle, MidiNoteOptions, MidiProgram, MidiSong, MidiSound, Note, program,
};
pub use perro_ids::SoundFontID;
pub use player::BarkPlayer;
pub use types::{
    Audio2D, Audio3D, AudioCompression, AudioEq, AudioListener2D, AudioListener3D, AudioPan,
    AudioPlaybackRequest, SpatialAudioParams,
};

#[cfg(test)]
mod tests {
    use crate::codec::decode_static_pawdio;
    use crate::{Audio2D, Audio3D, AudioListener2D, AudioListener3D};

    #[test]
    fn decode_static_pawdio_accepts_v1_raw_payload() {
        let raw = [1u8, 2, 3, 4];
        let mut blob = Vec::new();
        blob.extend_from_slice(b"PAWDIO");
        blob.extend_from_slice(&1u32.to_le_bytes());
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        blob.extend_from_slice(&raw);

        let (decoded, _) = decode_static_pawdio(&blob).expect("decode pawdio v1");
        assert_eq!(decoded, raw);
    }

    #[test]
    fn decode_static_pawdio_rejects_non_v1() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"PAWDIO");
        blob.extend_from_slice(&2u32.to_le_bytes());
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.extend_from_slice(&0u32.to_le_bytes());

        let err = decode_static_pawdio(&blob).expect_err("non-v1 version must fail");
        assert!(err.contains("unsupported .pawdio version"));
    }

    #[test]
    fn audio_2d_maps_world_pos_to_listener_pan_and_volume() {
        let req = Audio2D::new("res://hit.wav", [5.0, 0.0], 10.0)
            .to_playback(AudioListener2D::default())
            .expect("in range");
        assert!((req.pan.x - 0.5).abs() < 1.0e-6);
        assert!((req.pan.y - 0.0).abs() < 1.0e-6);
        assert!((req.volume - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn audio_2d_culls_out_of_range() {
        let req = Audio2D::new("res://hit.wav", [11.0, 0.0], 10.0)
            .to_playback(AudioListener2D::default());
        assert!(req.is_none());
    }

    #[test]
    fn audio_3d_maps_world_pos_to_listener_pan_and_volume() {
        let req = Audio3D::new("res://hit.wav", [0.0, 0.0, -5.0], 10.0)
            .to_playback(AudioListener3D::default())
            .expect("in range");
        assert!((req.pan.x - 0.0).abs() < 1.0e-6);
        assert!((req.pan.y - 0.0).abs() < 1.0e-6);
        assert!((req.pan.z - 0.5).abs() < 1.0e-6);
        assert!((req.volume - 0.5).abs() < 1.0e-6);
    }
}
