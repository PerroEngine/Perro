use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

pub struct BarkPlayer {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: Mutex<HashMap<String, Sink>>,
}

enum AudioCommand {
    Play { source: String, looped: bool },
    Stop { source: String },
    StopAll,
}

#[derive(Clone)]
pub struct AudioController {
    tx: Sender<AudioCommand>,
}

impl BarkPlayer {
    pub fn new() -> Result<Self, String> {
        let (stream, handle) =
            OutputStream::try_default().map_err(|err| format!("audio output init failed: {err}"))?;
        Ok(Self {
            _stream: stream,
            handle,
            sinks: Mutex::new(HashMap::new()),
        })
    }

    pub fn play_source(&self, source: &str, looped: bool) -> Result<(), String> {
        let bytes = perro_io::load_asset(source)
            .map_err(|err| format!("failed to load audio asset `{source}`: {err}"))?;
        self.play_bytes(source, bytes, looped)
    }

    pub fn play_bytes(&self, source: &str, bytes: Vec<u8>, looped: bool) -> Result<(), String> {
        let cursor = Cursor::new(bytes);
        let reader = BufReader::new(cursor);
        let decoder = Decoder::new(reader)
            .map_err(|err| format!("failed to decode audio `{source}`: {err}"))?;

        let sink =
            Sink::try_new(&self.handle).map_err(|err| format!("failed to create sink: {err}"))?;

        if looped {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }

        sink.play();

        let mut sinks = self.sinks.lock().map_err(|_| "audio mutex poisoned".to_string())?;
        if let Some(previous) = sinks.insert(source.to_string(), sink) {
            previous.stop();
        }
        Ok(())
    }

    pub fn stop_source(&self, source: &str) -> bool {
        let Ok(mut sinks) = self.sinks.lock() else {
            return false;
        };
        if let Some(sink) = sinks.remove(source) {
            sink.stop();
            true
        } else {
            false
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut sinks) = self.sinks.lock() {
            for (_, sink) in sinks.drain() {
                sink.stop();
            }
        }
    }
}

impl AudioController {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<AudioCommand>();
        std::thread::Builder::new()
            .name("perro_bark_audio".to_string())
            .spawn(move || {
                let Ok(player) = BarkPlayer::new() else {
                    return;
                };

                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::Play { source, looped } => {
                            let _ = player.play_source(&source, looped);
                        }
                        AudioCommand::Stop { source } => {
                            let _ = player.stop_source(&source);
                        }
                        AudioCommand::StopAll => player.stop_all(),
                    }
                }
            })
            .map_err(|err| format!("failed to spawn audio thread: {err}"))?;
        Ok(Self { tx })
    }

    pub fn play_source(&self, source: &str, looped: bool) -> bool {
        self.tx
            .send(AudioCommand::Play {
                source: source.to_string(),
                looped,
            })
            .is_ok()
    }

    pub fn stop_source(&self, source: &str) -> bool {
        self.tx
            .send(AudioCommand::Stop {
                source: source.to_string(),
            })
            .is_ok()
    }

    pub fn stop_all(&self) -> bool {
        self.tx.send(AudioCommand::StopAll).is_ok()
    }
}
