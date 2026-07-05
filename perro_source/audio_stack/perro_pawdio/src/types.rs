use perro_ids::AudioBusID;

use crate::math::{distance_attenuation, inverse_rotate_vec3, spatial_pan};

#[derive(Clone, Copy, Debug)]
pub struct AudioEq {
    pub low_gain: f32,
    pub mid_gain: f32,
    pub high_gain: f32,
}

impl Default for AudioEq {
    fn default() -> Self {
        Self {
            low_gain: 1.0,
            mid_gain: 1.0,
            high_gain: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AudioCompression {
    pub threshold: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
}

impl Default for AudioCompression {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            ratio: 1.0,
            attack: 0.01,
            release: 0.1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SpatialAudioParams {
    pub pan: AudioPan,
    pub volume: f32,
    pub low_pass: f32,
    pub reverb_send: f32,
    pub echo: f32,
    pub reflection: f32,
    pub occlusion: f32,
    pub eq: AudioEq,
    pub compression: AudioCompression,
}

#[derive(Clone, Copy, Debug)]
pub struct AudioPan {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AudioPan {
    pub const CENTER: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn clamped(self) -> Self {
        Self {
            x: self.x.clamp(-1.0, 1.0),
            y: self.y.clamp(-1.0, 1.0),
            z: self.z.clamp(-1.0, 1.0),
        }
    }
}

impl Default for AudioPan {
    fn default() -> Self {
        Self::CENTER
    }
}

#[derive(Clone, Copy)]
pub struct AudioListener2D {
    pub position: [f32; 2],
    pub rotation_radians: f32,
}

impl Default for AudioListener2D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            rotation_radians: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct AudioListener3D {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
}

impl Default for AudioListener3D {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[derive(Clone, Copy)]
pub struct Audio2D<'a> {
    pub source: &'a str,
    pub bus_id: Option<AudioBusID>,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub position: [f32; 2],
    pub range: f32,
    pub from_start: f32,
    pub from_end: f32,
}

impl<'a> Audio2D<'a> {
    pub const fn new(source: &'a str, position: [f32; 2], range: f32) -> Self {
        Self {
            source,
            bus_id: None,
            looped: false,
            volume: 1.0,
            speed: 1.0,
            position,
            range,
            from_start: 0.0,
            from_end: 0.0,
        }
    }

    pub fn to_playback(self, listener: AudioListener2D) -> Option<AudioPlaybackRequest<'a>> {
        let range = self.range.max(0.0001);
        let dx = self.position[0] - listener.position[0];
        let dy = self.position[1] - listener.position[1];
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > range {
            return None;
        }
        let (sin, cos) = (-listener.rotation_radians).sin_cos();
        let local_x = dx * cos - dy * sin;
        let local_y = dx * sin + dy * cos;
        let attenuation = distance_attenuation(distance, range);
        let pan = spatial_pan([local_x, local_y, 0.0]);
        Some(AudioPlaybackRequest {
            id: 0,
            source: self.source,
            bus_id: self.bus_id,
            looped: self.looped,
            volume: self.volume * attenuation,
            speed: self.speed,
            pan: AudioPan::new(pan[0], pan[1], pan[2]),
            low_pass: 0.0,
            reverb_send: 0.0,
            echo: 0.0,
            reflection: 0.0,
            occlusion: 0.0,
            eq: AudioEq::default(),
            compression: AudioCompression::default(),
            from_start: self.from_start,
            from_end: self.from_end,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Audio3D<'a> {
    pub source: &'a str,
    pub bus_id: Option<AudioBusID>,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub position: [f32; 3],
    pub range: f32,
    pub from_start: f32,
    pub from_end: f32,
}

impl<'a> Audio3D<'a> {
    pub const fn new(source: &'a str, position: [f32; 3], range: f32) -> Self {
        Self {
            source,
            bus_id: None,
            looped: false,
            volume: 1.0,
            speed: 1.0,
            position,
            range,
            from_start: 0.0,
            from_end: 0.0,
        }
    }

    pub fn to_playback(self, listener: AudioListener3D) -> Option<AudioPlaybackRequest<'a>> {
        let range = self.range.max(0.0001);
        let dx = self.position[0] - listener.position[0];
        let dy = self.position[1] - listener.position[1];
        let dz = self.position[2] - listener.position[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance > range {
            return None;
        }
        let local = inverse_rotate_vec3(listener.rotation, [dx, dy, dz]);
        let attenuation = distance_attenuation(distance, range);
        let pan = spatial_pan([local[0], local[1], -local[2]]);
        Some(AudioPlaybackRequest {
            id: 0,
            source: self.source,
            bus_id: self.bus_id,
            looped: self.looped,
            volume: self.volume * attenuation,
            speed: self.speed,
            pan: AudioPan::new(pan[0], pan[1], pan[2]),
            low_pass: 0.0,
            reverb_send: 0.0,
            echo: 0.0,
            reflection: 0.0,
            occlusion: 0.0,
            eq: AudioEq::default(),
            compression: AudioCompression::default(),
            from_start: self.from_start,
            from_end: self.from_end,
        })
    }
}

#[derive(Clone, Copy)]
pub struct AudioPlaybackRequest<'a> {
    pub id: u64,
    pub source: &'a str,
    pub bus_id: Option<AudioBusID>,
    pub looped: bool,
    pub volume: f32,
    pub speed: f32,
    pub pan: AudioPan,
    pub low_pass: f32,
    pub reverb_send: f32,
    pub echo: f32,
    pub reflection: f32,
    pub occlusion: f32,
    pub eq: AudioEq,
    pub compression: AudioCompression,
    pub from_start: f32,
    pub from_end: f32,
}
