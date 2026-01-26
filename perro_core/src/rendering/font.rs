use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Font family/style helpers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Bold,
    Light,
    Medium,
    SemiBold,
    Thin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    Normal,
    Italic,
}

/// Font source - either embedded static data or loaded from file/system
#[derive(Debug, Clone)]
pub enum FontData {
    Static(&'static [u8]),
    Owned(Vec<u8>),
}

/// Font representation that can be from embedded, file, or system fonts
#[derive(Debug, Clone)]
pub struct Font {
    pub data: FontData,
    pub family_name: Option<String>, // Font family name if known
    pub weight: Weight,
    pub style: Style,
}

// Embedded fonts removed - we now use native system fonts via cosmic-text

// Font database removed - egui handles font loading natively

impl Font {
    /// Create font from embedded name (deprecated - use system fonts instead)
    /// This method is kept for backward compatibility but always returns None
    /// since embedded fonts have been removed in favor of native system fonts
    #[deprecated(note = "Embedded fonts removed. Use system fonts via TextRenderer instead.")]
    pub fn from_name(_family: &str, _weight: Weight, _style: Style) -> Option<Self> {
        None
    }
    
    /// Load font from file path (supports res:// paths)
    pub fn from_file(path: &str, weight: Weight, style: Style) -> Result<Self, String> {
        // Handle res:// paths
        let file_path = if path.starts_with("res://") {
            &path[6..] // Remove "res://" prefix
        } else {
            path
        };
        
        let font_data = std::fs::read(file_path)
            .map_err(|e| format!("Failed to read font file {}: {}", path, e))?;
        
        Ok(Font {
            data: FontData::Owned(font_data),
            family_name: None, // Will be extracted from font if needed
            weight,
            style,
        })
    }
    
    /// Load font from system by family name
    /// Note: Font loading now handled by egui - this is a stub for compatibility
    pub fn from_system(_family: &str, _weight: Weight, _style: Style) -> Option<Self> {
        // Font loading now handled by egui
        None
    }
    
    /// Get font data as bytes
    pub fn data(&self) -> &[u8] {
        match &self.data {
            FontData::Static(data) => data,
            FontData::Owned(data) => data,
        }
    }
    
    /// Try to load font from various sources (file or system)
    /// Note: This is deprecated in favor of the native TextRenderer system
    pub fn load(font_spec: &str, weight: Weight, style: Style) -> Option<Self> {
        // Try file path first (res:// or regular path)
        if font_spec.starts_with("res://") || font_spec.contains('.') {
            return Self::from_file(font_spec, weight, style).ok();
        }
        
        // Try system font
        Self::from_system(font_spec, weight, style)
    }
}

/// Per-glyph atlas data
#[derive(Debug, Clone)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub metrics: (f32, f32, f32, f32), // width, height, advance_width, advance_height (stub)
    pub bearing_x: f32,
    pub bearing_y: f32,
}

/// Font texture atlas
#[derive(Debug, Clone)]
pub struct FontAtlas {
    pub bitmap: Vec<u8>,              // raw grayscale atlas bitmap
    pub width: u32,                   // atlas width
    pub height: u32,                  // atlas height
    pub design_size: f32,             // rasterization size in px
    pub glyphs: HashMap<char, Glyph>, // glyph metadata
    pub ascent: f32,                  // typographic ascent
    pub descent: f32,                 // typographic descent
}

impl FontAtlas {
    /// FontAtlas is deprecated - text rendering now handled by egui
    /// This is kept for backward compatibility but returns empty atlas
    pub fn new(_font: Font, design_size: f32) -> Self {
        // Font atlas creation removed - egui handles font rendering natively
        FontAtlas {
            bitmap: Vec::new(),
            width: 0,
            height: 0,
            design_size,
            glyphs: HashMap::new(),
            ascent: design_size * 0.8,
            descent: design_size * 0.2,
        }
    }

    pub fn get_glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }
}
