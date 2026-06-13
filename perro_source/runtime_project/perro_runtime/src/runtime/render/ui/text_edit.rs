use super::*;

pub(in crate::runtime::render_ui) fn text_edit_command(ctx: TextEditCommandCtx<'_>) -> UiCommand {
    let TextEditCommandCtx {
        command:
            UiCommandCtx {
                node,
                rect,
                clip_rect,
                scale,
                virtual_font_scale,
                modulate,
            },
        edit,
        multiline,
        focused,
    } = ctx;
    let focused_style = &edit.focused_style;
    let style = &edit.style;
    let style_scale = ui_style_scale(scale);
    let font_scale = ui_font_scale(scale);
    UiCommand::UpsertTextEdit {
        node,
        rect,
        clip_rect,
        fill: if focused {
            Runtime::color_modulate_rgba(focused_style.fill.to_rgba(), modulate)
        } else {
            Runtime::color_modulate_rgba(style.fill.to_rgba(), modulate)
        },
        stroke: if focused {
            Runtime::color_modulate_rgba(focused_style.stroke.to_rgba(), modulate)
        } else {
            Runtime::color_modulate_rgba(style.stroke.to_rgba(), modulate)
        },
        stroke_width: if focused {
            focused_style.stroke_width
        } else {
            style.stroke_width
        } * style_scale,
        corner_radius: if focused {
            focused_style.corner_radius
        } else {
            style.corner_radius
        },
        shadow: ui_depth_effect_state(
            if focused {
                focused_style.shadow
            } else {
                style.shadow
            },
            style_scale,
        ),
        highlight: ui_depth_effect_state(
            if focused {
                focused_style.highlight
            } else {
                style.highlight
            },
            style_scale,
        ),
        text: Cow::Owned(edit.text.to_string()),
        placeholder: Cow::Owned(edit.placeholder.to_string()),
        color: Runtime::color_modulate(edit.color, modulate),
        placeholder_color: Runtime::color_modulate(edit.placeholder_color, modulate),
        selection_color: Runtime::color_modulate(edit.selection_color, modulate),
        caret_color: Runtime::color_modulate(edit.caret_color, modulate),
        font_size: {
            let (base, node_scale) =
                if let Some(px) = text_size_from_rect_ratio(rect.size, edit.text_size_ratio) {
                    (px, 1.0)
                } else {
                    (fallback_text_size(edit.font_size), font_scale)
                };
            resolve_font_size(base, node_scale, virtual_font_scale, edit.font_sizing)
        },
        padding: [
            edit.padding.left * scale.x.abs().max(0.0001),
            edit.padding.top * scale.y.abs().max(0.0001),
            edit.padding.right * scale.x.abs().max(0.0001),
            edit.padding.bottom * scale.y.abs().max(0.0001),
        ],
        scroll: [edit.h_scroll, edit.v_scroll],
        caret: edit.caret,
        anchor: edit.anchor,
        focused,
        multiline,
    }
}

pub(in crate::runtime::render_ui) fn resolve_font_size(
    font_size: f32,
    node_font_scale: f32,
    virtual_font_scale: f32,
    sizing: UiFontSizing,
) -> f32 {
    let viewport_scale = if sizing.relative_to_virtual {
        sizing.clamp_scale(virtual_font_scale)
    } else {
        1.0
    };
    font_size * node_font_scale * viewport_scale
}

pub(in crate::runtime::render_ui) fn fallback_text_size(font_size: f32) -> f32 {
    if font_size.is_finite() && font_size > 0.0 {
        font_size
    } else {
        16.0
    }
}

pub(in crate::runtime::render_ui) fn text_size_from_rect_ratio(
    rect_size: [f32; 2],
    ratio: f32,
) -> Option<f32> {
    if !ratio.is_finite() || ratio <= 0.0 {
        return None;
    }
    Some((rect_size[1].max(1.0) * ratio).max(1.0))
}

pub(in crate::runtime::render_ui) fn panel_command(
    node: NodeID,
    rect: UiRectState,
    clip_rect: [f32; 4],
    scale: Vector2,
    style: &UiStyle,
    modulate: Color,
) -> UiCommand {
    let style_scale = ui_style_scale(scale);
    UiCommand::UpsertPanel {
        node,
        rect,
        clip_rect,
        fill: Runtime::color_modulate_rgba(style.fill.to_rgba(), modulate),
        stroke: Runtime::color_modulate_rgba(style.stroke.to_rgba(), modulate),
        stroke_width: style.stroke_width * style_scale,
        corner_radius: style.corner_radius,
        shadow: ui_depth_effect_state(style.shadow, style_scale),
        highlight: ui_depth_effect_state(style.highlight, style_scale),
    }
}

pub(in crate::runtime::render_ui) fn ui_depth_effect_state(
    effect: perro_ui::UiDepthEffect,
    scale: f32,
) -> UiDepthEffectState {
    UiDepthEffectState {
        color: effect.color,
        distance: effect.distance * scale,
        falloff: effect.falloff * scale,
        vector: [effect.vector.x, effect.vector.y],
        size: effect.size,
    }
}

pub(in crate::runtime::render_ui) fn viewport_clip_rect(viewport: Vector2) -> [f32; 4] {
    [0.0, 0.0, viewport.x.max(1.0), viewport.y.max(1.0)]
}

pub(in crate::runtime::render_ui) fn rect_to_screen_clip(
    rect: ComputedUiRect,
    viewport: Vector2,
) -> [f32; 4] {
    let cx = viewport.x * 0.5 + rect.center.x;
    let cy = viewport.y * 0.5 - rect.center.y;
    let hx = rect.size.x * 0.5;
    let hy = rect.size.y * 0.5;
    [cx - hx, cy - hy, cx + hx, cy + hy]
}

pub(in crate::runtime::render_ui) fn intersect_clip_rect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let min_x = a[0].max(b[0]);
    let min_y = a[1].max(b[1]);
    let max_x = a[2].min(b[2]);
    let max_y = a[3].min(b[3]);
    [min_x, min_y, max_x.max(min_x), max_y.max(min_y)]
}

pub(in crate::runtime::render_ui) fn ui_command_clip_rect(command: &UiCommand) -> [f32; 4] {
    match command {
        UiCommand::UpsertPanel { clip_rect, .. }
        | UiCommand::UpsertShape { clip_rect, .. }
        | UiCommand::UpsertButton { clip_rect, .. }
        | UiCommand::UpsertCheckbox { clip_rect, .. }
        | UiCommand::UpsertLabel { clip_rect, .. }
        | UiCommand::UpsertImage { clip_rect, .. }
        | UiCommand::UpsertNineSlice { clip_rect, .. }
        | UiCommand::UpsertTextEdit { clip_rect, .. } => *clip_rect,
        UiCommand::RemoveNode { .. } | UiCommand::Clear => [0.0, 0.0, 1.0, 1.0],
    }
}

pub(in crate::runtime::render_ui) fn text_edit_ref(data: &SceneNodeData) -> Option<&UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&node.inner),
        _ => None,
    }
}

pub(in crate::runtime::render_ui) fn text_edit_mut(
    data: &mut SceneNodeData,
) -> Option<&mut UiTextEdit> {
    match data {
        SceneNodeData::UiTextBox(node) => Some(&mut node.inner),
        SceneNodeData::UiTextBlock(node) => Some(&mut node.inner),
        _ => None,
    }
}

pub(in crate::runtime::render_ui) fn text_edit_keys() -> &'static [KeyCode] {
    &[
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::Enter,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::KeyA,
        KeyCode::KeyC,
        KeyCode::KeyV,
        KeyCode::KeyX,
    ]
}

pub(in crate::runtime::render_ui) fn repeatable_text_edit_keys() -> &'static [KeyCode] {
    &[
        KeyCode::Backspace,
        KeyCode::Delete,
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Home,
        KeyCode::End,
    ]
}

pub(in crate::runtime::render_ui) fn insert_text_input(edit: &mut UiTextEdit, text: &str) -> bool {
    if !edit.editable || text.is_empty() {
        return false;
    }
    let filtered = normalize_text_input(text, edit.multiline);
    if filtered.is_empty() {
        return false;
    }
    replace_selection(edit, &filtered);
    true
}

pub(in crate::runtime::render_ui) fn apply_text_edit_key_input(
    edit: &mut UiTextEdit,
    shift: bool,
    ctrl: bool,
    repeat_key: Option<KeyCode>,
    input: &perro_input_api::InputSnapshot,
) -> bool {
    let mut changed = false;
    if ctrl && input.is_key_pressed(KeyCode::KeyA) {
        edit.anchor = 0;
        edit.caret = edit.text.len();
        return true;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyC) {
        copy_selection_to_clipboard(edit);
        return false;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyX) {
        if edit.editable && copy_selection_to_clipboard(edit) {
            replace_selection(edit, "");
            return true;
        }
        return false;
    }
    if ctrl && input.is_key_pressed(KeyCode::KeyV) {
        if edit.editable
            && let Some(text) = read_clipboard_text(edit.multiline)
        {
            replace_selection(edit, &text);
            return true;
        }
        return false;
    }
    if repeat_key == Some(KeyCode::Backspace) && edit.editable {
        changed |= backspace(edit);
    }
    if repeat_key == Some(KeyCode::Delete) && edit.editable {
        changed |= delete(edit);
    }
    if input.is_key_pressed(KeyCode::Enter) && edit.editable && edit.multiline {
        replace_selection(edit, "\n");
        changed = true;
    }
    if repeat_key == Some(KeyCode::ArrowLeft) {
        move_caret(edit, prev_char(edit.text.as_ref(), edit.caret), shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::ArrowRight) {
        move_caret(edit, next_char(edit.text.as_ref(), edit.caret), shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::Home) {
        let line = line_for_index(edit.text.as_ref(), edit.caret);
        move_caret(edit, line.start, shift);
        changed = true;
    }
    if repeat_key == Some(KeyCode::End) {
        let line = line_for_index(edit.text.as_ref(), edit.caret);
        move_caret(edit, line.end, shift);
        changed = true;
    }
    if edit.multiline && repeat_key == Some(KeyCode::ArrowUp) {
        move_vertical(edit, -1, shift);
        changed = true;
    }
    if edit.multiline && repeat_key == Some(KeyCode::ArrowDown) {
        move_vertical(edit, 1, shift);
        changed = true;
    }
    changed
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(in crate::runtime::render_ui) fn copy_selection_to_clipboard(edit: &UiTextEdit) -> bool {
    let (start, end) = selection_range(edit);
    if start == end {
        return false;
    }
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    clipboard
        .set_text(edit.text[start..end].to_string())
        .is_ok()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub(in crate::runtime::render_ui) fn copy_selection_to_clipboard(_edit: &UiTextEdit) -> bool {
    false
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub(in crate::runtime::render_ui) fn read_clipboard_text(multiline: bool) -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    let text = normalize_text_input(&text, multiline);
    (!text.is_empty()).then_some(text)
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
pub(in crate::runtime::render_ui) fn read_clipboard_text(_multiline: bool) -> Option<String> {
    None
}

pub(in crate::runtime::render_ui) fn replace_selection(edit: &mut UiTextEdit, replacement: &str) {
    let mut text = edit.text.to_string();
    let (start, end) = selection_range(edit);
    text.replace_range(start..end, replacement);
    let caret = start + replacement.len();
    edit.text = Cow::Owned(text);
    edit.caret = caret;
    edit.anchor = caret;
}

pub(in crate::runtime::render_ui) fn selection_range(edit: &UiTextEdit) -> (usize, usize) {
    let text = edit.text.as_ref();
    let a = clamp_char_boundary(text, edit.anchor);
    let b = clamp_char_boundary(text, edit.caret);
    if a <= b { (a, b) } else { (b, a) }
}

pub(in crate::runtime::render_ui) fn backspace(edit: &mut UiTextEdit) -> bool {
    if edit.caret != edit.anchor {
        replace_selection(edit, "");
        return true;
    }
    let prev = prev_char(edit.text.as_ref(), edit.caret);
    if prev == edit.caret {
        return false;
    }
    let mut text = edit.text.to_string();
    text.replace_range(prev..edit.caret, "");
    edit.text = Cow::Owned(text);
    edit.caret = prev;
    edit.anchor = prev;
    true
}

pub(in crate::runtime::render_ui) fn delete(edit: &mut UiTextEdit) -> bool {
    if edit.caret != edit.anchor {
        replace_selection(edit, "");
        return true;
    }
    let next = next_char(edit.text.as_ref(), edit.caret);
    if next == edit.caret {
        return false;
    }
    let mut text = edit.text.to_string();
    text.replace_range(edit.caret..next, "");
    edit.text = Cow::Owned(text);
    edit.anchor = edit.caret;
    true
}

pub(in crate::runtime::render_ui) fn move_caret(edit: &mut UiTextEdit, index: usize, extend: bool) {
    let index = clamp_char_boundary(edit.text.as_ref(), index);
    edit.caret = index;
    if !extend {
        edit.anchor = index;
    }
}

pub(in crate::runtime::render_ui) fn move_vertical(
    edit: &mut UiTextEdit,
    delta: i32,
    extend: bool,
) {
    let text = edit.text.as_ref();
    let lines = text_line_ranges(text);
    let Some(current_line) = lines
        .iter()
        .position(|line| edit.caret >= line.start && edit.caret <= line.end)
    else {
        return;
    };
    let target_line = (current_line as i32 + delta).clamp(0, lines.len() as i32 - 1) as usize;
    let col = text[lines[current_line].start..edit.caret].chars().count();
    let target = index_at_col(text, lines[target_line], col);
    move_caret(edit, target, extend);
}

pub(in crate::runtime::render_ui) fn ensure_caret_visible(
    edit: &mut UiTextEdit,
    rect: Option<ComputedUiRect>,
) {
    let Some(rect) = rect else {
        return;
    };
    let content_w = (rect.size.x - edit.padding.horizontal()).max(1.0);
    let content_h = (rect.size.y - edit.padding.vertical()).max(1.0);
    let caret_pos = caret_text_pos(edit);
    let line_h = text_line_height(edit);
    if caret_pos.x < edit.h_scroll {
        edit.h_scroll = caret_pos.x.max(0.0);
    } else if caret_pos.x + 2.0 > edit.h_scroll + content_w {
        edit.h_scroll = (caret_pos.x + 2.0 - content_w).max(0.0);
    }
    if edit.multiline {
        if caret_pos.y < edit.v_scroll {
            edit.v_scroll = caret_pos.y.max(0.0);
        } else if caret_pos.y + line_h > edit.v_scroll + content_h {
            edit.v_scroll = (caret_pos.y + line_h - content_h).max(0.0);
        }
    } else {
        edit.v_scroll = 0.0;
    }
}

pub(in crate::runtime::render_ui) fn text_index_from_local(
    edit: &UiTextEdit,
    local: Vector2,
) -> usize {
    let lines = text_line_ranges(edit.text.as_ref());
    let line_h = text_line_height(edit);
    let char_w = text_char_width(edit);
    let line_idx = if edit.multiline {
        ((local.y / line_h).floor() as isize).clamp(0, lines.len() as isize - 1) as usize
    } else {
        0
    };
    let col = ((local.x / char_w).round() as isize).max(0) as usize;
    index_at_col(edit.text.as_ref(), lines[line_idx], col)
}

pub(in crate::runtime::render_ui) fn caret_text_pos(edit: &UiTextEdit) -> Vector2 {
    let text = edit.text.as_ref();
    let lines = text_line_ranges(text);
    let mut line_idx = 0usize;
    let mut line = lines[0];
    for (idx, candidate) in lines.iter().copied().enumerate() {
        if edit.caret >= candidate.start && edit.caret <= candidate.end {
            line_idx = idx;
            line = candidate;
            break;
        }
    }
    let col = text[line.start..edit.caret.min(line.end)].chars().count();
    Vector2::new(
        col as f32 * text_char_width(edit),
        line_idx as f32 * text_line_height(edit),
    )
}

#[derive(Clone, Copy)]
pub(in crate::runtime::render_ui) struct TextRange {
    pub(in crate::runtime::render_ui) start: usize,
    pub(in crate::runtime::render_ui) end: usize,
}

pub(in crate::runtime::render_ui) fn line_for_index(text: &str, index: usize) -> TextRange {
    text_line_ranges(text)
        .into_iter()
        .find(|line| index >= line.start && index <= line.end)
        .unwrap_or(TextRange {
            start: 0,
            end: text.len(),
        })
}

pub(in crate::runtime::render_ui) fn text_line_ranges(text: &str) -> Vec<TextRange> {
    if text.is_empty() {
        return vec![TextRange { start: 0, end: 0 }];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            out.push(TextRange { start, end: idx });
            start = idx + ch.len_utf8();
        }
    }
    out.push(TextRange {
        start,
        end: text.len(),
    });
    out
}

pub(in crate::runtime::render_ui) fn normalize_text_input(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.replace(['\r', '\n', '\t'], " ")
    }
}

pub(in crate::runtime::render_ui) fn index_at_col(
    text: &str,
    line: TextRange,
    col: usize,
) -> usize {
    for (count, (idx, _)) in text[line.start..line.end].char_indices().enumerate() {
        if count == col {
            return line.start + idx;
        }
    }
    line.end
}

pub(in crate::runtime::render_ui) fn prev_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[..index]
        .char_indices()
        .last()
        .map(|(idx, _)| idx)
        .unwrap_or(index)
}

pub(in crate::runtime::render_ui) fn next_char(text: &str, index: usize) -> usize {
    let index = clamp_char_boundary(text, index);
    text[index..]
        .chars()
        .next()
        .map(|ch| index + ch.len_utf8())
        .unwrap_or(index)
}

pub(in crate::runtime::render_ui) fn clamp_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub(in crate::runtime::render_ui) fn text_char_width(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 0.6).max(1.0)
}

pub(in crate::runtime::render_ui) fn text_line_height(edit: &UiTextEdit) -> f32 {
    (edit.font_size * 1.25).max(1.0)
}
