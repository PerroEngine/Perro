use crate::ResPathSource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GltfInfo {
    pub mesh_count: usize,
    pub material_count: usize,
    pub skeleton_count: usize,
    pub animation_count: usize,
    pub node_count: usize,
    pub scene_count: usize,
    pub texture_count: usize,
}

pub trait GltfAPI {
    fn inspect_gltf(&self, source: &str) -> Option<GltfInfo>;
}

pub struct GlbModule<'res, R: GltfAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: GltfAPI + ?Sized> GlbModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn inspect<S: ResPathSource>(&self, source: S) -> Option<GltfInfo> {
        self.api.inspect_gltf(source.as_res_path_str())
    }

    #[inline]
    pub fn mesh_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.mesh_count)
    }

    #[inline]
    pub fn material_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.material_count)
    }

    #[inline]
    pub fn skeleton_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.skeleton_count)
    }

    #[inline]
    pub fn animation_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.animation_count)
    }

    #[inline]
    pub fn node_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.node_count)
    }

    #[inline]
    pub fn scene_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.scene_count)
    }

    #[inline]
    pub fn texture_count<S: ResPathSource>(&self, source: S) -> Option<usize> {
        self.inspect(source).map(|info| info.texture_count)
    }
}

#[macro_export]
macro_rules! glb_inspect {
    ($res:expr, $source:expr) => {
        $res.Glbs().inspect($source)
    };
}

#[macro_export]
macro_rules! mesh_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().mesh_count($source)
    };
}

#[macro_export]
macro_rules! material_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().material_count($source)
    };
}

#[macro_export]
macro_rules! skeleton_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().skeleton_count($source)
    };
}

#[macro_export]
macro_rules! animation_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().animation_count($source)
    };
}

#[macro_export]
macro_rules! node_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().node_count($source)
    };
}

#[macro_export]
macro_rules! scene_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().scene_count($source)
    };
}

#[macro_export]
macro_rules! texture_count {
    ($res:expr, $source:expr) => {
        $res.Glbs().texture_count($source)
    };
}
