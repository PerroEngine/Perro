use crate::prelude::*;
use perro_nodes::{UiVideoPlayer, VideoPlayer2D, VideoPlayer3D};

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    res: &ResourceWindow<'_, R>,
    _ipt: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let delta = delta_time!(ctx).max(0.0);

    let mut needs_rerender = false;
    let updated = with_node_mut!(ctx, VideoPlayer2D, id, |node| {
        let update = res.Videos().update_node(id, &node.video, delta);
        let changed = node.video.texture != update.texture || update.frame_changed;
        node.video.texture = update.texture;
        changed
    })
    .or_else(|| {
        with_node_mut!(ctx, VideoPlayer3D, id, |node| {
            let update = res.Videos().update_node(id, &node.video, delta);
            let changed = node.video.texture != update.texture || update.frame_changed;
            node.video.texture = update.texture;
            changed
        })
    })
    .or_else(|| {
        with_node_mut!(ctx, UiVideoPlayer, id, |node| {
            let update = res.Videos().update_node(id, &node.video, delta);
            let changed = node.video.texture != update.texture || update.frame_changed;
            node.video.texture = update.texture;
            changed
        })
    })
    .unwrap_or(false);

    if updated {
        needs_rerender = true;
    }

    if needs_rerender {
        let _ = ctx.Nodes().mark_needs_rerender(id);
    }
}

pub fn internal_fixed_update<RT, R, IP>(
    _ctx: &mut RuntimeWindow<'_, RT>,
    _res: &ResourceWindow<'_, R>,
    _ipt: &InputWindow<'_, IP>,
    _id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
}
