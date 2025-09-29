use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Font family/style helpers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Regular,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    Normal,
    Italic,
}

#[derive(Debug, Clone)]
pub struct Font {
    pub data: &'static [u8],
}

type FontKey = (Weight, Style);

/// Embedded NotoSans fonts (expandable)
static NOTO_SANS: Lazy<HashMap<FontKey, &'static [u8]>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        (Weight::Regular, Style::Normal),
        include_bytes!("fonts/NotoSans/NotoSans-Regular.ttf").as_ref(),
    );
    m.insert(
        (Weight::Bold, Style::Normal),
        include_bytes!("fonts/NotoSans/NotoSans-Bold.ttf").as_ref(),
    );
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

/// Per-glyph atlas data
#[derive(Debug, Clone)]
pub struct Glyph {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub metrics: fontdue::Metrics,
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
    pub fn new(font: Font, design_size: f32) -> Self {
        use fontdue::Font as Fontdue;
        use fontdue::FontSettings;

        let fd_font =
            Fontdue::from_bytes(font.data, FontSettings::default()).expect("Invalid font data");

        // ✅ Get horizontal line metrics for proper baseline alignment
        let line_metrics = fd_font
            .horizontal_line_metrics(design_size)
            .expect("Font missing horizontal line metrics");

        // Preload ASCII 32-126
        let chars: Vec<char> = (32u8..=126u8).map(|c| c as char).collect();
        let atlas_w: u32 = 1024;
        let atlas_h: u32 = 1024;

        let mut bitmap = vec![0u8; (atlas_w * atlas_h) as usize];
        let mut glyphs = HashMap::new();

        let mut pen_x: u32 = 2;
        let mut pen_y: u32 = 2;
        let mut row_h: u32 = 0;

        for ch in chars {
            let (metrics, bmp) = fd_font.rasterize(ch, design_size);

            let gw = metrics.width as u32;
            let gh = metrics.height as u32;

            if gw == 0 || gh == 0 {
                glyphs.insert(
                    ch,
                    Glyph {
                        u0: 0.0,
                        v0: 0.0,
                        u1: 0.0,
                        v1: 0.0,
                        metrics,
                        bearing_x: metrics.xmin as f32,
                        bearing_y: metrics.ymin as f32,
                    },
                );
                continue;
            }

            if pen_x + gw + 2 > atlas_w {
                pen_x = 2;
                pen_y += row_h + 2;
                row_h = 0;
            }
            if pen_y + gh + 2 > atlas_h {
                break;
            }

            // Blit glyph bitmap into atlas
            for y in 0..gh {
                for x in 0..gw {
                    let src = (y as usize * gw as usize) + x as usize;
                    let dst = ((pen_y + y) as usize * atlas_w as usize) + (pen_x + x) as usize;
                    bitmap[dst] = bmp[src];
                }
            }

            let u0 = pen_x as f32 / atlas_w as f32;
            let v0 = pen_y as f32 / atlas_h as f32;
            let u1 = (pen_x + gw) as f32 / atlas_w as f32;
            let v1 = (pen_y + gh) as f32 / atlas_h as f32;

            glyphs.insert(
                ch,
                Glyph {
                    u0,
                    v0,
                    u1,
                    v1,
                    metrics,
                    bearing_x: metrics.xmin as f32,
                    bearing_y: metrics.ymin as f32,
                },
            );

            pen_x += gw + 2;
            row_h = row_h.max(gh);
        }

        FontAtlas {
            bitmap,
            width: atlas_w,
            height: atlas_h,
            design_size,
            glyphs,
            ascent: line_metrics.ascent,
            descent: line_metrics.descent,
        }
    }

    pub fn get_glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch)
    }
}