use perro_nodes::skeleton_3d::Bone3D;

pub trait SkeletonAPI {
    fn load_bones(&self, source: &str) -> Vec<Bone3D>;
}

pub struct SkeletonModule<'res, R: SkeletonAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: SkeletonAPI + ?Sized> SkeletonModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn load_bones<S: AsRef<str>>(&self, source: S) -> Vec<Bone3D> {
        self.api.load_bones(source.as_ref())
    }
}

#[macro_export]
macro_rules! skeleton_load_bones {
    ($res:expr, $source:expr) => {
        $res.Skeletons().load_bones($source)
    };
}
