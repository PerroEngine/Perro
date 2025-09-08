use once_cell::sync::Lazy;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Thin,
    Light,
    Regular,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    Normal,
    Italic,
}

pub struct Font {
    pub data: &'static [u8],
}

type FontKey = (Weight, Style);

// Mapping from (Weight, Style) -> font bytes for NotoSans
static NOTO_SANS: Lazy<HashMap<FontKey, &'static [u8]>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert((Weight::Thin, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-Thin.ttf") as &[u8]);
    m.insert((Weight::Thin, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-ThinItalic.ttf"));
    m.insert((Weight::Light, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-Light.ttf"));
    m.insert((Weight::Light, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-LightItalic.ttf"));
    m.insert((Weight::Regular, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-Regular.ttf"));
    m.insert((Weight::Regular, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-Italic.ttf"));
    m.insert((Weight::Medium, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-Medium.ttf"));
    m.insert((Weight::Medium, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-MediumItalic.ttf"));
    m.insert((Weight::SemiBold, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-SemiBold.ttf"));
    m.insert((Weight::SemiBold, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-SemiBoldItalic.ttf"));
    m.insert((Weight::Bold, Style::Normal), include_bytes!("fonts/NotoSans/NotoSans-Bold.ttf"));
    m.insert((Weight::Bold, Style::Italic), include_bytes!("fonts/NotoSans/NotoSans-BoldItalic.ttf"));
    m
});

impl Font {
    pub fn from_name(family: &str, weight: Weight, style: Style) -> Option<Self> {
        match family {
            "NotoSans" => NOTO_SANS.get(&(weight, style)).map(|&data| Font { data }),
            _ => None,
        }
    }
}

// Glyph information in the atlas
#[derive(Debug, Clone)]
pub struct Glyph {
    pub x: u32,          // Position in atlas
    pub y: u32,
    pub width: u32,      // Size in atlas
    pub height: u32,
    pub x_offset: i32,   // Bearing/offset when rendering
    pub y_offset: i32,
    pub advance: f32,    // How much to advance cursor
}

// Font atlas containing SDF texture data
#[derive(Debug)]
pub struct FontAtlas {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,                    // SDF data (single channel)
    pub glyphs: HashMap<char, Glyph>,       // Character -> glyph mapping
    pub line_height: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl FontAtlas {
    pub fn new(font: Font, atlas_size: f32) -> Self {
        use ab_glyph::{FontRef, PxScale, ScaleFont, Font as AbFont, Glyph as AbGlyph};
        
        let font_ref = FontRef::try_from_slice(font.data)
            .expect("Invalid font data");
        
        let scale = PxScale::from(atlas_size);
        let scaled_font = font_ref.as_scaled(scale);
        
        // Common ASCII characters to include in atlas
        let chars: Vec<char> = (32..=126).map(|i| i as u8 as char).collect();
        
        let mut glyphs = HashMap::new();
        let mut positioned_glyphs = Vec::new();
        
        // Calculate glyph positions and metrics
        for ch in &chars {
            let glyph_id = font_ref.glyph_id(*ch);
            let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(0.0, 0.0));
            
            if let Some(outlined) = scaled_font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                positioned_glyphs.push((*ch, outlined, bounds));
            }
        }
        
        // Simple atlas packing - arrange glyphs in rows
        let atlas_width = 512u32;
        let atlas_height = 512u32;
        let padding = 2u32;
        
        let mut x = padding;
        let mut y = padding;
        let mut row_height = 0u32;
        
        for (ch, outlined_glyph, bounds) in &positioned_glyphs {
            let width = bounds.width().ceil() as u32;
            let height = bounds.height().ceil() as u32;
            
            // Check if we need to move to next row
            if x + width + padding > atlas_width {
                x = padding;
                y += row_height + padding;
                row_height = 0;
            }
            
            // Skip if we're out of vertical space
            if y + height + padding > atlas_height {
                break;
            }
            
            let h_metrics = scaled_font.h_advance(outlined_glyph.glyph().id);
            
            glyphs.insert(*ch, Glyph {
                x,
                y,
                width,
                height,
                x_offset: bounds.min.x.floor() as i32,
                y_offset: bounds.min.y.floor() as i32,
                advance: h_metrics,
            });
            
            x += width + padding;
            row_height = row_height.max(height);
        }
        
        // Create atlas bitmap with SDF generation
        let mut bitmap = vec![0u8; (atlas_width * atlas_height) as usize];
        
        for (ch, outlined_glyph, bounds) in positioned_glyphs {
            if let Some(glyph_info) = glyphs.get(&ch) {
                // Simple rasterization (not true SDF, but works for basic text)
                // For true SDF, you'd want to use a library like msdfgen
                let mut glyph_bitmap = vec![0f32; (glyph_info.width * glyph_info.height) as usize];
                
                outlined_glyph.draw(|gx, gy, coverage| {
                    let idx = (gy * glyph_info.width + gx) as usize;
                    if idx < glyph_bitmap.len() {
                        glyph_bitmap[idx] = coverage;
                    }
                });
                
                // Copy glyph to atlas
                for gy in 0..glyph_info.height {
                    for gx in 0..glyph_info.width {
                        let src_idx = (gy * glyph_info.width + gx) as usize;
                        let dst_x = glyph_info.x + gx;
                        let dst_y = glyph_info.y + gy;
                        let dst_idx = (dst_y * atlas_width + dst_x) as usize;
                        
                        if src_idx < glyph_bitmap.len() && dst_idx < bitmap.len() {
                            bitmap[dst_idx] = (glyph_bitmap[src_idx] * 255.0) as u8;
                        }
                    }
                }
            }
        }
        
        let v_metrics = scaled_font.height() + scaled_font.descent();
        
        FontAtlas {
            width: atlas_width,
            height: atlas_height,
            bitmap,
            glyphs,
            line_height: scaled_font.height(),
            ascent: scaled_font.ascent(),
            descent: scaled_font.descent(),
        }
    }
    
    pub fn get_glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }
    
    pub fn measure_text(&self, text: &str, font_size: f32) -> (f32, f32) {
        let scale = font_size / self.line_height;
        let mut width = 0.0;
        
        for ch in text.chars() {
            if let Some(glyph) = self.get_glyph(ch) {
                width += glyph.advance * scale;
            }
        }
        
        (width, self.line_height * scale)
    }
}