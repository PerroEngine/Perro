use perro_ids::{AnimationID, NodeID};

pub trait AnimPlayerAPI {
    fn animation_set_clip(&mut self, player: NodeID, animation: AnimationID) -> bool;
    fn animation_play(&mut self, player: NodeID) -> bool;
    fn animation_pause(&mut self, player: NodeID, paused: bool) -> bool;
    fn animation_seek_time(&mut self, player: NodeID, time_seconds: f32) -> bool;
    fn animation_seek_frame(&mut self, player: NodeID, frame: u32) -> bool;
    fn animation_set_speed(&mut self, player: NodeID, speed: f32) -> bool;
    fn animation_bind(&mut self, player: NodeID, track: &str, node: NodeID) -> bool;
    fn animation_clear_bindings(&mut self, player: NodeID) -> bool;
}

pub struct AnimPlayerModule<'rt, R: AnimPlayerAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: AnimPlayerAPI + ?Sized> AnimPlayerModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn set_clip(&mut self, player: NodeID, animation: AnimationID) -> bool {
        self.rt.animation_set_clip(player, animation)
    }

    #[inline]
    pub fn play(&mut self, player: NodeID) -> bool {
        self.rt.animation_play(player)
    }

    #[inline]
    pub fn pause(&mut self, player: NodeID, paused: bool) -> bool {
        self.rt.animation_pause(player, paused)
    }

    #[inline]
    pub fn seek_time(&mut self, player: NodeID, time_seconds: f32) -> bool {
        self.rt.animation_seek_time(player, time_seconds)
    }

    #[inline]
    pub fn seek_frame(&mut self, player: NodeID, frame: u32) -> bool {
        self.rt.animation_seek_frame(player, frame)
    }

    #[inline]
    pub fn set_speed(&mut self, player: NodeID, speed: f32) -> bool {
        self.rt.animation_set_speed(player, speed)
    }

    #[inline]
    pub fn bind<S: AsRef<str>>(&mut self, player: NodeID, track: S, node: NodeID) -> bool {
        self.rt.animation_bind(player, track.as_ref(), node)
    }

    #[inline]
    pub fn clear_bindings(&mut self, player: NodeID) -> bool {
        self.rt.animation_clear_bindings(player)
    }
}

#[macro_export]
macro_rules! anim_player_set_clip {
    ($ctx:expr, $player:expr, $animation:expr) => {
        $ctx.AnimPlayer().set_clip($player, $animation)
    };
}

#[macro_export]
macro_rules! anim_player_play {
    ($ctx:expr, $player:expr) => {
        $ctx.AnimPlayer().play($player)
    };
}

#[macro_export]
macro_rules! anim_player_pause {
    ($ctx:expr, $player:expr, $paused:expr) => {
        $ctx.AnimPlayer().pause($player, $paused)
    };
}

#[macro_export]
macro_rules! anim_player_seek_time {
    ($ctx:expr, $player:expr, $time:expr) => {
        $ctx.AnimPlayer().seek_time($player, $time)
    };
}

#[macro_export]
macro_rules! anim_player_seek_frame {
    ($ctx:expr, $player:expr, $frame:expr) => {
        $ctx.AnimPlayer().seek_frame($player, $frame)
    };
}

#[macro_export]
macro_rules! anim_player_set_speed {
    ($ctx:expr, $player:expr, $speed:expr) => {
        $ctx.AnimPlayer().set_speed($player, $speed)
    };
}

#[macro_export]
macro_rules! anim_player_bind {
    ($ctx:expr, $player:expr, [ $($track:tt : $node:expr),* $(,)? ]) => {{
        let __player = $player;
        let mut __all_ok = true;
        $(
            __all_ok = $ctx.AnimPlayer().bind(__player, $track, $node) && __all_ok;
        )*
        __all_ok
    }};
    ($ctx:expr, $player:expr, { $($track:expr => $node:expr),* $(,)? }) => {{
        let __player = $player;
        let mut __all_ok = true;
        $(
            __all_ok = $ctx.AnimPlayer().bind(__player, $track, $node) && __all_ok;
        )*
        __all_ok
    }};
    ($ctx:expr, $player:expr, $track:expr, $node:expr) => {
        $ctx.AnimPlayer().bind($player, $track, $node)
    };
}

#[macro_export]
macro_rules! anim_player_clear_bindings {
    ($ctx:expr, $player:expr) => {
        $ctx.AnimPlayer().clear_bindings($player)
    };
}
