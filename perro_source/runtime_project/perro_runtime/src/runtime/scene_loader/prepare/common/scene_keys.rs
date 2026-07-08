const COLOR_MODULATE_KEYS: &[&str] = &["tint", "color", "modulate"];
const TEXT_COLOR_KEYS: &[&str] = &["color", "text_color", "modulate", "tint"];
const TEXTURE_REGION_KEYS: &[&str] = &["texture_region", "region", "atlas_region"];
const FLIP_X_KEYS: &[&str] = &["flip_x", "flip_h", "mirror_x"];
const FLIP_Y_KEYS: &[&str] = &["flip_y", "flip_v", "mirror_y"];
const HOVER_ENTER_SIGNAL_KEYS: &[&str] =
    &["hover_signals", "hovered_signals", "hover_enter_signals"];
const HOVER_EXIT_SIGNAL_KEYS: &[&str] =
    &["hover_exit_signals", "unhover_signals", "unhovered_signals"];

#[inline]
fn scene_key_in(name: &str, keys: &[&str]) -> bool {
    keys.contains(&name)
}
