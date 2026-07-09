use perro_ids::{NodeID, TextureID};
pub use perro_nodes::VideoPlayer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoUpdate {
    pub texture: TextureID,
    pub frame_changed: bool,
}

pub trait VideoAPI {
    fn video_update_node(
        &self,
        node: NodeID,
        player: &VideoPlayer,
        delta_seconds: f32,
    ) -> VideoUpdate;
    fn video_release_node(&self, node: NodeID) -> bool;
}

pub struct VideoModule<'res, R: VideoAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: VideoAPI + ?Sized> VideoModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    #[inline]
    pub fn update_node(
        &self,
        node: NodeID,
        player: &VideoPlayer,
        delta_seconds: f32,
    ) -> VideoUpdate {
        self.api.video_update_node(node, player, delta_seconds)
    }

    #[inline]
    pub fn release_node(&self, node: NodeID) -> bool {
        self.api.video_release_node(node)
    }
}

#[macro_export]
macro_rules! video_update_node {
    ($res:expr, $node:expr, $player:expr, $delta:expr) => {
        $res.Videos().update_node($node, $player, $delta)
    };
}

#[macro_export]
macro_rules! video_release_node {
    ($res:expr, $node:expr) => {
        $res.Videos().release_node($node)
    };
}
