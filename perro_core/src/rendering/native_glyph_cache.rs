//! Native glyph cache using swash for rendering and cosmic-text for layout
//! Completely replaces the old FontAtlas system

use rustc_hash::FxHashMap;
use cosmic_text::FontSystem;
use swash::{scale::*, FontRef};
use wgpu::{Device, Queue, Texture, TextureView, TextureFormat, TextureUsages, TextureDescriptor, Extent3d, TextureDimension, TextureViewDescriptor, BindGroup, BindGroupLayout, BindGroupDescriptor, BindGroupEntry, BindingResource, Sampler, SamplerDescriptor, AddressMode, FilterMode};

/// Native glyph cache entry
#[derive(Debug, Clone)]
pub struct CachedGlyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub width: f32,
    pub height: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub advance: f32,
}

/// Native glyph atlas using swash rendering
pub struct NativeGlyphAtlas {
    // Texture atlas
    pub texture: Texture,
    pub view: TextureView,
    pub sampler: Sampler,
    pub bind_group: BindGroup,
    pub width: u32,
    pub height: u32,
    
    // Glyph cache: (font_id, glyph_id, size) -> CachedGlyph
    // Use String key since tuple doesn't implement Hash for fontdb::ID
    glyph_cache: FxHashMap<String, CachedGlyph>,
    
    // Atlas packing state
    pen_x: u32,
    pen_y: u32,
    row_height: u32,
    
    // Swash scale context for rendering
    scale_context: ScaleContext,
}

impl NativeGlyphAtlas {
    pub fn new(device: &Device, width: u32, height: u32, bind_group_layout: &BindGroupLayout, queue: &Queue) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Native Glyph Atlas"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Unorm, // R8Unorm is compatible with texture_2d<f32> in shader
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        
        // Initialize texture with black (transparent) background
        // This ensures unused areas are transparent
        let clear_data = vec![0u8; (width * height) as usize];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            clear_data.as_slice(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        
        let view = texture.create_view(&TextureViewDescriptor::default());
        
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Native Glyph Atlas Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });
        
        // Use the provided bind group layout (must match the font pipeline)
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Native Glyph Atlas Bind Group"),
            layout: bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&sampler),
                },
            ],
        });
        
        Self {
            texture,
            view,
            sampler,
            bind_group,
            width,
            height,
            glyph_cache: FxHashMap::default(),
            pen_x: 2,
            pen_y: 2,
            row_height: 0,
            scale_context: ScaleContext::new(),
        }
    }
    
    /// Get or render a glyph using swash
    pub fn get_glyph(
        &mut self,
        font_system: &mut FontSystem,
        font_id: cosmic_text::fontdb::ID,
        glyph_id: u16,
        char_code: Option<char>,
        size: f32,
        device: &Device,
        queue: &Queue,
    ) -> Option<&CachedGlyph> {
        // Create a unique key for this glyph
        // Use a hash of the font_id since we can't access its internals
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        font_id.hash(&mut hasher);
        let key = format!("font_{}_{}_{}", hasher.finish(), glyph_id, size);
        
        // Check cache first
        if self.glyph_cache.contains_key(&key) {
            return self.glyph_cache.get(&key);
        }
        
        // Get font data from font system
        let font_data = font_system.db().with_face_data(font_id, |data, index| {
            (data.to_vec(), index)
        })?;
        
        // Create font reference for swash
        let font_ref = FontRef::from_index(font_data.0.as_slice(), font_data.1 as usize)?;
        
        // For now, use fontdue as a temporary fallback until swash API is properly integrated
        // This provides native rendering while we work on full swash integration
        use fontdue::Font as Fontdue;
        use fontdue::FontSettings;
        
        let fontdue_font = Fontdue::from_bytes(font_data.0.as_slice(), FontSettings::default()).ok()?;
        
        // Try to get character code - if not available, try using glyph_id as char code (fallback)
        // This is a workaround - ideally we'd get the actual character from cosmic-text
        let char_code = if let Some(ch) = char_code {
            ch
        } else {
            // Fallback: try to convert glyph_id to char (may fail for complex glyphs)
            std::char::from_u32(glyph_id as u32).unwrap_or('?')
        };
        
        let (fontdue_metrics, bitmap_data) = fontdue_font.rasterize(char_code, size);
        
        let glyph_width = fontdue_metrics.width as u32;
        let glyph_height = fontdue_metrics.height as u32;
        let advance = fontdue_metrics.advance_width as f32;
        
        if glyph_width > 0 && glyph_height > 0 {
            // Pack into atlas
            if self.pen_x + glyph_width + 2 > self.width {
                self.pen_x = 2;
                self.pen_y += self.row_height + 2;
                self.row_height = 0;
            }
            
            if self.pen_y + glyph_height + 2 > self.height {
                // Atlas full - would need to resize or evict
                // For now, return None
                return None;
            }
            
            // Apply gamma encoding to match what the shader expects
            // The shader decodes with pow(alpha, 1.8), so we encode with pow(alpha, 1/1.8)
            let gamma_encoded: Vec<u8> = bitmap_data.iter()
                .map(|&alpha| {
                    let normalized = alpha as f32 / 255.0;
                    let encoded = normalized.powf(1.0 / 1.8);
                    (encoded * 255.0).clamp(0.0, 255.0) as u8
                })
                .collect();
            
            // Upload glyph bitmap to atlas (with gamma encoding)
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: self.pen_x,
                        y: self.pen_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                gamma_encoded.as_slice(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(glyph_width),
                    rows_per_image: Some(glyph_height),
                },
                Extent3d {
                    width: glyph_width,
                    height: glyph_height,
                    depth_or_array_layers: 1,
                },
            );
            
            // Calculate UV coordinates
            let u0 = self.pen_x as f32 / self.width as f32;
            let v0 = self.pen_y as f32 / self.height as f32;
            let u1 = (self.pen_x + glyph_width) as f32 / self.width as f32;
            let v1 = (self.pen_y + glyph_height) as f32 / self.height as f32;
            
            let cached = CachedGlyph {
                u0,
                v0,
                u1,
                v1,
                width: glyph_width as f32,
                height: glyph_height as f32,
                bearing_x: fontdue_metrics.xmin as f32,
                bearing_y: fontdue_metrics.ymin as f32,
                advance,
            };
            
            self.glyph_cache.insert(key.clone(), cached);
            self.pen_x += glyph_width + 2;
            self.row_height = self.row_height.max(glyph_height);
            
            return self.glyph_cache.get(&key);
        }
        
        None
    }
}
