use crate::prelude::*;
use perro_nodes::UiAnimatedImage;

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
    let changed = with_node_mut!(ctx, UiAnimatedImage, id, |image| {
        step_ui_animated_image(image, delta)
    })
    .unwrap_or(false);

    if changed {
        let _ = force_rerender!(ctx, id);
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

fn step_ui_animated_image(image: &mut UiAnimatedImage, delta_seconds: f32) -> bool {
    let Some(animation) = image.current_animation_data() else {
        return false;
    };
    let frame_count = animation.frame_count.max(1);
    let fps = animation.fps.max(0.0) * image.fps_scale.max(0.0);
    image.current_frame = image.current_frame.min(frame_count.saturating_sub(1));
    if !image.playing || fps <= 0.0 || frame_count <= 1 {
        return false;
    }

    image.frame_accum += delta_seconds * fps;
    let steps = image.frame_accum.floor() as u32;
    if steps == 0 {
        return false;
    }
    image.frame_accum -= steps as f32;

    let previous = image.current_frame;
    if image.looping {
        image.current_frame = (image.current_frame + steps) % frame_count;
    } else {
        image.current_frame = image
            .current_frame
            .saturating_add(steps)
            .min(frame_count.saturating_sub(1));
        if image.current_frame == frame_count.saturating_sub(1) {
            image.playing = false;
            image.frame_accum = 0.0;
        }
    }

    image.current_frame != previous
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loops_frames() {
        let mut image = UiAnimatedImage::new();
        let mut animation = perro_nodes::UiAnimatedImageFrameSet::new("default");
        animation.frame_count = 4;
        animation.fps = 10.0;
        image.animations.push(animation);

        assert!(step_ui_animated_image(&mut image, 0.25));
        assert_eq!(image.current_frame, 2);

        assert!(step_ui_animated_image(&mut image, 0.25));
        assert_eq!(image.current_frame, 1);
    }

    #[test]
    fn non_loop_stops_on_last_frame() {
        let mut image = UiAnimatedImage::new();
        let mut animation = perro_nodes::UiAnimatedImageFrameSet::new("default");
        animation.frame_count = 3;
        animation.fps = 10.0;
        image.animations.push(animation);
        image.looping = false;

        assert!(step_ui_animated_image(&mut image, 1.0));
        assert_eq!(image.current_frame, 2);
        assert!(!image.playing);
    }
}
