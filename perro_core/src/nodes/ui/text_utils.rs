use unicode_segmentation::UnicodeSegmentation;

pub fn line_height(font_size: f32) -> f32 {
    // Simple line height calculation (egui handles this natively)
    font_size * 1.2
}

pub fn grapheme_count(text: &str) -> usize {
    UnicodeSegmentation::graphemes(text, true).count()
}

pub fn grapheme_byte_index(text: &str, grapheme_index: usize) -> usize {
    if grapheme_index == 0 {
        return 0;
    }

    let mut count = 0;
    for (byte_idx, _) in UnicodeSegmentation::grapheme_indices(text, true) {
        if count == grapheme_index {
            return byte_idx;
        }
        count += 1;
    }

    text.len()
}

pub fn byte_range_for_graphemes(text: &str, start: usize, end: usize) -> (usize, usize) {
    let start_byte = grapheme_byte_index(text, start);
    let end_byte = grapheme_byte_index(text, end);
    (start_byte, end_byte)
}

pub fn delete_grapheme_range(text: &mut String, start: usize, end: usize) {
    let (start_byte, end_byte) = byte_range_for_graphemes(text, start, end);
    if start_byte < end_byte && end_byte <= text.len() {
        text.replace_range(start_byte..end_byte, "");
    }
}

pub fn grapheme_slice(text: &str, start: usize, end: usize) -> String {
    if start >= end {
        return String::new();
    }

    let mut out = String::new();
    let mut index = 0;
    for grapheme in UnicodeSegmentation::graphemes(text, true) {
        if index >= start && index < end {
            out.push_str(grapheme);
        }
        if index >= end {
            break;
        }
        index += 1;
    }

    out
}

pub fn grapheme_positions(text: &str, font_size: f32) -> Vec<f32> {
    // Simple fallback for grapheme positions (egui handles accurate positioning natively)
    // This is only used for cursor positioning in the old text input system
    // When using egui, cursor positioning is handled automatically
    let mut positions = Vec::new();
    let fallback_width = font_size * 0.6;
    let mut cumulative_width = 0.0;
    for _ in UnicodeSegmentation::graphemes(text, true) {
        cumulative_width += fallback_width;
        positions.push(cumulative_width);
    }
    positions
}
