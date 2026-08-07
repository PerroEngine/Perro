use super::*;

impl PerroGraphics {
    pub(super) fn decode_texture_source(
        source: &str,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) -> Option<DecodedTextureRgba> {
        decode_texture_source_rgba(source, static_texture_lookup)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(super) fn start_async_mesh_load(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: MeshID,
        source: String,
    ) {
        self.queued_async_mesh_loads.push(AsyncMeshLoadJob {
            request,
            id,
            source,
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(super) fn flush_async_mesh_loads(&mut self) {
        if self.queued_async_mesh_loads.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.queued_async_mesh_loads);
        let static_mesh_lookup = self.static_mesh_lookup;
        // 1 spawn per job: a scene load queues many meshes in one flush; a
        // single task w/ serial loop pinned them all to one worker thread.
        for job in jobs {
            let tx = self.async_mesh_load_tx.clone();
            rayon::spawn(move || {
                let error = validate_mesh_source(job.source.as_str(), static_mesh_lookup).err();
                let mesh = if error.is_none() {
                    load_mesh3d_from_source(job.source.as_str(), static_mesh_lookup)
                        .map(std::sync::Arc::new)
                } else {
                    None
                };
                let _ = tx.send(AsyncMeshLoadResult {
                    request: job.request,
                    id: job.id,
                    source: job.source,
                    mesh,
                    error,
                });
            });
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn start_async_mesh_load(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: MeshID,
        source: String,
    ) {
        if let Err(reason) = validate_mesh_source(source.as_str(), self.static_mesh_lookup) {
            self.resources.drop_mesh(id);
            self.events.push(RenderEvent::Failed { request, reason });
            return;
        }
        let mesh_data = load_mesh3d_from_source(source.as_str(), self.static_mesh_lookup)
            .map(std::sync::Arc::new);
        if let Some(mesh) = mesh_data.clone() {
            self.resources
                .set_runtime_mesh_data(source.as_str(), mesh.clone());
            let _ = self.resources.set_runtime_mesh_data_by_id(id, mesh);
        }
        self.events.push(RenderEvent::MeshCreated {
            request,
            id,
            mesh: mesh_data,
        });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(super) fn poll_async_mesh_loads(&mut self) {
        while let Ok(result) = self.async_mesh_load_rx.try_recv() {
            let requests = self
                .pending_async_mesh_loads
                .remove(&result.id)
                .unwrap_or_else(|| vec![result.request]);
            if let Some(reason) = result.error {
                self.resources.drop_mesh(result.id);
                for request in requests {
                    self.events.push(RenderEvent::Failed {
                        request,
                        reason: reason.clone(),
                    });
                }
                continue;
            }
            if let Some(mesh) = result.mesh.clone() {
                self.resources
                    .set_runtime_mesh_data(result.source.as_str(), mesh.clone());
                let _ = self.resources.set_runtime_mesh_data_by_id(result.id, mesh);
            }
            for request in requests {
                self.events.push(RenderEvent::MeshCreated {
                    request,
                    id: result.id,
                    mesh: result.mesh.clone(),
                });
            }
            self.redraw_requested = true;
        }
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn poll_async_mesh_loads(&mut self) {}

    #[cfg(any(target_arch = "wasm32", test))]
    pub(super) fn flush_async_mesh_loads(&mut self) {}

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn start_async_texture_load(&mut self, id: TextureID, source: String) {
        self.queued_async_texture_loads
            .push(AsyncTextureLoadJob { id, source });
    }

    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    pub(super) fn flush_async_texture_loads(&mut self) {
        if self.queued_async_texture_loads.is_empty() {
            return;
        }
        let jobs = std::mem::take(&mut self.queued_async_texture_loads);
        let static_texture_lookup = self.static_texture_lookup;
        // 1 spawn per job: parallel read+decode across the pool (see mesh note).
        // The decode stage inside is capped by DECODE_GATE so a wide pool
        // cannot hold N full-size rgba buffers in flight at once.
        for job in jobs {
            let tx = self.async_texture_load_tx.clone();
            rayon::spawn(move || {
                let texture =
                    decode_texture_source_rgba_gated(job.source.as_str(), static_texture_lookup)
                        .ok_or_else(|| format!("failed to decode texture source `{}`", job.source));
                let _ = tx.send(AsyncTextureLoadResult {
                    id: job.id,
                    texture,
                });
            });
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), test))]
    pub(super) fn flush_async_texture_loads(&mut self) {
        let jobs = std::mem::take(&mut self.queued_async_texture_loads);
        for job in jobs {
            let texture =
                Self::decode_texture_source(job.source.as_str(), self.static_texture_lookup)
                    .ok_or_else(|| format!("failed to decode texture source `{}`", job.source));
            let _ = self.async_texture_load_tx.send(AsyncTextureLoadResult {
                id: job.id,
                texture,
            });
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn start_async_texture_load(
        &mut self,
        request: perro_render_bridge::RenderRequestID,
        id: TextureID,
        source: String,
    ) {
        match Self::decode_texture_source(source.as_str(), self.static_texture_lookup) {
            Some(texture) => {
                if self.resources.set_decoded_texture_data(id, texture) {
                    self.events
                        .push(RenderEvent::TextureCreated { request, id });
                    self.events.push(RenderEvent::TextureLoaded { id });
                } else {
                    self.resources.drop_texture(id);
                    self.events.push(RenderEvent::Failed {
                        request,
                        reason: format!("failed to decode texture source `{source}`"),
                    });
                }
            }
            _ => {
                self.resources.drop_texture(id);
                self.events.push(RenderEvent::Failed {
                    request,
                    reason: format!("failed to decode texture source `{source}`"),
                });
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_async_texture_loads(&mut self) {
        while let Ok(result) = self.async_texture_load_rx.try_recv() {
            let Some(requests) = self.pending_async_texture_loads.remove(&result.id) else {
                continue;
            };
            match result.texture {
                Ok(texture) => {
                    if self.resources.set_decoded_texture_data(result.id, texture) {
                        for request in requests {
                            self.events.push(RenderEvent::TextureCreated {
                                request,
                                id: result.id,
                            });
                        }
                        self.events
                            .push(RenderEvent::TextureLoaded { id: result.id });
                        self.redraw_requested = true;
                    } else {
                        for request in requests {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: "texture dropped before async load completed".to_string(),
                            });
                        }
                    }
                }
                Err(reason) => {
                    self.resources.drop_texture(result.id);
                    for request in requests {
                        self.events.push(RenderEvent::Failed {
                            request,
                            reason: reason.clone(),
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn poll_async_texture_loads(&mut self) {}

    #[cfg(target_arch = "wasm32")]
    pub(super) fn flush_async_texture_loads(&mut self) {}
}

/// Decode a texture's pixels straight from its source (builtin, static PTEX
/// lookup, or file). Shared by the initial async load and by consumers whose
/// resident CPU copy was reclaimed by the idle sweep.
pub(crate) fn decode_texture_source_rgba(
    source: &str,
    static_texture_lookup: Option<StaticTextureLookup>,
) -> Option<DecodedTextureRgba> {
    let (rgba, width, height): (Arc<[u8]>, u32, u32) = if source == "__default__" {
        (Arc::from(&[255u8, 255, 255, 255][..]), 1, 1)
    } else if source == "__perro_builtin_logo_svg__" {
        decode_image_rgba_arc(perro_builtin_assets::PERRO_LOGO_SVG)?
    } else if let Some(lookup) = static_texture_lookup {
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty() {
            let (rgba, width, height) = decode_ptex(bytes)?;
            (rgba.into(), width, height)
        } else {
            load_texture_rgba_arc(source)?
        }
    } else {
        load_texture_rgba_arc(source)?
    };
    Some(DecodedTextureRgba {
        rgba,
        width: width.max(1),
        height: height.max(1),
    })
}

/// Max simultaneous CPU image decodes across the async load pool. One 2048^2
/// decode transiently holds the encoded bytes + a ~16MB rgba buffer + its
/// Arc copy; an unbounded rayon fan-out on a 16-core machine peaked at ~16x
/// that. 4 keeps the pool fed while bounding transient decode memory.
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
const MAX_CONCURRENT_TEXTURE_DECODES: usize = 4;

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct DecodeGate {
    active: std::sync::Mutex<usize>,
    ready: std::sync::Condvar,
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
static DECODE_GATE: DecodeGate = DecodeGate {
    active: std::sync::Mutex::new(0),
    ready: std::sync::Condvar::new(),
};

/// RAII permit: at most `MAX_CONCURRENT_TEXTURE_DECODES` alive at once.
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
struct DecodePermit;

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
impl DecodePermit {
    fn acquire() -> Self {
        let mut active = DECODE_GATE
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *active >= MAX_CONCURRENT_TEXTURE_DECODES {
            active = DECODE_GATE
                .ready
                .wait(active)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *active += 1;
        DecodePermit
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(test)))]
impl Drop for DecodePermit {
    fn drop(&mut self) {
        let mut active = DECODE_GATE
            .active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *active = active.saturating_sub(1);
        drop(active);
        DECODE_GATE.ready.notify_one();
    }
}

/// `decode_texture_source_rgba` for the rayon async-load path: file IO runs
/// ungated, only the CPU decode stage takes a `DecodePermit`. The render-thread
/// re-decode path stays on the ungated fn and never blocks behind the pool.
#[cfg(all(not(target_arch = "wasm32"), not(test)))]
fn decode_texture_source_rgba_gated(
    source: &str,
    static_texture_lookup: Option<StaticTextureLookup>,
) -> Option<DecodedTextureRgba> {
    // constant/builtin sources: trivial or served from the svg raster cache.
    if source == "__default__" || source == "__perro_builtin_logo_svg__" {
        return decode_texture_source_rgba(source, static_texture_lookup);
    }
    if let Some(lookup) = static_texture_lookup {
        let source_hash = perro_ids::parse_hashed_source_uri(source)
            .unwrap_or_else(|| perro_ids::string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty() {
            // static bytes: no IO at all, gate the decode.
            let _permit = DecodePermit::acquire();
            let (rgba, width, height) = decode_ptex(bytes)?;
            return Some(DecodedTextureRgba {
                rgba: rgba.into(),
                width: width.max(1),
                height: height.max(1),
            });
        }
    }
    if source.contains(".glb") || source.contains(".gltf") {
        // gltf-embedded textures interleave buffer IO with decode; gate the
        // whole call (rare path, and the decode still dominates).
        let _permit = DecodePermit::acquire();
        return decode_texture_source_rgba(source, None);
    }
    // plain image/ptex/svg file: read the encoded bytes first (IO outside the
    // gate), then decode under a permit. decode_image_rgba_arc sniffs PTEX
    // magic and svg content just like load_texture_rgba does.
    let bytes = perro_io::load_asset_cow(source).ok()?;
    let _permit = DecodePermit::acquire();
    let (rgba, width, height) = decode_image_rgba_arc(&bytes)?;
    Some(DecodedTextureRgba {
        rgba,
        width: width.max(1),
        height: height.max(1),
    })
}
