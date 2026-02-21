pub trait TimeAPI {
    fn get_delta(&self) -> f32;
    fn get_fixed_delta(&self) -> f32;
    fn get_elapsed(&self) -> f32;
}

pub struct TimeModule<'rt, R: TimeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: TimeAPI + ?Sized> TimeModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn get_delta(&mut self) -> f32 {
        self.rt.get_delta()
    }

    pub fn get_fixed_delta(&mut self) -> f32 {
        self.rt.get_fixed_delta()
    }

    pub fn get_elapsed(&mut self) -> f32 {
        self.rt.get_elapsed()
    }
}
