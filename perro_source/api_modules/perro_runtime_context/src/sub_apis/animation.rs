use perro_ids::{AnimationID, NodeID};

pub trait AnimationAPI {
    fn animation_set_clip(&mut self, player: NodeID, animation: AnimationID) -> bool;
    fn animation_play(&mut self, player: NodeID) -> bool;
    fn animation_pause(&mut self, player: NodeID, paused: bool) -> bool;
    fn animation_seek_time(&mut self, player: NodeID, time_seconds: f32) -> bool;
    fn animation_seek_frame(&mut self, player: NodeID, frame: u32) -> bool;
    fn animation_set_speed(&mut self, player: NodeID, speed: f32) -> bool;
    fn animation_bind(&mut self, player: NodeID, track: &str, node: NodeID) -> bool;
    fn animation_clear_bindings(&mut self, player: NodeID) -> bool;
}

pub struct AnimationModule<'rt, R: AnimationAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: AnimationAPI + ?Sized> AnimationModule<'rt, R> {
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
        $ctx.Animations().set_clip($player, $animation)
    };
}

#[macro_export]
macro_rules! anim_player_play {
    ($ctx:expr, $player:expr) => {
        $ctx.Animations().play($player)
    };
}

#[macro_export]
macro_rules! anim_player_pause {
    ($ctx:expr, $player:expr, $paused:expr) => {
        $ctx.Animations().pause($player, $paused)
    };
}

#[macro_export]
macro_rules! anim_player_seek_time {
    ($ctx:expr, $player:expr, $time:expr) => {
        $ctx.Animations().seek_time($player, $time)
    };
}

#[macro_export]
macro_rules! anim_player_seek_frame {
    ($ctx:expr, $player:expr, $frame:expr) => {
        $ctx.Animations().seek_frame($player, $frame)
    };
}

#[macro_export]
macro_rules! anim_player_set_speed {
    ($ctx:expr, $player:expr, $speed:expr) => {
        $ctx.Animations().set_speed($player, $speed)
    };
}

#[macro_export]
macro_rules! anim_player_bind {
    ($ctx:expr, $player:expr, [ $($track:tt : $node:expr),* $(,)? ]) => {{
        let __player = $player;
        let mut __all_ok = true;
        $(
            __all_ok = $ctx.Animations().bind(__player, $track, $node) && __all_ok;
        )*
        __all_ok
    }};
    ($ctx:expr, $player:expr, { $($track:expr => $node:expr),* $(,)? }) => {{
        let __player = $player;
        let mut __all_ok = true;
        $(
            __all_ok = $ctx.Animations().bind(__player, $track, $node) && __all_ok;
        )*
        __all_ok
    }};
    ($ctx:expr, $player:expr, $track:expr, $node:expr) => {
        $ctx.Animations().bind($player, $track, $node)
    };
}

#[macro_export]
macro_rules! anim_player_clear_bindings {
    ($ctx:expr, $player:expr) => {
        $ctx.Animations().clear_bindings($player)
    };
}

// Backward-compatible aliases.
#[macro_export]
macro_rules! animation_set_clip {
    ($ctx:expr, $player:expr, $animation:expr) => {
        $crate::anim_player_set_clip!($ctx, $player, $animation)
    };
}

#[macro_export]
macro_rules! animation_play {
    ($ctx:expr, $player:expr) => {
        $crate::anim_player_play!($ctx, $player)
    };
}

#[macro_export]
macro_rules! animation_pause {
    ($ctx:expr, $player:expr, $paused:expr) => {
        $crate::anim_player_pause!($ctx, $player, $paused)
    };
}

#[macro_export]
macro_rules! animation_seek_time {
    ($ctx:expr, $player:expr, $time:expr) => {
        $crate::anim_player_seek_time!($ctx, $player, $time)
    };
}

#[macro_export]
macro_rules! animation_seek_frame {
    ($ctx:expr, $player:expr, $frame:expr) => {
        $crate::anim_player_seek_frame!($ctx, $player, $frame)
    };
}

#[macro_export]
macro_rules! animation_set_speed {
    ($ctx:expr, $player:expr, $speed:expr) => {
        $crate::anim_player_set_speed!($ctx, $player, $speed)
    };
}

#[macro_export]
macro_rules! animation_bind {
    ($ctx:expr, $player:expr, [ $($track:tt : $node:expr),* $(,)? ]) => {
        $crate::anim_player_bind!($ctx, $player, [ $($track : $node),* ])
    };
    ($ctx:expr, $player:expr, { $($track:expr => $node:expr),* $(,)? }) => {
        $crate::anim_player_bind!($ctx, $player, { $($track => $node),* })
    };
    ($ctx:expr, $player:expr, $track:expr, $node:expr) => {
        $crate::anim_player_bind!($ctx, $player, $track, $node)
    };
}

#[macro_export]
macro_rules! animation_clear_bindings {
    ($ctx:expr, $player:expr) => {
        $crate::anim_player_clear_bindings!($ctx, $player)
    };
}
