use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
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
    // BLACK & WHITE
    ("black", Color::new(0, 0, 0, 255)),
    ("white", Color::new(255, 255, 255, 255)),
    ("transparent", Color::new(255, 255, 255, 0)),
    ("trans", Color::new(255, 255, 255, 0)),

    // SLATE
    ("slate-1", Color::new(248, 250, 252, 255)),
    ("slate-2", Color::new(241, 245, 249, 255)),
    ("slate-3", Color::new(226, 232, 240, 255)),
    ("slate-4", Color::new(203, 213, 225, 255)),
    ("slate-5", Color::new(148, 163, 184, 255)),
    ("slate-6", Color::new(100, 116, 139, 255)),
    ("slate-7", Color::new(71, 85, 105, 255)),
    ("slate-8", Color::new(51, 65, 85, 255)),
    ("slate-9", Color::new(30, 41, 59, 255)),
    ("slate-10", Color::new(15, 23, 42, 255)),

    // GRAY
    ("gray-1", Color::new(249, 250, 251, 255)),
    ("gray-2", Color::new(243, 244, 246, 255)),
    ("gray-3", Color::new(229, 231, 235, 255)),
    ("gray-4", Color::new(209, 213, 219, 255)),
    ("gray-5", Color::new(156, 163, 175, 255)),
    ("gray-6", Color::new(107, 114, 128, 255)),
    ("gray-7", Color::new(75, 85, 99, 255)),
    ("gray-8", Color::new(55, 65, 81, 255)),
    ("gray-9", Color::new(31, 41, 55, 255)),
    ("gray-10", Color::new(17, 24, 39, 255)),

    // ZINC
    ("zinc-1", Color::new(250, 250, 250, 255)),
    ("zinc-2", Color::new(244, 244, 245, 255)),
    ("zinc-3", Color::new(228, 228, 231, 255)),
    ("zinc-4", Color::new(212, 212, 216, 255)),
    ("zinc-5", Color::new(161, 161, 170, 255)),
    ("zinc-6", Color::new(113, 113, 122, 255)),
    ("zinc-7", Color::new(82, 82, 91, 255)),
    ("zinc-8", Color::new(63, 63, 70, 255)),
    ("zinc-9", Color::new(39, 39, 42, 255)),
    ("zinc-10", Color::new(24, 24, 27, 255)),

    // NEUTRAL
    ("neutral-1", Color::new(250, 250, 250, 255)),
    ("neutral-2", Color::new(245, 245, 245, 255)),
    ("neutral-3", Color::new(229, 229, 229, 255)),
    ("neutral-4", Color::new(212, 212, 212, 255)),
    ("neutral-5", Color::new(163, 163, 163, 255)),
    ("neutral-6", Color::new(115, 115, 115, 255)),
    ("neutral-7", Color::new(82, 82, 82, 255)),
    ("neutral-8", Color::new(64, 64, 64, 255)),
    ("neutral-9", Color::new(38, 38, 38, 255)),
    ("neutral-10", Color::new(23, 23, 23, 255)),

    // STONE
    ("stone-1", Color::new(250, 250, 249, 255)),
    ("stone-2", Color::new(245, 245, 244, 255)),
    ("stone-3", Color::new(231, 229, 228, 255)),
    ("stone-4", Color::new(214, 211, 209, 255)),
    ("stone-5", Color::new(168, 162, 158, 255)),
    ("stone-6", Color::new(120, 113, 108, 255)),
    ("stone-7", Color::new(87, 83, 78, 255)),
    ("stone-8", Color::new(68, 64, 60, 255)),
    ("stone-9", Color::new(41, 37, 36, 255)),
    ("stone-10", Color::new(28, 25, 23, 255)),

    // RED
    ("red-1", Color::new(254, 242, 242, 255)),
    ("red-2", Color::new(254, 226, 226, 255)),
    ("red-3", Color::new(254, 202, 202, 255)),
    ("red-4", Color::new(252, 165, 165, 255)),
    ("red-5", Color::new(248, 113, 113, 255)),
    ("red-6", Color::new(239, 68, 68, 255)),
    ("red-7", Color::new(220, 38, 38, 255)),
    ("red-8", Color::new(185, 28, 28, 255)),
    ("red-9", Color::new(153, 27, 27, 255)),
    ("red-10", Color::new(127, 29, 29, 255)),

    // ORANGE
    ("orange-1", Color::new(255, 247, 237, 255)),
    ("orange-2", Color::new(255, 237, 213, 255)),
    ("orange-3", Color::new(254, 215, 170, 255)),
    ("orange-4", Color::new(253, 186, 116, 255)),
    ("orange-5", Color::new(251, 146, 60, 255)),
    ("orange-6", Color::new(249, 115, 22, 255)),
    ("orange-7", Color::new(234, 88, 12, 255)),
    ("orange-8", Color::new(194, 65, 12, 255)),
    ("orange-9", Color::new(154, 52, 18, 255)),
    ("orange-10", Color::new(124, 45, 18, 255)),

    // AMBER
    ("amber-1", Color::new(255, 251, 235, 255)),
    ("amber-2", Color::new(254, 243, 199, 255)),
    ("amber-3", Color::new(253, 230, 138, 255)),
    ("amber-4", Color::new(252, 211, 77, 255)),
    ("amber-5", Color::new(251, 191, 36, 255)),
    ("amber-6", Color::new(245, 158, 11, 255)),
    ("amber-7", Color::new(217, 119, 6, 255)),
    ("amber-8", Color::new(180, 83, 9, 255)),
    ("amber-9", Color::new(146, 64, 14, 255)),
    ("amber-10", Color::new(120, 53, 15, 255)),

    // YELLOW
    ("yellow-1", Color::new(254, 252, 232, 255)),
    ("yellow-2", Color::new(254, 249, 195, 255)),
    ("yellow-3", Color::new(254, 240, 138, 255)),
    ("yellow-4", Color::new(253, 224, 71, 255)),
    ("yellow-5", Color::new(250, 204, 21, 255)),
    ("yellow-6", Color::new(234, 179, 8, 255)),
    ("yellow-7", Color::new(202, 138, 4, 255)),
    ("yellow-8", Color::new(161, 98, 7, 255)),
    ("yellow-9", Color::new(133, 77, 14, 255)),
    ("yellow-10", Color::new(113, 63, 18, 255)),

    // LIME
    ("lime-1", Color::new(247, 254, 231, 255)),
    ("lime-2", Color::new(236, 252, 203, 255)),
    ("lime-3", Color::new(217, 249, 157, 255)),
    ("lime-4", Color::new(190, 242, 100, 255)),
    ("lime-5", Color::new(163, 230, 53, 255)),
    ("lime-6", Color::new(132, 204, 22, 255)),
    ("lime-7", Color::new(101, 163, 13, 255)),
    ("lime-8", Color::new(77, 124, 15, 255)),
    ("lime-9", Color::new(63, 98, 18, 255)),
    ("lime-10", Color::new(54, 83, 20, 255)),

    // GREEN
    ("green-1", Color::new(240, 253, 244, 255)),
    ("green-2", Color::new(220, 252, 231, 255)),
    ("green-3", Color::new(187, 247, 208, 255)),
    ("green-4", Color::new(134, 239, 172, 255)),
    ("green-5", Color::new(74, 222, 128, 255)),
    ("green-6", Color::new(34, 197, 94, 255)),
    ("green-7", Color::new(22, 163, 74, 255)),
    ("green-8", Color::new(21, 128, 61, 255)),
    ("green-9", Color::new(22, 101, 52, 255)),
    ("green-10", Color::new(20, 83, 45, 255)),

    // EMERALD
    ("emerald-1", Color::new(236, 253, 245, 255)),
    ("emerald-2", Color::new(209, 250, 229, 255)),
    ("emerald-3", Color::new(167, 243, 208, 255)),
    ("emerald-4", Color::new(110, 231, 183, 255)),
    ("emerald-5", Color::new(52, 211, 153, 255)),
    ("emerald-6", Color::new(16, 185, 129, 255)),
    ("emerald-7", Color::new(5, 150, 105, 255)),
    ("emerald-8", Color::new(4, 120, 87, 255)),
    ("emerald-9", Color::new(6, 95, 70, 255)),
    ("emerald-10", Color::new(6, 78, 59, 255)),

    // TEAL
    ("teal-1", Color::new(240, 253, 250, 255)),
    ("teal-2", Color::new(204, 251, 241, 255)),
    ("teal-3", Color::new(153, 246, 228, 255)),
    ("teal-4", Color::new(94, 234, 212, 255)),
    ("teal-5", Color::new(45, 212, 191, 255)),
    ("teal-6", Color::new(20, 184, 166, 255)),
    ("teal-7", Color::new(13, 148, 136, 255)),
    ("teal-8", Color::new(15, 118, 110, 255)),
    ("teal-9", Color::new(17, 94, 89, 255)),
    ("teal-10", Color::new(19, 78, 74, 255)),

    // CYAN
    ("cyan-1", Color::new(236, 254, 255, 255)),
    ("cyan-2", Color::new(207, 250, 254, 255)),
    ("cyan-3", Color::new(165, 243, 252, 255)),
    ("cyan-4", Color::new(103, 232, 249, 255)),
    ("cyan-5", Color::new(34, 211, 238, 255)),
    ("cyan-6", Color::new(6, 182, 212, 255)),
    ("cyan-7", Color::new(8, 145, 178, 255)),
    ("cyan-8", Color::new(14, 116, 144, 255)),
    ("cyan-9", Color::new(21, 94, 117, 255)),
    ("cyan-10", Color::new(12, 74, 90, 255)),

    // SKY
    ("sky-1", Color::new(240, 249, 255, 255)),
    ("sky-2", Color::new(224, 242, 254, 255)),
    ("sky-3", Color::new(186, 230, 253, 255)),
    ("sky-4", Color::new(125, 211, 252, 255)),
    ("sky-5", Color::new(56, 189, 248, 255)),
    ("sky-6", Color::new(14, 165, 233, 255)),
    ("sky-7", Color::new(2, 132, 199, 255)),
    ("sky-8", Color::new(3, 105, 161, 255)),
    ("sky-9", Color::new(7, 89, 133, 255)),
    ("sky-10", Color::new(12, 74, 110, 255)),

    // BLUE
    ("blue-1", Color::new(239, 246, 255, 255)),
    ("blue-2", Color::new(219, 234, 254, 255)),
    ("blue-3", Color::new(191, 219, 254, 255)),
    ("blue-4", Color::new(147, 197, 253, 255)),
    ("blue-5", Color::new(96, 165, 250, 255)),
    ("blue-6", Color::new(59, 130, 246, 255)),
    ("blue-7", Color::new(37, 99, 235, 255)),
    ("blue-8", Color::new(29, 78, 216, 255)),
    ("blue-9", Color::new(30, 64, 175, 255)),
    ("blue-10", Color::new(30, 58, 138, 255)),

    // INDIGO
    ("indigo-1", Color::new(238, 242, 255, 255)),
    ("indigo-2", Color::new(224, 231, 255, 255)),
    ("indigo-3", Color::new(199, 210, 254, 255)),
    ("indigo-4", Color::new(165, 180, 252, 255)),
    ("indigo-5", Color::new(129, 140, 248, 255)),
    ("indigo-6", Color::new(99, 102, 241, 255)),
    ("indigo-7", Color::new(79, 70, 229, 255)),
    ("indigo-8", Color::new(67, 56, 202, 255)),
    ("indigo-9", Color::new(55, 48, 163, 255)),
    ("indigo-10", Color::new(49, 46, 129, 255)),

    // VIOLET
    ("violet-1", Color::new(245, 243, 255, 255)),
    ("violet-2", Color::new(237, 233, 254, 255)),
    ("violet-3", Color::new(221, 214, 254, 255)),
    ("violet-4", Color::new(196, 181, 253, 255)),
    ("violet-5", Color::new(167, 139, 250, 255)),
    ("violet-6", Color::new(139, 92, 246, 255)),
    ("violet-7", Color::new(124, 58, 237, 255)),
    ("violet-8", Color::new(109, 40, 217, 255)),
    ("violet-9", Color::new(91, 33, 182, 255)),
    ("violet-10", Color::new(76, 29, 149, 255)),

    // PURPLE
    ("purple-1", Color::new(250, 245, 255, 255)),
    ("purple-2", Color::new(243, 232, 255, 255)),
    ("purple-3", Color::new(233, 213, 255, 255)),
    ("purple-4", Color::new(216, 180, 254, 255)),
    ("purple-5", Color::new(192, 132, 252, 255)),
    ("purple-6", Color::new(168, 85, 247, 255)),
    ("purple-7", Color::new(147, 51, 234, 255)),
    ("purple-8", Color::new(126, 34, 206, 255)),
    ("purple-9", Color::new(107, 33, 168, 255)),
    ("purple-10", Color::new(88, 28, 135, 255)),

    // FUCHSIA
    ("fuchsia-1", Color::new(253, 244, 255, 255)),
    ("fuchsia-2", Color::new(250, 232, 254, 255)),
    ("fuchsia-3", Color::new(245, 208, 254, 255)),
    ("fuchsia-4", Color::new(240, 171, 252, 255)),
    ("fuchsia-5", Color::new(232, 121, 249, 255)),
    ("fuchsia-6", Color::new(217, 70, 211, 255)),
    ("fuchsia-7", Color::new(192, 38, 211, 255)),
    ("fuchsia-8", Color::new(162, 28, 175, 255)),
    ("fuchsia-9", Color::new(134, 25, 143, 255)),
    ("fuchsia-10", Color::new(112, 26, 117, 255)),

    // PINK
    ("pink-1", Color::new(253, 242, 248, 255)),
    ("pink-2", Color::new(252, 231, 243, 255)),
    ("pink-3", Color::new(251, 207, 232, 255)),
    ("pink-4", Color::new(249, 168, 212, 255)),
    ("pink-5", Color::new(244, 114, 182, 255)),
    ("pink-6", Color::new(236, 72, 153, 255)),
    ("pink-7", Color::new(219, 39, 119, 255)),
    ("pink-8", Color::new(190, 24, 93, 255)),
    ("pink-9", Color::new(157, 23, 77, 255)),
    ("pink-10", Color::new(131, 19, 67, 255)),

    // ROSE
    ("rose-1", Color::new(255, 241, 242, 255)),
    ("rose-2", Color::new(255, 228, 230, 255)),
    ("rose-3", Color::new(254, 205, 211, 255)),
    ("rose-4", Color::new(253, 164, 175, 255)),
    ("rose-5", Color::new(251, 113, 133, 255)),
    ("rose-6", Color::new(244, 63, 94, 255)),
    ("rose-7", Color::new(225, 29, 72, 255)),
    ("rose-8", Color::new(190, 18, 60, 255)),
    ("rose-9", Color::new(159, 18, 57, 255)),
    ("rose-10", Color::new(136, 19, 55, 255)),

    // RUBY (deep jewel red)
("ruby-1",  Color::new(255, 235, 238, 255)),
("ruby-2",  Color::new(250, 200, 205, 255)),
("ruby-3",  Color::new(240, 160, 170, 255)),
("ruby-4",  Color::new(225, 120, 140, 255)),
("ruby-5",  Color::new(210, 80, 110, 255)),
("ruby-6",  Color::new(185, 50, 85, 255)),
("ruby-7",  Color::new(160, 35, 65, 255)),
("ruby-8",  Color::new(130, 25, 50, 255)),
("ruby-9",  Color::new(100, 20, 40, 255)),
("ruby-10", Color::new(75, 15, 30, 255)),

// GOLD (rich metallic)
("gold-1",  Color::new(255, 250, 230, 255)),
("gold-2",  Color::new(255, 240, 200, 255)),
("gold-3",  Color::new(250, 225, 160, 255)),
("gold-4",  Color::new(240, 210, 120, 255)),
("gold-5",  Color::new(230, 190, 80, 255)),
("gold-6",  Color::new(210, 160, 50, 255)),
("gold-7",  Color::new(180, 130, 30, 255)),
("gold-8",  Color::new(150, 100, 20, 255)),
("gold-9",  Color::new(120, 80, 15, 255)),
("gold-10", Color::new(90, 60, 10, 255)),

// FOREST (deep natural green)
("forest-1",  Color::new(230, 240, 230, 255)),
("forest-2",  Color::new(200, 225, 200, 255)),
("forest-3",  Color::new(170, 210, 170, 255)),
("forest-4",  Color::new(130, 180, 140, 255)),
("forest-5",  Color::new(90, 150, 110, 255)),
("forest-6",  Color::new(60, 120, 85, 255)),
("forest-7",  Color::new(45, 95, 65, 255)),
("forest-8",  Color::new(35, 75, 50, 255)),
("forest-9",  Color::new(25, 55, 35, 255)),
("forest-10", Color::new(15, 35, 25, 255)),

// SEA (Mediterranean teal-blue)
("sea-1",  Color::new(220, 245, 245, 255)),
("sea-2",  Color::new(190, 230, 235, 255)),
("sea-3",  Color::new(150, 210, 220, 255)),
("sea-4",  Color::new(110, 185, 200, 255)),
("sea-5",  Color::new(70, 160, 180, 255)),
("sea-6",  Color::new(50, 135, 155, 255)),
("sea-7",  Color::new(35, 110, 130, 255)),
("sea-8",  Color::new(25, 85, 105, 255)),
("sea-9",  Color::new(18, 65, 80, 255)),
("sea-10", Color::new(12, 45, 60, 255)),

// SAPPHIRE (deep gemstone blue)
("sapphire-1",  Color::new(235, 240, 255, 255)),
("sapphire-2",  Color::new(200, 220, 250, 255)),
("sapphire-3",  Color::new(160, 200, 245, 255)),
("sapphire-4",  Color::new(120, 175, 240, 255)),
("sapphire-5",  Color::new(80, 150, 230, 255)),
("sapphire-6",  Color::new(50, 120, 210, 255)),
("sapphire-7",  Color::new(35, 95, 180, 255)),
("sapphire-8",  Color::new(25, 75, 150, 255)),
("sapphire-9",  Color::new(18, 55, 120, 255)),
("sapphire-10", Color::new(12, 40, 90, 255)),

// OCEAN (deep blue-green gradient)
("ocean-1",  Color::new(225, 245, 245, 255)),
("ocean-2",  Color::new(190, 230, 235, 255)),
("ocean-3",  Color::new(150, 215, 225, 255)),
("ocean-4",  Color::new(110, 190, 210, 255)),
("ocean-5",  Color::new(70, 165, 190, 255)),
("ocean-6",  Color::new(50, 140, 165, 255)),
("ocean-7",  Color::new(35, 115, 140, 255)),
("ocean-8",  Color::new(25, 90, 115, 255)),
("ocean-9",  Color::new(18, 65, 90, 255)),
("ocean-10", Color::new(12, 45, 70, 255)),
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