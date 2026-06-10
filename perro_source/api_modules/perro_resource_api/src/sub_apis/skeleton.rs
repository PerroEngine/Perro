//! Skeleton resource API.
//!
//! Loads skeleton bone data from resource paths.

use crate::ResPathSource;
use perro_nodes::{skeleton_2d::Bone2D, skeleton_3d::Bone3D};

pub trait SkeletonAPI {
    fn load_bones_2d(&self, source: &str) -> Vec<Bone2D>;
    fn load_bones_3d(&self, source: &str) -> Vec<Bone3D>;

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
    pub fn load_bones_2d<S: ResPathSource>(&self, source: S) -> Vec<Bone2D> {
        self.api.load_bones_2d(source.as_res_path_str())
    }

    #[inline]
    pub fn load_bones_3d<S: ResPathSource>(&self, source: S) -> Vec<Bone3D> {
        self.api.load_bones_3d(source.as_res_path_str())
    }

    #[inline]
    pub fn load_bones<S: ResPathSource>(&self, source: S) -> Vec<Bone3D> {
        self.api.load_bones(source.as_res_path_str())
    }
}

#[macro_export]
macro_rules! skeleton_load_bones {
    ($res:expr, $source:expr) => {
        $res.Skeletons().load_bones($source)
    };
}
