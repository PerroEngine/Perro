use crate::prelude::*;
use perro_nodes::AnimatedSprite2D;

pub fn internal_update<RT, R, IP>(
    ctx: &mut RuntimeWindow<'_, RT>,
    _res: &ResourceWindow<'_, R>,
    _ipt: &InputWindow<'_, IP>,
    id: NodeID,
) where
    RT: RuntimeAPI + ?Sized,
    R: ResourceAPI + ?Sized,
    IP: InputAPI + ?Sized,
{
    let delta = delta_time!(ctx).max(0.0);
    let changed = with_node_mut!(ctx, AnimatedSprite2D, id, |sprite| {
        step_animated_sprite(sprite, delta)
    })
    .unwrap_or(false);

    if changed {
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

fn step_animated_sprite(sprite: &mut AnimatedSprite2D, delta_seconds: f32) -> bool {
    let Some(animation) = sprite.current_animation_data() else {
        return false;
    };
    let frame_count = animation.frame_count.max(1);
    let fps = animation.fps.max(0.0) * sprite.fps_scale.max(0.0);
    sprite.current_frame = sprite.current_frame.min(frame_count.saturating_sub(1));
    if !sprite.playing || fps <= 0.0 || frame_count <= 1 {
        return false;
    }

    sprite.frame_accum += delta_seconds * fps;
    let steps = sprite.frame_accum.floor() as u32;
    if steps == 0 {
        return false;
    }
    sprite.frame_accum -= steps as f32;

    let previous = sprite.current_frame;
    if sprite.looping {
        sprite.current_frame = (sprite.current_frame + steps) % frame_count;
    } else {
        sprite.current_frame = sprite
            .current_frame
            .saturating_add(steps)
            .min(frame_count.saturating_sub(1));
        if sprite.current_frame == frame_count.saturating_sub(1) {
            sprite.playing = false;
            sprite.frame_accum = 0.0;
        }
    }

    sprite.current_frame != previous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loops_frames() {
        let mut sprite = AnimatedSprite2D::new();
        let mut animation = perro_nodes::AnimatedSprite::new("default");
        animation.frame_count = 4;
        animation.fps = 10.0;
        sprite.animations.push(animation);

        assert!(step_animated_sprite(&mut sprite, 0.25));
        assert_eq!(sprite.current_frame, 2);

        assert!(step_animated_sprite(&mut sprite, 0.25));
        assert_eq!(sprite.current_frame, 1);
    }

    #[test]
    fn non_loop_stops_on_last_frame() {
        let mut sprite = AnimatedSprite2D::new();
        let mut animation = perro_nodes::AnimatedSprite::new("default");
        animation.frame_count = 3;
        animation.fps = 10.0;
        sprite.animations.push(animation);
        sprite.looping = false;

        assert!(step_animated_sprite(&mut sprite, 1.0));
        assert_eq!(sprite.current_frame, 2);
        assert!(!sprite.playing);
    }
}
