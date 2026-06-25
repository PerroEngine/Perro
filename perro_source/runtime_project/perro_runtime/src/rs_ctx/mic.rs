use super::core::RuntimeResourceApi;
use perro_io::{ProjectRoot, get_project_root, save_asset};
use perro_resource_api::sub_apis::{MicAPI, MicClip, MicSettings};

impl MicAPI for RuntimeResourceApi {
    fn mic_start(&self, settings: MicSettings) -> Result<(), String> {
        self.mic
            .lock()
            .map_err(|_| "mic mutex poisoned".to_string())?
            .start(settings)
    }

    fn mic_stop(&self) -> Option<MicClip> {
        self.mic.lock().ok()?.stop()
    }

    fn mic_clip(&self) -> Option<MicClip> {
        self.mic.lock().ok()?.clip()
    }

    fn mic_stream_clip(&self) -> Option<MicClip> {
        self.mic.lock().ok()?.stream_clip()
    }

    fn mic_stream_bytes(&self) -> Option<Vec<u8>> {
        self.mic.lock().ok()?.stream_bytes()
    }

    fn mic_is_listening(&self) -> bool {
        self.mic
            .lock()
            .map(|mic| mic.is_listening())
            .unwrap_or(false)
    }

    fn mic_play(&self, bus_id: Option<perro_ids::AudioBusID>, clip: &MicClip, volume: f32) -> bool {
        let Ok(guard) = self.bark.lock() else {
            return false;
        };
        let Some(player) = guard.as_ref() else {
            return false;
        };
        player.play_clip(
            "mic://clip",
            clip.clone(),
            bus_id,
            volume,
            perro_pawdio::AudioPan::CENTER,
        )
    }

    fn mic_save_wav(&self, source: &str, clip: &MicClip) -> Result<(), String> {
        let bytes = clip.wav_bytes();
        if let Some(stripped) = source.strip_prefix("res://")
            && let ProjectRoot::Disk { root, .. } = get_project_root()
        {
            let path = root.join("res").join(stripped);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create mic wav dir `{source}`: {err}"))?;
            }
            std::fs::write(&path, &bytes)
                .map_err(|err| format!("failed to save mic wav `{source}`: {err}"))?;
        } else {
            save_asset(source, &bytes)
                .map_err(|err| format!("failed to save mic wav `{source}`: {err}"))?;
        }
        Ok(())
    }
}
