use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, Device, Extent3d, FilterMode,
    Queue, Sampler, SamplerDescriptor, ShaderStages, TextureDescriptor, TextureDimension,
    TextureFormat, TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor,
};

/// Create the standard texture bind group layout
/// This should be created once per device and reused for all textures
pub fn create_texture_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("texture_bind_group_layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Calculate the required alignment for texture data rows (WGPU requirement)
#[inline]
fn align_to(value: u32, alignment: u32) -> u32 {
    (value + alignment - 1) & !(alignment - 1)
}

/// Calculate bytes per row with proper alignment for GPU upload
#[inline]
fn calculate_bytes_per_row(width: u32) -> u32 {
    // WGPU requires row pitch to be a multiple of 256 bytes
    let bytes_per_pixel = 4u32; // RGBA8
    let unaligned_bytes_per_row = width * bytes_per_pixel;
    align_to(unaligned_bytes_per_row, 256)
}

/// Pre-decoded texture data for static assets (compile-time decoded RGBA8 bytes)
#[derive(Debug, Clone)]
pub struct StaticTextureData {
    pub width: u32,
    pub height: u32,
    pub rgba8_bytes: &'static [u8],
}

impl StaticTextureData {
    /// Create ImageTexture from pre-decoded data at runtime
    pub fn to_image_texture(&self, device: &Device, queue: &Queue) -> ImageTexture {
        let texture_size = Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("StaticImageTexture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // OPTIMIZED: Use aligned bytes per row for better GPU performance
        let bytes_per_row = calculate_bytes_per_row(self.width);
        let actual_bytes_per_row = 4 * self.width;

        // If alignment is needed, copy data to aligned buffer
        if bytes_per_row != actual_bytes_per_row {
            let total_size = (bytes_per_row * self.height) as usize;
            let mut aligned_data = vec![0u8; total_size];

            // Copy rows with proper alignment
            for row in 0..self.height {
                let src_offset = (row * actual_bytes_per_row) as usize;
                let dst_offset = (row * bytes_per_row) as usize;
                let src_end = src_offset + actual_bytes_per_row as usize;
                aligned_data[dst_offset..dst_offset + actual_bytes_per_row as usize]
                    .copy_from_slice(&self.rgba8_bytes[src_offset..src_end]);
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &aligned_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
                texture_size,
            );
        } else {
            // No alignment needed, use data directly
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.rgba8_bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(actual_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
                texture_size,
            );
        }

        let view = texture.create_view(&TextureViewDescriptor::default());

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("StaticImageTextureSampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest, // Use nearest for pixel-perfect sprites without filtering artifacts
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // OPTIMIZED: Use shared bind group layout (should be cached by caller)
        let bind_group_layout = create_texture_bind_group_layout(device);

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("texture_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        ImageTexture {
            view,
            bind_group,
            width: self.width,
            height: self.height,
            sampler,
        }
    }
}

#[derive(Debug)]
pub struct ImageTexture {
    pub view: TextureView,
    pub bind_group: BindGroup,
    pub width: u32,
    pub height: u32,
    pub(crate) sampler: Sampler,
}

impl ImageTexture {
    /// Create ImageTexture directly from RGBA8 data (faster, avoids intermediate conversions)
    pub fn from_rgba8(rgba: &image::RgbaImage, device: &Device, queue: &Queue) -> Self {
        let (width, height) = rgba.dimensions();
        Self::from_rgba8_bytes(rgba.as_raw(), width, height, device, queue)
    }

    /// Create ImageTexture from raw RGBA8 bytes (most efficient)
    pub fn from_rgba8_bytes(
        rgba_bytes: &[u8],
        width: u32,
        height: u32,
        device: &Device,
        queue: &Queue,
    ) -> Self {
        // Describe the texture
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&TextureDescriptor {
            label: Some("ImageTexture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb, // typical format for sRGB images
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // OPTIMIZED: Use aligned bytes per row for better GPU performance
        let bytes_per_row = calculate_bytes_per_row(width);
        let actual_bytes_per_row = 4 * width;

        // If alignment is needed, copy data to aligned buffer
        if bytes_per_row != actual_bytes_per_row {
            let total_size = (bytes_per_row * height) as usize;
            let mut aligned_data = vec![0u8; total_size];

            // Copy rows with proper alignment
            for row in 0..height {
                let src_offset = (row * actual_bytes_per_row) as usize;
                let dst_offset = (row * bytes_per_row) as usize;
                let src_end = src_offset + actual_bytes_per_row as usize;
                if src_end <= rgba_bytes.len() {
                    aligned_data[dst_offset..dst_offset + actual_bytes_per_row as usize]
                        .copy_from_slice(&rgba_bytes[src_offset..src_end]);
                }
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &aligned_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
                texture_size,
            );
        } else {
            // No alignment needed, use data directly (zero-copy path)
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba_bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(actual_bytes_per_row),
                    rows_per_image: Some(height),
                },
                texture_size,
            );
        }

        // Create texture view
        let view = texture.create_view(&TextureViewDescriptor::default());

        // Create sampler for the texture
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("ImageTextureSampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest, // Use nearest for pixel-perfect sprites without filtering artifacts
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // OPTIMIZED: Use shared bind group layout (should be cached by caller)
        let bind_group_layout = create_texture_bind_group_layout(device);

        // Create bind group for texture + sampler
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("texture_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            view,
            bind_group,
            width,
            height,
            sampler,
        }
    }

    pub fn from_image(img: &image::DynamicImage, device: &Device, queue: &Queue) -> Self {
        // Convert image to RGBA8 format and get dimensions
        let rgba = img.to_rgba8();
        Self::from_rgba8(&rgba, device, queue)
    }
}
