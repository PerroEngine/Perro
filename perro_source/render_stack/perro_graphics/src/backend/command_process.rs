use super::*;

impl PerroGraphics {
    pub(super) fn process_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
        for command in commands {
            match command {
                RenderCommand::CameraStream(command) => match command {
                    CameraStreamCommand::Upsert { node, state } => {
                        let state = *state;
                        upsert_camera_stream_state(
                            &mut self.retained_camera_streams,
                            node,
                            state.clone(),
                        );
                        if camera_stream_uses_render_target(&state) {
                            self.upsert_camera_stream_texture(
                                node,
                                state.output_texture,
                                state.resolution,
                            );
                        }
                    }
                    CameraStreamCommand::RemoveNode { node } => {
                        let id = camera_stream_texture_id(node);
                        self.camera_stream_targets.remove(&node);
                        self.retained_camera_streams.retain(|(id, _)| *id != node);
                        let _ = self.resources.drop_texture(id);
                    }
                },
                RenderCommand::Resource(resource_cmd) => match resource_cmd {
                    ResourceCommand::CreateMesh {
                        request,
                        id,
                        source,
                        reserved,
                    } => {
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        if asset_ready_log_enabled() {
                            eprintln!(
                                "[perro][asset-ready] backend mesh request id={out_id:?} source={source}"
                            );
                        }
                        if let Some(mesh) = self.resources.runtime_mesh_data_by_id(out_id).cloned()
                        {
                            self.events.push(RenderEvent::MeshCreated {
                                request,
                                id: out_id,
                                mesh: Some(mesh),
                            });
                            continue;
                        }
                        #[cfg(all(not(target_arch = "wasm32"), not(test)))]
                        {
                            let waiters = self.pending_async_mesh_loads.entry(out_id).or_default();
                            let start_load = waiters.is_empty();
                            if !waiters.contains(&request) {
                                waiters.push(request);
                            }
                            if start_load {
                                self.start_async_mesh_load(request, out_id, source);
                            }
                        }
                        #[cfg(any(target_arch = "wasm32", test))]
                        self.start_async_mesh_load(request, out_id, source);
                    }
                    ResourceCommand::CreateRuntimeMesh {
                        request,
                        id,
                        source,
                        reserved,
                        mesh,
                    } => {
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        self.resources
                            .set_runtime_mesh_data(source.as_str(), mesh.clone());
                        let _ = self
                            .resources
                            .set_runtime_mesh_data_by_id(out_id, mesh.clone());
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                            mesh: Some(mesh),
                        });
                    }
                    ResourceCommand::CreateRuntimeMeshBytes {
                        request,
                        id,
                        source,
                        reserved,
                        bytes,
                    } => {
                        let Some(mesh) = load_mesh3d_from_bytes(bytes.as_ref()) else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!("invalid mesh bytes len={}", bytes.len()),
                            });
                            continue;
                        };
                        let out_id = if id.is_nil() {
                            self.resources.create_mesh(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_mesh_with_id(id, source.as_str(), reserved)
                        };
                        self.resources
                            .set_runtime_mesh_data(source.as_str(), mesh.clone());
                        let _ = self
                            .resources
                            .set_runtime_mesh_data_by_id(out_id, mesh.clone());
                        self.events.push(RenderEvent::MeshCreated {
                            request,
                            id: out_id,
                            mesh: Some(mesh),
                        });
                    }
                    ResourceCommand::WriteMeshData { id, mesh } => {
                        let _ = self.resources.set_runtime_mesh_data_by_id(id, mesh);
                    }
                    ResourceCommand::CreateTexture {
                        request,
                        id,
                        source,
                        reserved,
                    } => {
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        if self.resources.decoded_texture_data(id).is_none() {
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                let waiters =
                                    self.pending_async_texture_loads.entry(id).or_default();
                                let start_load = waiters.is_empty();
                                if !waiters.contains(&request) {
                                    waiters.push(request);
                                }
                                if start_load {
                                    self.start_async_texture_load(id, source);
                                }
                            }
                            #[cfg(target_arch = "wasm32")]
                            self.start_async_texture_load(request, id, source);
                        } else {
                            self.events
                                .push(RenderEvent::TextureCreated { request, id });
                        }
                    }
                    ResourceCommand::CreateRuntimeTexture {
                        request,
                        id,
                        source,
                        reserved,
                        width,
                        height,
                        rgba,
                    } => {
                        let expected_len = (width as usize)
                            .checked_mul(height as usize)
                            .and_then(|pixels| pixels.checked_mul(4));
                        if width == 0 || height == 0 || expected_len != Some(rgba.len()) {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!(
                                    "invalid rgba texture {width}x{height} len={}",
                                    rgba.len()
                                ),
                            });
                            continue;
                        }
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let _ = self.resources.set_decoded_texture_data(
                            id,
                            DecodedTextureRgba {
                                rgba: rgba.to_vec(),
                                width,
                                height,
                            },
                        );
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::CreateRuntimeTextureBytes {
                        request,
                        id,
                        source,
                        reserved,
                        bytes,
                    } => {
                        let decoded = decode_ptex(bytes.as_ref())
                            .or_else(|| decode_image_rgba(bytes.as_ref()))
                            .map(|(rgba, width, height)| DecodedTextureRgba {
                                rgba,
                                width,
                                height,
                            });
                        let Some(decoded) = decoded else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!("invalid texture bytes len={}", bytes.len()),
                            });
                            continue;
                        };
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let _ = self.resources.set_decoded_texture_data(id, decoded);
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::CreateExternalTexture {
                        request,
                        id,
                        source,
                        reserved,
                        width,
                        height,
                    } => {
                        let Some(len) = checked_runtime_texture_rgba_len(width, height) else {
                            self.events.push(RenderEvent::Failed {
                                request,
                                reason: format!(
                                    "external texture size {width}x{height} exceeds runtime limits"
                                ),
                            });
                            continue;
                        };
                        let id = if id.is_nil() {
                            self.resources.create_texture(source.as_str(), reserved)
                        } else {
                            self.resources
                                .create_texture_with_id(id, source.as_str(), reserved)
                        };
                        let mut rgba = vec![0; len];
                        for pixel in rgba.chunks_exact_mut(4) {
                            pixel[3] = 255;
                        }
                        let _ = self.resources.set_decoded_texture_data(
                            id,
                            DecodedTextureRgba {
                                rgba,
                                width,
                                height,
                            },
                        );
                        self.events
                            .push(RenderEvent::TextureCreated { request, id });
                        self.events.push(RenderEvent::TextureLoaded { id });
                    }
                    ResourceCommand::WriteTextureRgba {
                        id,
                        width,
                        height,
                        rgba,
                    } => {
                        if checked_runtime_texture_rgba_len(width, height) != Some(rgba.len()) {
                            continue;
                        }
                        // keep resident CPU copy current in place (reuses buffer
                        // when same size; drops the redundant by_source dup).
                        let has_texture = self
                            .resources
                            .write_stream_texture_data(id, &rgba, width, height);
                        if !has_texture {
                            continue;
                        }
                        let texture_source = self.resources.texture_source(id).map(str::to_owned);
                        let same_size =
                            self.stream_texture_dims.get(&id).copied() == Some([width, height]);
                        if same_size {
                            // repeat frame: update texels in place. no GPU rebuild,
                            // no retained re-stage, no full scene rescan.
                            if let Some(gpu) = self.gpu.as_mut() {
                                gpu.write_stream_texture(
                                    id,
                                    texture_source.as_deref(),
                                    width,
                                    height,
                                    &rgba,
                                );
                            }
                            self.events.push(RenderEvent::TextureTexelsUpdated { id });
                        } else {
                            // first frame or resolution change: reload path. mark
                            // as a stream so the rebuild is single-level, drop the
                            // stale cache, and re-scan so pending refs resolve.
                            self.stream_texture_dims.insert(id, [width, height]);
                            if let Some(gpu) = self.gpu.as_mut() {
                                gpu.set_stream_texture(id, true);
                                gpu.invalidate_texture(id, texture_source.as_deref());
                            }
                            self.retained_draws_cache_revision = u64::MAX;
                            self.retained_decals_3d_cache_revision = u64::MAX;
                            self.retained_sprites_cache_revision = u64::MAX;
                            self.events.push(RenderEvent::TextureLoaded { id });
                        }
                        self.redraw_requested = true;
                    }
                    ResourceCommand::WriteTextureRgbaRegion {
                        id,
                        x,
                        y,
                        width,
                        height,
                        rgba,
                    } => {
                        if self.resources.write_decoded_texture_region(
                            id,
                            x,
                            y,
                            width,
                            height,
                            rgba.as_ref(),
                        ) {
                            let texture_source =
                                self.resources.texture_source(id).map(str::to_owned);
                            if let Some(gpu) = self.gpu.as_mut() {
                                gpu.invalidate_texture(id, texture_source.as_deref());
                            }
                            self.retained_draws_cache_revision = u64::MAX;
                            self.retained_decals_3d_cache_revision = u64::MAX;
                            self.retained_sprites_cache_revision = u64::MAX;
                            self.events.push(RenderEvent::TextureLoaded { id });
                            self.redraw_requested = true;
                        }
                    }
                    ResourceCommand::CreateMaterial {
                        request,
                        id,
                        material,
                        source,
                        reserved,
                    } => {
                        let log_kind = if asset_ready_log_enabled() {
                            Some(match source.as_deref() {
                                Some(path) => format!("kind=source path={path}"),
                                None if material == Material3D::default() => {
                                    "kind=default".to_string()
                                }
                                None => "kind=inline".to_string(),
                            })
                        } else {
                            None
                        };
                        let id = if id.is_nil() {
                            self.resources
                                .create_material(material, source.as_deref(), reserved)
                        } else {
                            self.resources.create_material_with_id(
                                id,
                                material,
                                source.as_deref(),
                                reserved,
                            )
                        };
                        self.events
                            .push(RenderEvent::MaterialCreated { request, id });
                        if let Some(log_kind) = log_kind {
                            eprintln!(
                                "[perro][asset-ready] backend material created id={id:?} {log_kind}"
                            );
                        }
                        self.events.push(RenderEvent::MaterialLoaded { id });
                    }
                    ResourceCommand::SetSceneResourceRefs {
                        textures,
                        meshes,
                        materials,
                    } => {
                        self.scene_texture_refs_cache.clear();
                        self.scene_texture_refs_cache.extend(
                            textures
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                        self.scene_mesh_refs_cache.clear();
                        self.scene_mesh_refs_cache.extend(
                            meshes
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                        self.scene_material_refs_cache.clear();
                        self.scene_material_refs_cache.extend(
                            materials
                                .into_iter()
                                .filter(|(id, nodes)| !id.is_nil() && !nodes.is_empty()),
                        );
                    }
                    ResourceCommand::WriteMaterialData { id, material } => {
                        if self.resources.set_material_data(id, material) {
                            if asset_ready_log_enabled() {
                                eprintln!(
                                    "[perro][asset-ready] backend material data applied id={id:?}"
                                );
                            }
                            self.events.push(RenderEvent::MaterialLoaded { id });
                        }
                    }
                    ResourceCommand::SetMeshReserved { id, reserved } => {
                        self.resources.set_mesh_reserved(id, reserved);
                    }
                    ResourceCommand::SetTextureReserved { id, reserved } => {
                        self.resources.set_texture_reserved(id, reserved);
                    }
                    ResourceCommand::SetMaterialReserved { id, reserved } => {
                        self.resources.set_material_reserved(id, reserved);
                    }
                    ResourceCommand::DropMesh { id } => {
                        if self.resources.drop_mesh(id) {
                            self.events.push(RenderEvent::MeshDropped { id });
                        }
                    }
                    ResourceCommand::DropTexture { id } => {
                        if self.stream_texture_dims.remove(&id).is_some()
                            && let Some(gpu) = self.gpu.as_mut()
                        {
                            gpu.set_stream_texture(id, false);
                        }
                        if self.resources.drop_texture(id) {
                            self.events.push(RenderEvent::TextureDropped { id });
                        }
                    }
                    ResourceCommand::DropMaterial { id } => {
                        if self.resources.drop_material(id) {
                            self.events.push(RenderEvent::MaterialDropped { id });
                        }
                    }
                },
                RenderCommand::TwoD(cmd_2d) => match cmd_2d {
                    Command2D::UpsertCameraStream {
                        node,
                        stream,
                        sprite,
                    } => {
                        let stream = *stream;
                        if camera_stream_uses_render_target(&stream) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.renderer_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertSprite { node, sprite } => {
                        self.renderer_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertTileMap { node, tilemap } => {
                        self.renderer_2d.upsert_tilemap(node, tilemap);
                    }
                    Command2D::UpsertRect { node, rect } => {
                        self.renderer_2d.queue_rect(node, rect);
                    }
                    Command2D::UpsertPointParticles { node, particles } => {
                        self.renderer_2d.queue_point_particles(node, *particles);
                    }
                    Command2D::UpsertWater { node, water } => {
                        self.renderer_2d.upsert_water(node, *water);
                    }
                    Command2D::UpsertShadowCaster { node, caster } => {
                        self.renderer_2d.upsert_shadow_caster(node, caster);
                    }
                    Command2D::SetAmbientLight { node, light } => {
                        self.renderer_2d.set_ambient_light(node, light);
                    }
                    Command2D::SetRayLight { node, light } => {
                        self.renderer_2d.set_ray_light(node, light);
                    }
                    Command2D::SetPointLight { node, light } => {
                        self.renderer_2d.set_point_light(node, light);
                    }
                    Command2D::SetSpotLight { node, light } => {
                        self.renderer_2d.set_spot_light(node, light);
                    }
                    Command2D::RemoveNode { node } => {
                        self.renderer_2d.remove_node(node);
                    }
                    Command2D::SetCamera { camera } => {
                        self.renderer_2d.set_camera(camera);
                    }
                    Command2D::DrawShape { draw } => {
                        self.renderer_2d.queue_shape(draw);
                    }
                },
                RenderCommand::ThreeD(cmd_3d) => match *cmd_3d {
                    Command3D::UpsertCameraStream { node, stream, quad } => {
                        let stream = *stream;
                        if camera_stream_uses_render_target(&stream) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.renderer_3d.queue_camera_stream_quad(
                            node,
                            stream.output_texture,
                            quad.model,
                            quad.size,
                            quad.tint.to_float_slice(),
                        );
                    }
                    Command3D::Draw {
                        mesh,
                        surfaces,
                        node,
                        model,
                        skeleton,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw(
                            node,
                            mesh,
                            surfaces,
                            model,
                            skeleton,
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawMulti {
                        mesh,
                        surfaces,
                        node,
                        instance_mats,
                        skeleton,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw_multi(
                            node,
                            mesh,
                            surfaces,
                            instance_mats,
                            skeleton,
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawMultiDense {
                        mesh,
                        surfaces,
                        node,
                        node_model,
                        instance_scale,
                        instances,
                        blend_shape_weights,
                        meshlet_override,
                        lod,
                        blend,
                        cast_shadows,
                        receive_shadows,
                        ..
                    } => {
                        self.renderer_3d.queue_draw_multi_dense(
                            node,
                            mesh,
                            surfaces,
                            crate::three_d::renderer::DenseMultiMeshDraw3D {
                                node_model,
                                instance_scale,
                                instances,
                            },
                            blend_shape_weights,
                            meshlet_override,
                            lod,
                            blend,
                            cast_shadows,
                            receive_shadows,
                        );
                    }
                    Command3D::DrawDebugPoint3D {
                        node,
                        position,
                        size,
                        color,
                    } => {
                        self.renderer_3d
                            .queue_debug_point(node, position, size, color);
                    }
                    Command3D::DrawDebugLine3D {
                        node,
                        start,
                        end,
                        thickness,
                        color,
                    } => {
                        self.renderer_3d
                            .queue_debug_line(node, start, end, thickness, color);
                    }
                    Command3D::SetCamera { camera } => {
                        self.renderer_3d.set_camera(camera);
                    }
                    Command3D::SetAmbientLight { node, light } => {
                        self.renderer_3d.set_ambient_light(node, light);
                    }
                    Command3D::SetSky { node, sky } => {
                        self.renderer_3d.set_sky(node, *sky);
                    }
                    Command3D::SetRayLight { node, light } => {
                        self.renderer_3d.set_ray_light(node, light);
                    }
                    Command3D::SetPointLight { node, light } => {
                        self.renderer_3d.set_point_light(node, light);
                    }
                    Command3D::SetSpotLight { node, light } => {
                        self.renderer_3d.set_spot_light(node, light);
                    }
                    Command3D::SetDecal { node, decal } => {
                        self.renderer_3d.set_decal(node, *decal);
                    }
                    Command3D::UpsertPointParticles { node, particles } => {
                        self.particles_3d.queue_point_particles(node, *particles);
                    }
                    Command3D::UpsertWater { node, water } => {
                        self.renderer_3d.upsert_water(node, *water);
                    }
                    Command3D::RemoveNode { node } => {
                        self.renderer_3d.remove_node(node);
                        self.particles_3d.remove_node(node);
                    }
                },
                RenderCommand::Ui(cmd) => {
                    self.renderer_ui.submit(cmd);
                }
                RenderCommand::VisualAccessibility(command) => match command {
                    VisualAccessibilityCommand::EnableColorBlind { mode, strength } => {
                        self.accessibility.color_blind =
                            Some(perro_structs::ColorBlindSetting::new(mode, strength));
                    }
                    VisualAccessibilityCommand::DisableColorBlind => {
                        self.accessibility.color_blind = None;
                    }
                },
                RenderCommand::PostProcessing(command) => {
                    match command {
                        PostProcessingCommand::SetGlobal(set) => {
                            self.global_post_processing = set;
                        }
                        PostProcessingCommand::AddGlobalNamed { name, effect } => {
                            self.global_post_processing.add(name, effect);
                        }
                        PostProcessingCommand::AddGlobalUnnamed(effect) => {
                            self.global_post_processing.add_unnamed(effect);
                        }
                        PostProcessingCommand::RemoveGlobalByName(name) => {
                            self.global_post_processing.remove(name.as_ref());
                        }
                        PostProcessingCommand::RemoveGlobalByIndex(index) => {
                            self.global_post_processing.remove_index(index);
                        }
                        PostProcessingCommand::ClearGlobal => {
                            self.global_post_processing.clear();
                        }
                    }
                    // Any global post-processing edit invalidates the cached
                    // effects Arc rebuilt in `render`.
                    self.global_post_processing_cache_dirty = true;
                }
                RenderCommand::Display(DisplayCommand::SetHdrMode(mode)) => {
                    self.hdr_mode = mode;
                    let status = self.gpu.as_mut().map_or_else(
                        || perro_render_bridge::HdrStatus {
                            requested: mode,
                            ..Default::default()
                        },
                        |gpu| gpu.set_hdr_mode(mode),
                    );
                    self.events.push(RenderEvent::HdrStatusChanged(status));
                    self.redraw_requested = true;
                }
            }
        }
        self.flush_async_mesh_loads();
        self.flush_async_texture_loads();
        self.poll_async_mesh_loads();
        self.poll_async_texture_loads();
    }

    pub(super) fn upsert_camera_stream_texture(
        &mut self,
        node: NodeID,
        texture: TextureID,
        resolution: [u32; 2],
    ) {
        let [width, height] = resolution;
        let Some(len) = checked_runtime_texture_rgba_len(width, height) else {
            return;
        };
        let texture = if texture.is_nil() {
            camera_stream_texture_id(node)
        } else {
            texture
        };
        let source = format!("__camera_stream__:{}", node.as_u64());
        let id = self
            .resources
            .create_texture_with_id(texture, &source, true);
        let resolution = [width, height];
        if self.camera_stream_targets.get(&node).copied() == Some(resolution) {
            return;
        }
        self.camera_stream_targets.insert(node, resolution);
        let _ = self.resources.set_decoded_texture_data(
            id,
            DecodedTextureRgba {
                rgba: vec![0; len],
                width,
                height,
            },
        );
    }

    pub(super) fn process_late_overlay_commands<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = RenderCommand>,
    {
        for command in commands {
            match command {
                RenderCommand::Resource(resource_cmd) => {
                    self.process_commands(std::iter::once(RenderCommand::Resource(resource_cmd)));
                }
                RenderCommand::TwoD(cmd_2d) => match cmd_2d {
                    Command2D::UpsertCameraStream {
                        node,
                        stream,
                        sprite,
                    } => {
                        let stream = *stream;
                        if camera_stream_uses_render_target(&stream) {
                            self.upsert_camera_stream_texture(
                                node,
                                stream.output_texture,
                                stream.resolution,
                            );
                        }
                        self.late_overlay_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertSprite { node, sprite } => {
                        self.late_overlay_2d.queue_sprite(node, sprite);
                    }
                    Command2D::UpsertTileMap { node, tilemap } => {
                        self.late_overlay_2d.upsert_tilemap(node, tilemap);
                    }
                    Command2D::UpsertRect { node, rect } => {
                        self.late_overlay_2d.queue_rect(node, rect);
                    }
                    Command2D::UpsertPointParticles { node, particles } => {
                        self.late_overlay_2d.queue_point_particles(node, *particles);
                    }
                    Command2D::UpsertWater { node, water } => {
                        self.late_overlay_2d.upsert_water(node, *water);
                    }
                    Command2D::UpsertShadowCaster { node, caster } => {
                        self.late_overlay_2d.upsert_shadow_caster(node, caster);
                    }
                    Command2D::SetAmbientLight { node, light } => {
                        self.late_overlay_2d.set_ambient_light(node, light);
                    }
                    Command2D::SetRayLight { node, light } => {
                        self.late_overlay_2d.set_ray_light(node, light);
                    }
                    Command2D::SetPointLight { node, light } => {
                        self.late_overlay_2d.set_point_light(node, light);
                    }
                    Command2D::SetSpotLight { node, light } => {
                        self.late_overlay_2d.set_spot_light(node, light);
                    }
                    Command2D::RemoveNode { node } => {
                        self.late_overlay_2d.remove_node(node);
                    }
                    Command2D::SetCamera { camera } => {
                        self.late_overlay_2d.set_camera(camera);
                    }
                    Command2D::DrawShape { draw } => {
                        self.late_overlay_2d.queue_shape(draw);
                    }
                },
                _ => self.process_commands(std::iter::once(command)),
            }
        }
    }
}
