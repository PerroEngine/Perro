pub trait AudioAPI {
    fn play_audio(&self, source: &str, looped: bool) -> bool;
    fn stop_audio(&self, source: &str) -> bool;
    fn stop_all_audio(&self);
}

pub struct AudioModule<'res, R: AudioAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: AudioAPI + ?Sized> AudioModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn play<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.play_audio(source.as_ref(), false)
    }

    #[inline]
    pub fn play_looped<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.play_audio(source.as_ref(), true)
    }

    #[inline]
    pub fn stop<S: AsRef<str>>(&self, source: S) -> bool {
        self.api.stop_audio(source.as_ref())
    }

    #[inline]
    pub fn stop_all(&self) {
        self.api.stop_all_audio();
    }
}

#[macro_export]
macro_rules! play_audio {
    ($res:expr, $source:expr) => {
        $res.Audio().play($source)
    };
}

#[macro_export]
macro_rules! loop_audio {
    ($res:expr, $source:expr) => {
        $res.Audio().play_looped($source)
    };
}

#[macro_export]
macro_rules! stop_audio {
    ($res:expr, $source:expr) => {
        $res.Audio().stop($source)
    };
}

#[macro_export]
macro_rules! stop_all_audio {
    ($res:expr) => {
        $res.Audio().stop_all()
    };
}
