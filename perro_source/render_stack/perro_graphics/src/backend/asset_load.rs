use super::*;

impl PerroGraphics {
    pub(super) fn decode_texture_source(
        source: &str,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) -> Option<DecodedTextureRgba> {
        let (rgba, width, height) = if source == "__default__" {
            (vec![255u8, 255, 255, 255], 1, 1)
        } else if source == "__perro_builtin_logo_svg__" {
            decode_image_rgba(include_bytes!(
                "../../../../api_modules/perro_api/src/assets/perro.svg"
            ))?
        } else if let Some(lookup) = static_texture_lookup {
            let source_hash = perro_ids::parse_hashed_source_uri(source)
                .unwrap_or_else(|| perro_ids::string_to_u64(source));
            let bytes = lookup(source_hash);
            if !bytes.is_empty() {
                decode_ptex(bytes)?
            } else {
                Self::decode_texture_file(source)?
            }
        } else {
            Self::decode_texture_file(source)?
        };
        Some(DecodedTextureRgba {
            rgba,
            width: width.max(1),
            height: height.max(1),
        })
    }

    pub(super) fn decode_texture_file(source: &str) -> Option<(Vec<u8>, u32, u32)> {
        load_texture_rgba(source)
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
        let tx = self.async_mesh_load_tx.clone();
        let static_mesh_lookup = self.static_mesh_lookup;
        rayon::spawn(move || {
            for job in jobs {
                let error = validate_mesh_source(job.source.as_str(), static_mesh_lookup).err();
                let mesh = if error.is_none() {
                    load_mesh3d_from_source(job.source.as_str(), static_mesh_lookup)
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
            }
        });
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
        let mesh_data = load_mesh3d_from_source(source.as_str(), self.static_mesh_lookup);
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
        let tx = self.async_texture_load_tx.clone();
        let static_texture_lookup = self.static_texture_lookup;
        rayon::spawn(move || {
            for job in jobs {
                let texture =
                    Self::decode_texture_source(job.source.as_str(), static_texture_lookup)
                        .ok_or_else(|| format!("failed to decode texture source `{}`", job.source));
                let _ = tx.send(AsyncTextureLoadResult {
                    id: job.id,
                    texture,
                });
            }
        });
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
