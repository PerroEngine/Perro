use perro_ids::NodeID;
use perro_render_bridge::{CameraStreamSourceState, CameraStreamState};
use std::sync::Arc;

/// Source kind understood by the renderer capture sink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureRenderSource {
    MainWindow,
    Camera2D(NodeID),
    Camera3D(NodeID),
    UISubView(NodeID),
    RenderTarget(NodeID),
}

/// Aspect policy for source extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureRenderFraming {
    Fit,
    Crop,
    Expand,
    Stretch,
}

/// Source route passed from runtime/app into renderer capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRenderRoute {
    pub source: CaptureRenderSource,
    pub render_node: Option<NodeID>,
    pub source_size: [u32; 2],
    pub target_size: [u32; 2],
    pub framing: CaptureRenderFraming,
}

/// Resolve a route to an active retained stream node.
pub fn resolve_stream_node(
    route: CaptureRenderRoute,
    streams: &[(NodeID, Arc<CameraStreamState>)],
) -> Result<Option<NodeID>, String> {
    if !matches!(route.source, CaptureRenderSource::MainWindow) {
        let node = route
            .render_node
            .or(match route.source {
                CaptureRenderSource::Camera2D(node)
                | CaptureRenderSource::Camera3D(node)
                | CaptureRenderSource::UISubView(node)
                | CaptureRenderSource::RenderTarget(node) => Some(node),
                CaptureRenderSource::MainWindow => None,
            })
            .ok_or_else(|| "capture source has no render node".to_owned())?;
        let (_, state) = streams
            .iter()
            .find(|(stream_node, _)| *stream_node == node)
            .ok_or_else(|| format!("capture render stream {node} not active"))?;
        match route.source {
            CaptureRenderSource::Camera2D(_)
                if !matches!(state.source, CameraStreamSourceState::TwoD(_)) =>
            {
                return Err("capture route needs a 2D stream".to_owned());
            }
            CaptureRenderSource::Camera3D(_)
                if !matches!(state.source, CameraStreamSourceState::ThreeD(_)) =>
            {
                return Err("capture route needs a 3D stream".to_owned());
            }
            _ => {}
        }
        Ok(Some(node))
    } else {
        Ok(None)
    }
}

/// Pixel mapping for source -> capture target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureFrameRect {
    pub source_origin: [u32; 2],
    pub source_size: [u32; 2],
    pub target_origin: [u32; 2],
    pub target_size: [u32; 2],
}

/// Compute framing without float drift at pixel edges.
pub fn frame_rect(
    source: [u32; 2],
    target: [u32; 2],
    framing: CaptureRenderFraming,
) -> Result<CaptureFrameRect, String> {
    if source.contains(&0) || target.contains(&0) {
        return Err("capture frame dimensions must be > 0".to_owned());
    }
    if framing == CaptureRenderFraming::Stretch {
        return Ok(CaptureFrameRect {
            source_origin: [0, 0],
            source_size: source,
            target_origin: [0, 0],
            target_size: target,
        });
    }
    let source_ratio = u64::from(source[0]) * u64::from(target[1]);
    let target_ratio = u64::from(target[0]) * u64::from(source[1]);
    match framing {
        CaptureRenderFraming::Crop => {
            let source_size = if source_ratio > target_ratio {
                [
                    u64::from(target[0]) * u64::from(source[1]) / u64::from(target[1]),
                    u64::from(source[1]),
                ]
            } else {
                [
                    u64::from(source[0]),
                    u64::from(target[1]) * u64::from(source[0]) / u64::from(target[0]),
                ]
            };
            let source_size = [
                u32::try_from(source_size[0].max(1)).map_err(|_| "source crop overflow")?,
                u32::try_from(source_size[1].max(1)).map_err(|_| "source crop overflow")?,
            ];
            Ok(CaptureFrameRect {
                source_origin: [
                    (source[0] - source_size[0]) / 2,
                    (source[1] - source_size[1]) / 2,
                ],
                source_size,
                target_origin: [0, 0],
                target_size: target,
            })
        }
        CaptureRenderFraming::Fit | CaptureRenderFraming::Expand => {
            let target_size = if source_ratio > target_ratio {
                [
                    target[0],
                    u32::try_from(
                        u64::from(target[0]) * u64::from(source[1]) / u64::from(source[0]),
                    )
                    .map_err(|_| "target fit overflow")?
                    .max(1),
                ]
            } else {
                [
                    u32::try_from(
                        u64::from(target[1]) * u64::from(source[0]) / u64::from(source[1]),
                    )
                    .map_err(|_| "target fit overflow")?
                    .max(1),
                    target[1],
                ]
            };
            Ok(CaptureFrameRect {
                source_origin: [0, 0],
                source_size: source,
                target_origin: [
                    (target[0] - target_size[0]) / 2,
                    (target[1] - target_size[1]) / 2,
                ],
                target_size,
            })
        }
        CaptureRenderFraming::Stretch => unreachable!(),
    }
}

/// Scale/crop RGBA source into target; fit bars stay transparent.
pub fn frame_rgba(
    rgba: &[u8],
    source: [u32; 2],
    target: [u32; 2],
    framing: CaptureRenderFraming,
) -> Result<Vec<u8>, String> {
    let source_len = usize::try_from(u64::from(source[0]) * u64::from(source[1]) * 4)
        .map_err(|_| "source frame buffer too large")?;
    let target_len = usize::try_from(u64::from(target[0]) * u64::from(target[1]) * 4)
        .map_err(|_| "target frame buffer too large")?;
    if rgba.len() != source_len {
        return Err("source RGBA buffer length mismatch".to_owned());
    }
    let rect = frame_rect(source, target, framing)?;
    let mut out = vec![0; target_len];
    for y in 0..rect.target_size[1] {
        let source_y = rect.source_origin[1]
            + (u64::from(y) * u64::from(rect.source_size[1]) / u64::from(rect.target_size[1]))
                as u32;
        for x in 0..rect.target_size[0] {
            let source_x = rect.source_origin[0]
                + (u64::from(x) * u64::from(rect.source_size[0]) / u64::from(rect.target_size[0]))
                    as u32;
            let source_index =
                ((u64::from(source_y) * u64::from(source[0]) + u64::from(source_x)) * 4) as usize;
            let target_index = ((u64::from(rect.target_origin[1] + y) * u64::from(target[0])
                + u64::from(rect.target_origin[0] + x))
                * 4) as usize;
            out[target_index..target_index + 4]
                .copy_from_slice(&rgba[source_index..source_index + 4]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::TextureID;
    use perro_render_bridge::CameraStreamState;

    fn stream(source: CameraStreamSourceState) -> Arc<CameraStreamState> {
        Arc::new(CameraStreamState {
            ui_commands: Arc::from([]),
            source,
            tone_map_output: false,
            overlay_camera_2d: None,
            transparent_background: false,
            clear_color: None,
            resolution: [2, 2],
            aspect_ratio: 0.0,
            post_processing: Arc::from([]),
            output_texture: TextureID::nil(),
            sprites_2d: Arc::from([]),
            lights_2d: Arc::from([]),
            point_particles_2d: Arc::from([]),
            waters_2d: Arc::from([]),
            draws_3d: Arc::from([]),
            lighting_3d: Default::default(),
            point_particles_3d: Arc::from([]),
            waters_3d: Arc::from([]),
        })
    }

    #[test]
    fn route_selects_stream_and_checks_kind() {
        let node = NodeID::from_parts(7, 0);
        let streams = vec![(
            node,
            stream(CameraStreamSourceState::TwoD(Default::default())),
        )];
        let route = CaptureRenderRoute {
            source: CaptureRenderSource::Camera2D(node),
            render_node: Some(node),
            source_size: [2, 2],
            target_size: [2, 2],
            framing: CaptureRenderFraming::Fit,
        };
        assert_eq!(resolve_stream_node(route, &streams), Ok(Some(node)));
        let wrong = CaptureRenderRoute {
            source: CaptureRenderSource::Camera3D(node),
            ..route
        };
        assert!(resolve_stream_node(wrong, &streams).is_err());
    }

    #[test]
    fn frame_crop_and_fit_keep_exact_target() {
        let source = [4, 2];
        let rgba: Vec<u8> = (0..32).collect();
        let fit = frame_rgba(&rgba, source, [2, 2], CaptureRenderFraming::Fit)
            .unwrap_or_else(|error| panic!("fit: {error}"));
        assert_eq!(fit.len(), 16);
        let crop = frame_rgba(&rgba, source, [2, 2], CaptureRenderFraming::Crop)
            .unwrap_or_else(|error| panic!("crop: {error}"));
        assert_eq!(crop.len(), 16);
        assert_eq!(crop[0..4], rgba[4..8]);
    }
}
