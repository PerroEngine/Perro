//! Modern native text rendering system using cosmic-text for layout/shaping
//! Replaces the old FontAtlas system with a full-featured text engine

use std::sync::{Arc, Mutex};
use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};
use rustc_hash::FxHashMap;
use crate::Vector2;

/// Native text renderer using cosmic-text for shaping and layout
/// Provides text metrics, layout, and glyph positioning for the rendering system
pub struct TextRenderer {
    pub(crate) font_system: Arc<Mutex<FontSystem>>,
    
    // Per-text buffers (cached for layout)
    text_buffers: FxHashMap<crate::uid32::Uid32, Buffer>,
    
    // Font cache: font_spec -> font_id
    font_cache: FxHashMap<String, cosmic_text::fontdb::ID>,
}

impl TextRenderer {
    pub fn new() -> Self {
        // Initialize font system with system fonts
        let mut font_system = FontSystem::new_with_locale_and_db(
            "en-US".to_string(),
            cosmic_text::fontdb::Database::new(),
        );
        
        // Load system fonts
        font_system.db_mut().load_system_fonts();
        
        // Debug: Check how many fonts were loaded
        let font_count = font_system.db().faces().count();
        eprintln!("DEBUG: Loaded {} system fonts", font_count);
        
        if font_count == 0 {
            eprintln!("WARNING: No system fonts loaded! Text rendering will fail.");
        }
        
        Self {
            font_system: Arc::new(Mutex::new(font_system)),
            text_buffers: FxHashMap::default(),
            font_cache: FxHashMap::default(),
        }
    }
    
    /// Load a font from file or system
    pub fn load_font(&mut self, font_spec: &str) -> Option<cosmic_text::fontdb::ID> {
        // Check cache first
        if let Some(&font_id) = self.font_cache.get(font_spec) {
            return Some(font_id);
        }
        
        let mut font_system = self.font_system.lock().ok()?;
        
        // Try loading from file first
        if font_spec.starts_with("res://") || font_spec.contains('.') {
            let file_path = if font_spec.starts_with("res://") {
                &font_spec[6..]
            } else {
                font_spec
            };
            
            if let Ok(data) = std::fs::read(file_path) {
                // load_font_data loads the font and returns the font_id
                let font_id = font_system.db_mut().load_font_data(data);
                // Query the font to get its ID after loading
                let family = cosmic_text::Family::Name(font_spec);
                if let Some(font_id) = font_system.db().query(&cosmic_text::fontdb::Query {
                    families: &[family],
                    weight: cosmic_text::fontdb::Weight::NORMAL,
                    stretch: cosmic_text::fontdb::Stretch::Normal,
                    style: cosmic_text::fontdb::Style::Normal,
                }) {
                    self.font_cache.insert(font_spec.to_string(), font_id);
                    return Some(font_id);
                }
            }
        }
        
        // Try system font
        let family = cosmic_text::Family::Name(font_spec);
        if let Some(font_id) = font_system.db().query(&cosmic_text::fontdb::Query {
            families: &[family],
            weight: cosmic_text::fontdb::Weight::NORMAL,
            stretch: cosmic_text::fontdb::Stretch::Normal,
            style: cosmic_text::fontdb::Style::Normal,
        }) {
            self.font_cache.insert(font_spec.to_string(), font_id);
            return Some(font_id);
        }
        
        // Fallback to system default fonts (cosmic-text has built-in fallback)
        // Try common system fonts as fallback
        let fallback_fonts = ["Arial", "Helvetica", "Times New Roman", "Courier New", "Verdana"];
        for fallback in &fallback_fonts {
            let fallback_family = cosmic_text::Family::Name(fallback);
            if let Some(font_id) = font_system.db().query(&cosmic_text::fontdb::Query {
                families: &[fallback_family],
                weight: cosmic_text::fontdb::Weight::NORMAL,
                stretch: cosmic_text::fontdb::Stretch::Normal,
                style: cosmic_text::fontdb::Style::Normal,
            }) {
                // Cache the fallback font with the original spec name
                self.font_cache.insert(font_spec.to_string(), font_id);
                return Some(font_id);
            }
        }
        
        // Last resort: use cosmic-text's default font (first font in database)
        // This ensures we always have a font to render with
        if let Some(first_font_id) = font_system.db().faces().next().map(|face| face.id) {
            self.font_cache.insert(font_spec.to_string(), first_font_id);
            return Some(first_font_id);
        }
        
        None
    }
    
    /// Get or create a text buffer for rendering
    pub fn get_or_create_buffer(&mut self, text_id: crate::uid32::Uid32, content: &str, font_spec: Option<&str>, font_size: f32) -> &mut Buffer {
        use std::collections::hash_map::Entry;
        
        // Load font first (before borrowing self.text_buffers)
        // If font_spec is None or empty, use system default
        let effective_font_spec = font_spec.filter(|s| !s.is_empty());
        if let Some(font_spec) = effective_font_spec {
            // Try to load the specified font, with fallback handled in load_font
            self.load_font(font_spec);
        }
        
        match self.text_buffers.entry(text_id) {
            Entry::Occupied(mut entry) => {
                let buffer = entry.get_mut();
                let mut font_system = self.font_system.lock().unwrap();
                
                // Create attributes with font (or use default if None)
                let mut attrs = Attrs::new();
                if let Some(font_spec) = effective_font_spec {
                    attrs = attrs.family(cosmic_text::Family::Name(font_spec));
                }
                // If no font_spec, cosmic-text will use its default font
                
                // Update buffer if content changed
                buffer.set_text(&mut *font_system, content, attrs, Shaping::Advanced);
                // CRITICAL: Layout the buffer after setting text
                buffer.set_size(&mut *font_system, f32::INFINITY, f32::INFINITY);
                buffer.shape_until_scroll(&mut *font_system, false);
                entry.into_mut()
            }
            Entry::Vacant(entry) => {
                let mut font_system = self.font_system.lock().unwrap();
                
                // Create attributes with font (or use default if None)
                let mut attrs = Attrs::new();
                if let Some(font_spec) = effective_font_spec {
                    attrs = attrs.family(cosmic_text::Family::Name(font_spec));
                }
                // If no font_spec, cosmic-text will use its default font
                
                // Create buffer with metrics
                let metrics = Metrics::new(font_size, font_size * 1.2);
                let mut buffer = Buffer::new(&mut *font_system, metrics);
                buffer.set_text(&mut *font_system, content, attrs, Shaping::Advanced);
                // CRITICAL: Layout the buffer after setting text
                buffer.set_size(&mut *font_system, f32::INFINITY, f32::INFINITY);
                buffer.shape_until_scroll(&mut *font_system, false);
                
                entry.insert(buffer)
            }
        }
    }
    
    /// Get text size using cosmic-text layout
    pub fn measure_text(&mut self, content: &str, font_spec: Option<&str>, font_size: f32, max_width: Option<f32>) -> Vector2 {
        // Load font first (before borrowing font_system)
        if let Some(font_spec) = font_spec {
            self.load_font(font_spec);
        }
        
        let mut font_system = self.font_system.lock().unwrap();
        
        // Create temporary buffer for measurement
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut *font_system, metrics);
        
        // Set font if specified
        let mut attrs = Attrs::new();
        if let Some(font_spec) = font_spec {
            attrs = attrs.family(cosmic_text::Family::Name(font_spec));
        }
        
        // Set text with optional width constraint
        buffer.set_text(&mut *font_system, content, attrs, Shaping::Advanced);
        if let Some(max_w) = max_width {
            buffer.set_size(&mut *font_system, max_w, f32::INFINITY);
        }
        
        // Get layout bounds - use layout_runs to calculate size
        let mut width = 0.0f32;
        let mut max_line_y = 0.0f32;
        let metrics = buffer.metrics();
        let mut line_count = 0;
        for line in buffer.layout_runs() {
            let line_width = line.glyphs.iter().map(|g| g.w).sum::<f32>();
            width = width.max(line_width);
            max_line_y = max_line_y.max(line.line_y);
            line_count += 1;
        }
        
        // Height is based on line metrics
        let height = if line_count > 0 {
            max_line_y + metrics.line_height
        } else {
            metrics.line_height
        };
        
        Vector2::new(width, height)
    }
    
    /// Get glyph positions for rendering
    pub fn get_glyph_positions(&mut self, text_id: crate::uid32::Uid32, content: &str, font_spec: Option<&str>, font_size: f32) -> Vec<GlyphPosition> {
        // If font_spec is None or empty, use system default
        let effective_font_spec = font_spec.filter(|s| !s.is_empty());
        // get_or_create_buffer already handles layout (shape_until_scroll)
        let buffer = self.get_or_create_buffer(text_id, content, effective_font_spec, font_size);
        
        let mut glyphs = Vec::new();
        
        // Build a mapping from glyph indices to characters
        // This is approximate - cosmic-text doesn't directly provide char->glyph mapping
        let chars: Vec<char> = content.chars().collect();
        let mut char_idx = 0;
        
        // Iterate through layout runs to get glyph positions
        let layout_runs: Vec<_> = buffer.layout_runs().collect();
        
        if layout_runs.is_empty() {
            eprintln!("DEBUG: No layout runs for text: '{}' (buffer line count: {})", 
                content, buffer.lines.len());
        }
        
        for line in layout_runs {
            let line_y = line.line_y;
            for glyph in line.glyphs.iter() {
                // Try to get the character for this glyph
                // This is approximate - for complex scripts, one glyph may represent multiple chars
                let char_code = if char_idx < chars.len() {
                    Some(chars[char_idx])
                } else {
                    None
                };
                // Advance char index (may need adjustment for complex scripts)
                char_idx += 1;
                
                glyphs.push(GlyphPosition {
                    x: glyph.x,
                    y: line_y + glyph.y,
                    width: glyph.w,
                    height: glyph.w, // Use width as height approximation if h field doesn't exist
                    font_id: glyph.font_id,
                    glyph_id: glyph.glyph_id,
                    char_code,
                });
            }
        }
        
        glyphs
    }
    
    /// Remove a text buffer from cache
    pub fn remove_buffer(&mut self, text_id: crate::uid32::Uid32) {
        self.text_buffers.remove(&text_id);
    }
}

/// Glyph position for rendering
#[derive(Debug, Clone)]
pub struct GlyphPosition {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_id: cosmic_text::fontdb::ID,
    pub glyph_id: u16,
    pub char_code: Option<char>, // Character code for rasterization (may be None for complex glyphs)
}
