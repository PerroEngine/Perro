use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for Color {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Lighten a color by a percentage (0.0 to 1.0)
    /// 0.0 = no change, 1.0 = white
    pub fn lighten(&self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            r: ((self.r as f32) + (255.0 - self.r as f32) * amount) as u8,
            g: ((self.g as f32) + (255.0 - self.g as f32) * amount) as u8,
            b: ((self.b as f32) + (255.0 - self.b as f32) * amount) as u8,
            a: self.a,
        }
    }

    /// Darken a color by a percentage (0.0 to 1.0)
    /// 0.0 = no change, 1.0 = black
    pub fn darken(&self, amount: f32) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        Self {
            r: ((self.r as f32) * (1.0 - amount)) as u8,
            g: ((self.g as f32) * (1.0 - amount)) as u8,
            b: ((self.b as f32) * (1.0 - amount)) as u8,
            a: self.a,
        }
    }

    pub fn to_array(&self) -> [f32; 3] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        ]
    }

    pub fn from_hex(s: &str) -> Result<Self, String> {
        let s = s.trim_start_matches('#');
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
                Ok(Self::new(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?;
                let g = u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?;
                let b = u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?;
                let a = u8::from_str_radix(&s[6..8], 16).map_err(|e| e.to_string())?;
                Ok(Self::new(r, g, b, a))
            }
            _ => Err("Invalid hex color length, expected 6 or 8 hex digits".to_string()),
        }
    }

    pub fn from_preset(name: &str) -> Option<Self> {
        let colors: HashMap<&str, Color> = vec![
            // BLACK & WHITE & NEUTRALS
            ("black", Color::new(0, 0, 0, 255)),
            ("white", Color::new(255, 255, 255, 255)),
            ("transparent", Color::new(255, 255, 255, 0)),
            ("trans", Color::new(255, 255, 255, 0)),
            // GRAY FAMILY
            ("gray-1", Color::new(212, 214, 217, 255)),
            ("gray-2", Color::new(197, 199, 203, 255)),
            ("gray-3", Color::new(180, 183, 188, 255)),
            ("gray-4", Color::new(162, 166, 173, 255)),
            ("gray-5", Color::new(116, 126, 142, 255)),
            ("gray-6", Color::new(78, 89, 107, 255)),
            ("gray-7", Color::new(54, 68, 81, 255)),
            ("gray-8", Color::new(39, 52, 66, 255)),
            ("gray-9", Color::new(22, 32, 45, 255)),
            ("gray-10", Color::new(12, 18, 32, 255)),
            ("slate-1", Color::new(211, 217, 224, 255)),
            ("slate-2", Color::new(194, 203, 214, 255)),
            ("slate-3", Color::new(170, 182, 199, 255)),
            ("slate-4", Color::new(147, 165, 186, 255)),
            ("slate-5", Color::new(109, 128, 153, 255)),
            ("slate-6", Color::new(73, 93, 115, 255)),
            ("slate-7", Color::new(52, 68, 86, 255)),
            ("slate-8", Color::new(37, 52, 70, 255)),
            ("slate-9", Color::new(22, 32, 48, 255)),
            ("slate-10", Color::new(11, 17, 34, 255)),
            ("zinc-1", Color::new(213, 213, 213, 255)),
            ("zinc-2", Color::new(196, 196, 198, 255)),
            ("zinc-3", Color::new(173, 173, 179, 255)),
            ("zinc-4", Color::new(156, 156, 165, 255)),
            ("zinc-5", Color::new(118, 118, 133, 255)),
            ("zinc-6", Color::new(83, 83, 95, 255)),
            ("zinc-7", Color::new(60, 60, 71, 255)),
            ("zinc-8", Color::new(46, 46, 55, 255)),
            ("zinc-9", Color::new(29, 29, 33, 255)),
            ("zinc-10", Color::new(18, 18, 21, 255)),
            ("neutral-1", Color::new(213, 213, 213, 255)),
            ("neutral-2", Color::new(198, 198, 198, 255)),
            ("neutral-3", Color::new(175, 175, 175, 255)),
            ("neutral-4", Color::new(156, 156, 156, 255)),
            ("neutral-5", Color::new(120, 120, 120, 255)),
            ("neutral-6", Color::new(85, 85, 85, 255)),
            ("neutral-7", Color::new(60, 60, 60, 255)),
            ("neutral-8", Color::new(50, 50, 52, 255)),
            ("neutral-9", Color::new(40, 40, 42, 255)),
            ("neutral-10", Color::new(30, 30, 32, 255)),
            ("stone-1", Color::new(213, 213, 211, 255)),
            ("stone-2", Color::new(198, 198, 195, 255)),
            ("stone-3", Color::new(178, 175, 171, 255)),
            ("stone-4", Color::new(163, 159, 155, 255)),
            ("stone-5", Color::new(124, 121, 116, 255)),
            ("stone-6", Color::new(88, 83, 79, 255)),
            ("stone-7", Color::new(64, 61, 57, 255)),
            ("stone-8", Color::new(52, 49, 47, 255)),
            ("stone-9", Color::new(42, 40, 38, 255)),
            ("stone-10", Color::new(32, 30, 28, 255)),
            ("cinder-1", Color::new(44, 70, 84, 255)),
            ("cinder-2", Color::new(41, 64, 76, 255)),
            ("cinder-3", Color::new(37, 58, 69, 255)),
            ("cinder-4", Color::new(34, 51, 62, 255)),
            ("cinder-5", Color::new(31, 45, 54, 255)),
            ("cinder-6", Color::new(28, 42, 50, 255)),
            ("cinder-7", Color::new(25, 38, 47, 255)),
            ("cinder-8", Color::new(21, 35, 43, 255)),
            ("cinder-9", Color::new(18, 31, 39, 255)),
            ("cinder-10", Color::new(15, 28, 36, 255)),
            ("steel-1", Color::new(185, 188, 193, 255)),  
            ("steel-2", Color::new(165, 168, 173, 255)), 
            ("steel-3", Color::new(145, 148, 153, 255)),  
            ("steel-4", Color::new(125, 128, 133, 255)),  
            ("steel-5", Color::new(105, 108, 113, 255)),   
            ("steel-6", Color::new(85, 88, 93, 255)),  
            ("steel-7", Color::new(65, 70, 80, 255)),   
            ("steel-8", Color::new(55, 60, 70, 255)),     
            ("steel-9", Color::new(45, 50, 60, 255)),   
            ("steel-10", Color::new(35, 40, 50, 255)),
            ("red-1", Color::new(217, 187, 187, 255)),
            ("red-2", Color::new(217, 167, 167, 255)),
            ("red-3", Color::new(217, 147, 147, 255)),
            ("red-4", Color::new(215, 115, 115, 255)),
            ("red-5", Color::new(216, 79, 79, 255)),
            ("red-6", Color::new(203, 43, 43, 255)),
            ("red-7", Color::new(187, 22, 22, 255)),
            ("red-8", Color::new(157, 16, 16, 255)),
            ("red-9", Color::new(130, 15, 15, 255)),
            ("red-10", Color::new(108, 17, 17, 255)),
            ("ruby-1", Color::new(217, 181, 189, 255)),
            ("ruby-2", Color::new(213, 145, 159, 255)),
            ("ruby-3", Color::new(204, 111, 133, 255)),
            ("ruby-4", Color::new(191, 79, 108, 255)),
            ("ruby-5", Color::new(179, 54, 86, 255)),
            ("ruby-6", Color::new(157, 32, 66, 255)),
            ("ruby-7", Color::new(136, 22, 50, 255)),
            ("ruby-8", Color::new(111, 15, 39, 255)),
            ("ruby-9", Color::new(85, 12, 31, 255)),
            ("ruby-10", Color::new(64, 9, 23, 255)),
            ("rose-1", Color::new(217, 184, 187, 255)),
            ("rose-2", Color::new(217, 168, 174, 255)),
            ("rose-3", Color::new(216, 150, 163, 255)),
            ("rose-4", Color::new(215, 115, 136, 255)),
            ("rose-5", Color::new(213, 79, 103, 255)),
            ("rose-6", Color::new(207, 39, 73, 255)),
            ("rose-7", Color::new(191, 17, 56, 255)),
            ("rose-8", Color::new(162, 11, 46, 255)),
            ("rose-9", Color::new(135, 11, 44, 255)),
            ("rose-10", Color::new(116, 11, 42, 255)),
            // ORANGE FAMILY
            ("orange-1", Color::new(217, 194, 164, 255)),
            ("orange-2", Color::new(217, 180, 137, 255)),
            ("orange-3", Color::new(216, 158, 116, 255)),
            ("orange-4", Color::new(215, 130, 79, 255)),
            ("orange-5", Color::new(213, 98, 38, 255)),
            ("orange-6", Color::new(212, 79, 11, 255)),
            ("orange-7", Color::new(199, 61, 6, 255)),
            ("orange-8", Color::new(165, 45, 6, 255)),
            ("orange-9", Color::new(131, 34, 11, 255)),
            ("orange-10", Color::new(105, 29, 11, 255)),
            ("amber-1", Color::new(217, 209, 174, 255)),
            ("amber-2", Color::new(216, 195, 145, 255)),
            ("amber-3", Color::new(215, 178, 95, 255)),
            ("amber-4", Color::new(214, 159, 51, 255)),
            ("amber-5", Color::new(213, 139, 22, 255)),
            ("amber-6", Color::new(208, 119, 6, 255)),
            ("amber-7", Color::new(184, 87, 3, 255)),
            ("amber-8", Color::new(153, 58, 5, 255)),
            ("amber-9", Color::new(124, 44, 8, 255)),
            ("amber-10", Color::new(102, 36, 9, 255)),
            ("gold-1", Color::new(217, 205, 170, 255)),
            ("gold-2", Color::new(217, 192, 145, 255)),
            ("gold-3", Color::new(213, 174, 111, 255)),
            ("gold-4", Color::new(204, 162, 79, 255)),
            ("gold-5", Color::new(196, 139, 54, 255)),
            ("gold-6", Color::new(179, 119, 32, 255)),
            ("gold-7", Color::new(153, 95, 19, 255)),
            ("gold-8", Color::new(128, 73, 12, 255)),
            ("gold-9", Color::new(102, 58, 9, 255)),
            ("gold-10", Color::new(77, 44, 6, 255)),
            // YELLOW FAMILY
            ("yellow-1", Color::new(217, 209, 174, 255)),
            ("yellow-2", Color::new(216, 201, 140, 255)),
            ("yellow-3", Color::new(216, 192, 95, 255)),
            ("yellow-4", Color::new(215, 175, 47, 255)),
            ("yellow-5", Color::new(213, 159, 12, 255)),
            ("yellow-6", Color::new(199, 137, 4, 255)),
            ("yellow-7", Color::new(172, 102, 2, 255)),
            ("yellow-8", Color::new(137, 71, 4, 255)),
            ("yellow-9", Color::new(113, 56, 8, 255)),
            ("yellow-10", Color::new(96, 46, 11, 255)),
            ("lime-1", Color::new(196, 217, 170, 255)),
            ("lime-2", Color::new(184, 214, 147, 255)),
            ("lime-3", Color::new(167, 212, 108, 255)),
            ("lime-4", Color::new(143, 206, 70, 255)),
            ("lime-5", Color::new(122, 196, 35, 255)),
            ("lime-6", Color::new(97, 174, 12, 255)),
            ("lime-7", Color::new(74, 139, 7, 255)),
            ("lime-8", Color::new(56, 105, 9, 255)),
            ("lime-9", Color::new(46, 84, 11, 255)),
            ("lime-10", Color::new(39, 71, 12, 255)),
            // GREEN FAMILY
            ("green-1", Color::new(186, 215, 192, 255)),
            ("green-2", Color::new(167, 214, 178, 255)),
            ("green-3", Color::new(135, 210, 160, 255)),
            ("green-4", Color::new(91, 203, 129, 255)),
            ("green-5", Color::new(50, 189, 96, 255)),
            ("green-6", Color::new(22, 168, 70, 255)),
            ("green-7", Color::new(12, 139, 55, 255)),
            ("green-8", Color::new(12, 109, 46, 255)),
            ("green-9", Color::new(12, 86, 39, 255)),
            ("green-10", Color::new(12, 71, 34, 255)),
            ("forest-1", Color::new(181, 196, 181, 255)),
            ("forest-2", Color::new(155, 181, 155, 255)),
            ("forest-3", Color::new(131, 163, 131, 255)),
            ("forest-4", Color::new(96, 141, 108, 255)),
            ("forest-5", Color::new(66, 118, 85, 255)),
            ("forest-6", Color::new(44, 94, 66, 255)),
            ("forest-7", Color::new(33, 74, 50, 255)),
            ("forest-8", Color::new(26, 58, 39, 255)),
            ("forest-9", Color::new(19, 42, 27, 255)),
            ("forest-10", Color::new(11, 27, 19, 255)),
            ("emerald-1", Color::new(181, 215, 192, 255)),
            ("emerald-2", Color::new(159, 213, 178, 255)),
            ("emerald-3", Color::new(122, 207, 160, 255)),
            ("emerald-4", Color::new(79, 196, 141, 255)),
            ("emerald-5", Color::new(35, 179, 115, 255)),
            ("emerald-6", Color::new(9, 157, 97, 255)),
            ("emerald-7", Color::new(2, 128, 79, 255)),
            ("emerald-8", Color::new(2, 102, 65, 255)),
            ("emerald-9", Color::new(3, 81, 53, 255)),
            ("emerald-10", Color::new(3, 66, 44, 255)),
            // TEAL/CYAN FAMILY
            ("teal-1", Color::new(186, 215, 200, 255)),
            ("teal-2", Color::new(154, 213, 191, 255)),
            ("teal-3", Color::new(112, 209, 175, 255)),
            ("teal-4", Color::new(66, 199, 163, 255)),
            ("teal-5", Color::new(31, 180, 147, 255)),
            ("teal-6", Color::new(12, 157, 128, 255)),
            ("teal-7", Color::new(7, 126, 105, 255)),
            ("teal-8", Color::new(9, 100, 85, 255)),
            ("teal-9", Color::new(10, 80, 69, 255)),
            ("teal-10", Color::new(11, 66, 57, 255)),
            ("cyan-1", Color::new(181, 217, 217, 255)),
            ("cyan-2", Color::new(158, 213, 216, 255)),
            ("cyan-3", Color::new(119, 207, 214, 255)),
            ("cyan-4", Color::new(73, 197, 212, 255)),
            ("cyan-5", Color::new(22, 179, 202, 255)),
            ("cyan-6", Color::new(3, 155, 180, 255)),
            ("cyan-7", Color::new(4, 123, 151, 255)),
            ("cyan-8", Color::new(8, 99, 122, 255)),
            ("cyan-9", Color::new(12, 80, 99, 255)),
            ("cyan-10", Color::new(7, 63, 77, 255)),
            ("sea-1", Color::new(167, 196, 196, 255)),
            ("sea-2", Color::new(144, 184, 188, 255)),
            ("sea-3", Color::new(111, 163, 171, 255)),
            ("sea-4", Color::new(81, 144, 155, 255)),
            ("sea-5", Color::new(52, 125, 140, 255)),
            ("sea-6", Color::new(37, 105, 120, 255)),
            ("sea-7", Color::new(26, 86, 101, 255)),
            ("sea-8", Color::new(19, 66, 82, 255)),
            ("sea-9", Color::new(13, 50, 62, 255)),
            ("sea-10", Color::new(7, 35, 46, 255)),
            ("ocean-1", Color::new(171, 196, 196, 255)),
            ("ocean-2", Color::new(144, 184, 188, 255)),
            ("ocean-3", Color::new(111, 167, 175, 255)),
            ("ocean-4", Color::new(81, 148, 163, 255)),
            ("ocean-5", Color::new(52, 129, 148, 255)),
            ("ocean-6", Color::new(37, 109, 128, 255)),
            ("ocean-7", Color::new(26, 90, 109, 255)),
            ("ocean-8", Color::new(19, 70, 90, 255)),
            ("ocean-9", Color::new(13, 50, 70, 255)),
            ("ocean-10", Color::new(7, 35, 54, 255)),
            // BLUE FAMILY
            ("sky-1", Color::new(186, 204, 217, 255)),
            ("sky-2", Color::new(170, 194, 216, 255)),
            ("sky-3", Color::new(135, 178, 215, 255)),
            ("sky-4", Color::new(88, 162, 214, 255)),
            ("sky-5", Color::new(39, 145, 210, 255)),
            ("sky-6", Color::new(8, 128, 198, 255)),
            ("sky-7", Color::new(1, 102, 169, 255)),
            ("sky-8", Color::new(1, 82, 137, 255)),
            ("sky-9", Color::new(4, 69, 113, 255)),
            ("sky-10", Color::new(7, 57, 94, 255)),
            ("blue-1", Color::new(184, 195, 217, 255)),
            ("blue-2", Color::new(167, 182, 216, 255)),
            ("blue-3", Color::new(140, 168, 216, 255)),
            ("blue-4", Color::new(107, 152, 215, 255)),
            ("blue-5", Color::new(68, 127, 213, 255)),
            ("blue-6", Color::new(41, 101, 209, 255)),
            ("blue-7", Color::new(26, 73, 200, 255)),
            ("blue-8", Color::new(17, 58, 184, 255)),
            ("blue-9", Color::new(18, 48, 149, 255)),
            ("blue-10", Color::new(18, 43, 117, 255)),
            ("sapphire-1", Color::new(181, 190, 217, 255)),
            ("sapphire-2", Color::new(155, 171, 213, 255)),
            ("sapphire-3", Color::new(124, 155, 208, 255)),
            ("sapphire-4", Color::new(88, 136, 204, 255)),
            ("sapphire-5", Color::new(58, 118, 196, 255)),
            ("sapphire-6", Color::new(37, 94, 179, 255)),
            ("sapphire-7", Color::new(26, 74, 153, 255)),
            ("sapphire-8", Color::new(19, 58, 128, 255)),
            ("sapphire-9", Color::new(13, 42, 102, 255)),
            ("sapphire-10", Color::new(7, 31, 77, 255)),
            // INDIGO FAMILY
            ("indigo-1", Color::new(184, 190, 217, 255)),
            ("indigo-2", Color::new(170, 178, 217, 255)),
            ("indigo-3", Color::new(149, 163, 216, 255)),
            ("indigo-4", Color::new(120, 138, 214, 255)),
            ("indigo-5", Color::new(94, 108, 210, 255)),
            ("indigo-6", Color::new(70, 75, 205, 255)),
            ("indigo-7", Color::new(58, 52, 195, 255)),
            ("indigo-8", Color::new(49, 42, 172, 255)),
            ("indigo-9", Color::new(40, 36, 139, 255)),
            ("indigo-10", Color::new(36, 34, 110, 255)),
            // VIOLET FAMILY
            ("violet-1", Color::new(192, 187, 217, 255)),
            ("violet-2", Color::new(181, 180, 216, 255)),
            ("violet-3", Color::new(167, 165, 216, 255)),
            ("violet-4", Color::new(143, 140, 215, 255)),
            ("violet-5", Color::new(122, 107, 213, 255)),
            ("violet-6", Color::new(102, 68, 209, 255)),
            ("violet-7", Color::new(91, 43, 202, 255)),
            ("violet-8", Color::new(80, 29, 184, 255)),
            ("violet-9", Color::new(67, 24, 155, 255)),
            ("violet-10", Color::new(56, 21, 127, 255)),
            // PURPLE FAMILY
            ("purple-1", Color::new(200, 187, 217, 255)),
            ("purple-2", Color::new(192, 180, 217, 255)),
            ("purple-3", Color::new(178, 165, 217, 255)),
            ("purple-4", Color::new(164, 140, 216, 255)),
            ("purple-5", Color::new(143, 102, 214, 255)),
            ("purple-6", Color::new(123, 63, 210, 255)),
            ("purple-7", Color::new(108, 38, 199, 255)),
            ("purple-8", Color::new(92, 25, 175, 255)),
            ("purple-9", Color::new(78, 24, 143, 255)),
            ("purple-10", Color::new(65, 20, 115, 255)),
            // FUCHSIA/PINK FAMILY
            ("fuchsia-1", Color::new(215, 187, 217, 255)),
            ("fuchsia-2", Color::new(213, 180, 216, 255)),
            ("fuchsia-3", Color::new(208, 160, 216, 255)),
            ("fuchsia-4", Color::new(204, 127, 214, 255)),
            ("fuchsia-5", Color::new(197, 89, 211, 255)),
            ("fuchsia-6", Color::new(184, 52, 179, 255)),
            ("fuchsia-7", Color::new(163, 22, 179, 255)),
            ("fuchsia-8", Color::new(138, 16, 149, 255)),
            ("fuchsia-9", Color::new(114, 15, 122, 255)),
            ("fuchsia-10", Color::new(95, 15, 99, 255)),
            ("pink-1", Color::new(215, 187, 200, 255)),
            ("pink-2", Color::new(214, 180, 194, 255)),
            ("pink-3", Color::new(213, 160, 181, 255)),
            ("pink-4", Color::new(211, 125, 165, 255)),
            ("pink-5", Color::new(207, 84, 142, 255)),
            ("pink-6", Color::new(200, 53, 119, 255)),
            ("pink-7", Color::new(186, 29, 92, 255)),
            ("pink-8", Color::new(162, 14, 72, 255)),
            ("pink-9", Color::new(133, 14, 60, 255)),
            ("pink-10", Color::new(111, 11, 52, 255)),
        ]
        .into_iter()
        .collect();

        // If the name is exact match, return it
        if let Some(&color) = colors.get(name) {
            return Some(color);
        }

        // If no exact match, try to parse base and shade number like "red-5"
        let mut parts = name.splitn(2, '-');
        let base = parts.next()?;
        let shade_str = parts.next().unwrap_or("5");
        let shade_num: u8 = shade_str.parse().unwrap_or(5);
        let shade_name = format!("{}-{}", base, shade_num.clamp(1, 10));

        colors.get(shade_name.as_str()).copied()
    }
}
