use super::core::{RuntimeResourceApi, RuntimeVideoClip, RuntimeVideoFrame, RuntimeVideoNode};
use perro_ids::{NodeID, TextureID, string_to_u64};
use perro_render_bridge::{RenderCommand, ResourceCommand};
use perro_resource_api::sub_apis::{VideoAPI, VideoUpdate};
use std::sync::Arc;

const FALLBACK_RGBA: [u8; 4] = [0, 0, 0, 255];

impl VideoAPI for RuntimeResourceApi {
    fn video_update_node(
        &self,
        node: NodeID,
        player: &perro_nodes::VideoPlayer,
        delta_seconds: f32,
    ) -> VideoUpdate {
        let source = player.source.as_ref().trim();
        if source.is_empty() {
            let _ = self.video_release_node(node);
            return VideoUpdate {
                texture: TextureID::nil(),
                frame_changed: false,
            };
        }

        let source_hash = string_to_u64(source);
        let clip = self.video_clip(source_hash, source);
        let Some(clip) = clip else {
            return self.video_fallback_texture(node, source_hash);
        };
        if clip.frames.is_empty() {
            return self.video_fallback_texture(node, source_hash);
        }

        let mut frame_changed = false;
        let mut nodes = self
            .video_node_state
            .lock()
            .expect("video node mutex poisoned");
        let entry = nodes.entry(node).or_insert_with(|| {
            frame_changed = true;
            RuntimeVideoNode {
                source_hash,
                texture: TextureID::nil(),
                frame_index: 0,
                accum: 0.0,
            }
        });

        if entry.source_hash != source_hash || entry.texture.is_nil() {
            if !entry.texture.is_nil() {
                let _ = self.drop_video_texture(entry.texture);
            }
            entry.source_hash = source_hash;
            entry.frame_index = 0;
            entry.accum = 0.0;
            entry.texture = self.create_video_texture(node, &clip);
            frame_changed = true;
        }

        let frame_count = clip.frames.len();
        if player.playing && frame_count > 1 {
            let fps = clip.fps.max(0.0) * player.fps_scale.max(0.0);
            if fps > 0.0 {
                entry.accum += delta_seconds * fps;
                let steps = entry.accum.floor() as usize;
                if steps > 0 {
                    entry.accum -= steps as f32;
                    let before = entry.frame_index;
                    if player.looping {
                        entry.frame_index = (entry.frame_index + steps) % frame_count;
                    } else {
                        entry.frame_index = entry
                            .frame_index
                            .saturating_add(steps)
                            .min(frame_count.saturating_sub(1));
                    }
                    frame_changed |= entry.frame_index != before;
                }
            }
        }

        if frame_changed {
            let frame = &clip.frames[entry.frame_index];
            self.write_video_texture(entry.texture, clip.width, clip.height, frame.rgba.clone());
        }

        VideoUpdate {
            texture: entry.texture,
            frame_changed,
        }
    }

    fn video_release_node(&self, node: NodeID) -> bool {
        let texture = self
            .video_node_state
            .lock()
            .ok()
            .and_then(|mut nodes| nodes.remove(&node).map(|state| state.texture));
        if let Some(texture) = texture
            && !texture.is_nil()
        {
            return self.drop_video_texture(texture);
        }
        false
    }
}

impl RuntimeResourceApi {
    fn video_clip(&self, source_hash: u64, source: &str) -> Option<Arc<RuntimeVideoClip>> {
        if let Some(clip) = self
            .video_clip_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&source_hash).cloned())
        {
            return Some(clip);
        }

        let clip = load_y4m_clip(source).ok().map(Arc::new)?;
        if let Ok(mut cache) = self.video_clip_cache.lock() {
            cache.insert(source_hash, clip.clone());
        }
        Some(clip)
    }

    fn create_video_texture(&self, node: NodeID, clip: &RuntimeVideoClip) -> TextureID {
        let first = clip
            .frames
            .first()
            .map(|frame| frame.rgba.clone())
            .unwrap_or_else(|| Arc::from(FALLBACK_RGBA.as_slice()));
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let request = state.allocate_request();
        let id = state.allocate_texture_id();
        let source = format!("video://node/{}", node.as_u64());
        let source_hash = string_to_u64(&source);
        state.texture_by_source.insert(source_hash, id);
        state.texture_pending_by_source.insert(source_hash, request);
        state
            .texture_pending_source_by_request
            .insert(request, source.clone());
        state.texture_pending_id_by_request.insert(request, id);
        state.queued_commands.push(RenderCommand::Resource(
            ResourceCommand::CreateRuntimeTexture {
                request,
                id,
                source,
                reserved: true,
                width: clip.width.max(1),
                height: clip.height.max(1),
                rgba: first,
            },
        ));
        id
    }

    fn write_video_texture(&self, texture: TextureID, width: u32, height: u32, rgba: Arc<[u8]>) {
        if texture.is_nil() {
            return;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::WriteTextureRgba {
                id: texture,
                width: width.max(1),
                height: height.max(1),
                rgba,
            }));
    }

    fn drop_video_texture(&self, texture: TextureID) -> bool {
        if texture.is_nil() {
            return false;
        }
        let mut state = self.state.lock().expect("resource api mutex poisoned");
        let _ = state.free_texture_id(texture);
        state.texture_loaded_by_id.remove(&texture);
        state
            .texture_by_source
            .retain(|_, existing| *existing != texture);
        state
            .queued_commands
            .push(RenderCommand::Resource(ResourceCommand::DropTexture {
                id: texture,
            }));
        true
    }

    fn video_fallback_texture(&self, node: NodeID, source_hash: u64) -> VideoUpdate {
        let clip = RuntimeVideoClip {
            width: 1,
            height: 1,
            fps: 1.0,
            frames: Arc::from([RuntimeVideoFrame {
                rgba: Arc::from(FALLBACK_RGBA.as_slice()),
            }]),
        };
        let mut nodes = self
            .video_node_state
            .lock()
            .expect("video node mutex poisoned");
        let entry = nodes.entry(node).or_insert(RuntimeVideoNode {
            source_hash,
            texture: TextureID::nil(),
            frame_index: 0,
            accum: 0.0,
        });
        let mut frame_changed = false;
        if entry.source_hash != source_hash || entry.texture.is_nil() {
            if !entry.texture.is_nil() {
                let _ = self.drop_video_texture(entry.texture);
            }
            entry.source_hash = source_hash;
            entry.frame_index = 0;
            entry.accum = 0.0;
            entry.texture = self.create_video_texture(node, &clip);
            frame_changed = true;
        }
        VideoUpdate {
            texture: entry.texture,
            frame_changed,
        }
    }
}

fn load_y4m_clip(source: &str) -> Result<RuntimeVideoClip, String> {
    if !source.ends_with(".y4m") {
        return Err("only .y4m video is supported in this build".to_string());
    }
    let bytes = perro_io::load_asset(source).map_err(|err| err.to_string())?;
    let mut decoder =
        y4m::Decoder::new(std::io::Cursor::new(bytes)).map_err(|err| err.to_string())?;
    let width = u32::try_from(decoder.get_width()).map_err(|_| "video width too large")?;
    let height = u32::try_from(decoder.get_height()).map_err(|_| "video height too large")?;
    let fps_ratio = decoder.get_framerate();
    let fps = if fps_ratio.den == 0 {
        30.0
    } else {
        fps_ratio.num as f32 / fps_ratio.den as f32
    };
    let colorspace = decoder.get_colorspace();
    let mut frames = Vec::new();
    loop {
        match decoder.read_frame() {
            Ok(frame) => frames.push(RuntimeVideoFrame {
                rgba: y4m_frame_rgba(width as usize, height as usize, colorspace, &frame)?.into(),
            }),
            Err(y4m::Error::EOF) => break,
            Err(err) => return Err(err.to_string()),
        }
    }
    Ok(RuntimeVideoClip {
        width,
        height,
        fps: fps.max(0.001),
        frames: Arc::from(frames),
    })
}

fn y4m_frame_rgba(
    width: usize,
    height: usize,
    colorspace: y4m::Colorspace,
    frame: &y4m::Frame<'_>,
) -> Result<Vec<u8>, String> {
    if colorspace.get_bit_depth() != 8 {
        return Err("only 8-bit y4m video is supported".to_string());
    }
    let y = frame.get_y_plane();
    let u = frame.get_u_plane();
    let v = frame.get_v_plane();
    let mut rgba = vec![0u8; width * height * 4];
    for py in 0..height {
        for px in 0..width {
            let yv = *y.get(py * width + px).unwrap_or(&0);
            let (uv, vv) = match colorspace {
                y4m::Colorspace::Cmono => (128, 128),
                y4m::Colorspace::C420
                | y4m::Colorspace::C420jpeg
                | y4m::Colorspace::C420paldv
                | y4m::Colorspace::C420mpeg2 => {
                    let cw = width.div_ceil(2);
                    let ci = (py / 2) * cw + (px / 2);
                    (*u.get(ci).unwrap_or(&128), *v.get(ci).unwrap_or(&128))
                }
                y4m::Colorspace::C422 => {
                    let cw = width.div_ceil(2);
                    let ci = py * cw + (px / 2);
                    (*u.get(ci).unwrap_or(&128), *v.get(ci).unwrap_or(&128))
                }
                y4m::Colorspace::C444 => {
                    let ci = py * width + px;
                    (*u.get(ci).unwrap_or(&128), *v.get(ci).unwrap_or(&128))
                }
                _ => return Err("unsupported y4m colorspace".to_string()),
            };
            let [r, g, b] = yuv_to_rgb(yv, uv, vv);
            let out = (py * width + px) * 4;
            rgba[out] = r;
            rgba[out + 1] = g;
            rgba[out + 2] = b;
            rgba[out + 3] = 255;
        }
    }
    Ok(rgba)
}

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;
    [
        clamp_u8((298 * c + 409 * e + 128) >> 8),
        clamp_u8((298 * c - 100 * d - 208 * e + 128) >> 8),
        clamp_u8((298 * c + 516 * d + 128) >> 8),
    ]
}

fn clamp_u8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}
