pub trait TimeAPI {
    fn get_delta(&self) -> f32;
}

pub struct TimeModule<'rt, R: TimeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: TimeAPI + ?Sized> TimeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn get_delta(&self) -> f32 {
        self.rt.get_delta()
    }
}

impl<'rt, R: TimeAPI + ?Sized> TimeModule<'rt, R> {
    pub fn get_unix_time(&self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }
}
