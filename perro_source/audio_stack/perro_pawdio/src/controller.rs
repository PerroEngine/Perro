use perro_ids::AudioBusID;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

use crate::internal::AudioCommand;
use crate::player::BarkPlayer;
use crate::types::{AudioPlaybackRequest, SpatialAudioParams};

pub struct AudioController {
    tx: Sender<AudioCommand>,
    next_playback_id: Arc<AtomicU64>,
}

impl AudioController {
    pub fn new(static_audio_lookup: Option<fn(u64) -> &'static [u8]>) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        let next_playback_id = Arc::new(AtomicU64::new(1));
        std::thread::Builder::new()
            .name("perro_pawdio_audio".to_string())
            .spawn(move || {
                let Ok(player) = BarkPlayer::new(static_audio_lookup) else {
                    return;
                };

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::Load { source, reserved } => {
                            let _ = player.load_source(&source, reserved);
                        }
                        AudioCommand::DropAsset { source } => {
                            let _ = player.drop_source_asset(&source);
                        }
                        AudioCommand::Play { request } => {
                            let _ = player.play_source(AudioPlaybackRequest {
                                id: request.id,
                                source: request.source.as_str(),
                                bus_id: request.bus_id,
                                looped: request.looped,
                                volume: request.volume,
                                speed: request.speed,
                                pan: request.pan,
                                low_pass: request.low_pass,
                                reverb_send: request.reverb_send,
                                echo: request.echo,
                                reflection: request.reflection,
                                occlusion: request.occlusion,
                                eq: request.eq,
                                compression: request.compression,
                                from_start: request.from_start,
                                from_end: request.from_end,
                            });
                        }
                        AudioCommand::Stop { source } => {
                            let _ = player.stop_source(&source);
                        }
                        AudioCommand::StopMatch { request } => {
                            let _ = player.stop_match(AudioPlaybackRequest {
                                id: request.id,
                                source: request.source.as_str(),
                                bus_id: request.bus_id,
                                looped: request.looped,
                                volume: request.volume,
                                speed: request.speed,
                                pan: request.pan,
                                low_pass: request.low_pass,
                                reverb_send: request.reverb_send,
                                echo: request.echo,
                                reflection: request.reflection,
                                occlusion: request.occlusion,
                                eq: request.eq,
                                compression: request.compression,
                                from_start: request.from_start,
                                from_end: request.from_end,
                            });
                        }
                        AudioCommand::StopPlayback { id } => {
                            let _ = player.stop_playback(id);
                        }
                        AudioCommand::UpdateSpatial { id, params } => {
                            let _ = player.update_spatial(id, params);
                        }
                        AudioCommand::StopAll => player.stop_all(),
                        AudioCommand::SetMasterVolume { volume } => {
                            player.set_master_volume(volume)
                        }
                        AudioCommand::SetBusVolume { bus_id, volume } => {
                            player.set_bus_volume(bus_id, volume)
                        }
                        AudioCommand::SetBusSpeed { bus_id, speed } => {
                            player.set_bus_speed(bus_id, speed)
                        }
                        AudioCommand::PauseBus { bus_id } => player.pause_bus(bus_id),
                        AudioCommand::ResumeBus { bus_id } => player.resume_bus(bus_id),
                        AudioCommand::StopBus { bus_id } => {
                            let _ = player.stop_bus(bus_id);
                        }
                        AudioCommand::SourceLength { source, reply } => {
                            let _ = reply.send(player.source_length_seconds(&source));
                        }
                    }
                }
            })
            .map_err(|err| format!("failed to spawn audio thread: {err}"))?;
        Ok(Self {
            tx,
            next_playback_id,
        })
    }

    pub fn play_source(&self, request: AudioPlaybackRequest<'_>) -> bool {
        self.tx
            .send(AudioCommand::Play {
                request: request.into(),
            })
            .is_ok()
    }

    pub fn play_spatial_source(&self, mut request: AudioPlaybackRequest<'_>) -> Option<u64> {
        let id = self.next_playback_id.fetch_add(1, Ordering::Relaxed).max(1);
        request.id = id;
        self.tx
            .send(AudioCommand::Play {
                request: request.into(),
            })
            .is_ok()
            .then_some(id)
    }

    pub fn update_spatial(&self, id: u64, params: SpatialAudioParams) -> bool {
        self.tx
            .send(AudioCommand::UpdateSpatial { id, params })
            .is_ok()
    }

    pub fn stop_playback(&self, id: u64) -> bool {
        self.tx.send(AudioCommand::StopPlayback { id }).is_ok()
    }

    pub fn load_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Load {
                source: source.to_string(),
                reserved: false,
            })
            .is_ok()
    }

    pub fn reserve_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Load {
                source: source.to_string(),
                reserved: true,
            })
            .is_ok()
    }

    pub fn drop_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::DropAsset {
                source: source.to_string(),
            })
            .is_ok()
    }

    pub fn source_length_seconds(&self, source: &str) -> Option<f32> {
        let (reply_tx, reply_rx) = mpsc::channel::<Option<f32>>();
        if self
            .tx
            .send(AudioCommand::SourceLength {
                source: source.to_string(),
                reply: reply_tx,
            })
            .is_err()
        {
            return None;
        }
        reply_rx.recv().ok().flatten()
    }

    pub fn stop_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Stop {
                source: source.to_string(),
            })
            .is_ok()
    }

    pub fn stop_match(&self, request: AudioPlaybackRequest<'_>) -> bool {
        self.tx
            .send(AudioCommand::StopMatch {
                request: request.into(),
            })
            .is_ok()
    }

    pub fn stop_all(&self) -> bool {
        self.tx.send(AudioCommand::StopAll).is_ok()
    }

    pub fn set_master_volume(&self, volume: f32) -> bool {
        self.tx
            .send(AudioCommand::SetMasterVolume { volume })
            .is_ok()
    }

    pub fn set_bus_volume(&self, bus_id: AudioBusID, volume: f32) -> bool {
        self.tx
            .send(AudioCommand::SetBusVolume { bus_id, volume })
            .is_ok()
    }

    pub fn set_bus_speed(&self, bus_id: AudioBusID, speed: f32) -> bool {
        self.tx
            .send(AudioCommand::SetBusSpeed { bus_id, speed })
            .is_ok()
    }

    pub fn pause_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.send(AudioCommand::PauseBus { bus_id }).is_ok()
    }

    pub fn resume_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.send(AudioCommand::ResumeBus { bus_id }).is_ok()
    }

    pub fn stop_bus(&self, bus_id: AudioBusID) -> bool {
        self.tx.send(AudioCommand::StopBus { bus_id }).is_ok()
    }
}
