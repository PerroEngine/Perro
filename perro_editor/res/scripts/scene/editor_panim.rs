// .panim text model for the animation editor.
// Parses clip text into objects/tracks/keys, supports key edits, and
// serializes back to canonical .panim text. Values stay as raw text so
// anything the runtime parser accepts round-trips unchanged.

#[derive(Clone, Debug, PartialEq)]
pub struct PanimKey {
    pub frame: u32,
    pub open: bool,
    pub value: String,
    pub interp: Option<String>,
    pub ease: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanimTrack {
    pub object: String,
    pub field: String,
    pub keys: Vec<PanimKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanimEvent {
    pub frame: u32,
    pub object: Option<String>,
    pub line: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PanimDoc {
    pub name: String,
    pub fps: f32,
    pub default_interp: String,
    pub default_ease: String,
    pub vars: Vec<String>,
    pub objects: Vec<(String, String)>,
    pub tracks: Vec<PanimTrack>,
    pub events: Vec<PanimEvent>,
}

impl Default for PanimDoc {
    fn default() -> Self {
        Self {
            name: "Animation".to_string(),
            fps: 60.0,
            default_interp: "interpolate".to_string(),
            default_ease: "linear".to_string(),
            vars: Vec::new(),
            objects: Vec::new(),
            tracks: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl PanimDoc {
    pub fn total_frames(&self) -> u32 {
        let mut max = 0u32;
        for track in &self.tracks {
            for key in &track.keys {
                max = max.max(key.frame);
            }
        }
        for event in &self.events {
            max = max.max(event.frame);
        }
        max + 1
    }

    pub fn object_type(&self, object: &str) -> Option<&str> {
        self.objects
            .iter()
            .find(|(name, _)| name == object)
            .map(|(_, ty)| ty.as_str())
    }

    pub fn track_index(&self, object: &str, field: &str) -> Option<usize> {
        self.tracks
            .iter()
            .position(|track| track.object == object && track.field == field)
    }

    pub fn ensure_object(&mut self, object: &str, node_type: &str) {
        if self.object_type(object).is_none() {
            self.objects
                .push((object.to_string(), node_type.to_string()));
        }
    }

    pub fn ensure_track(&mut self, object: &str, field: &str) -> usize {
        if let Some(idx) = self.track_index(object, field) {
            return idx;
        }
        self.tracks.push(PanimTrack {
            object: object.to_string(),
            field: field.to_string(),
            keys: Vec::new(),
        });
        self.tracks.len() - 1
    }

    pub fn set_key(&mut self, object: &str, field: &str, frame: u32, value: String) {
        let idx = self.ensure_track(object, field);
        let keys = &mut self.tracks[idx].keys;
        match keys.binary_search_by_key(&frame, |key| key.frame) {
            Ok(pos) => {
                keys[pos].value = value;
                keys[pos].open = false;
            }
            Err(pos) => keys.insert(
                pos,
                PanimKey {
                    frame,
                    open: false,
                    value,
                    interp: None,
                    ease: None,
                },
            ),
        }
    }

    pub fn remove_key(&mut self, object: &str, field: &str, frame: u32) -> bool {
        let Some(idx) = self.track_index(object, field) else {
            return false;
        };
        let keys = &mut self.tracks[idx].keys;
        match keys.binary_search_by_key(&frame, |key| key.frame) {
            Ok(pos) => {
                keys.remove(pos);
                true
            }
            Err(_) => false,
        }
    }

    pub fn remove_track(&mut self, object: &str, field: &str) -> bool {
        let Some(idx) = self.track_index(object, field) else {
            return false;
        };
        self.tracks.remove(idx);
        let object_used = self.tracks.iter().any(|track| track.object == object)
            || self
                .events
                .iter()
                .any(|event| event.object.as_deref() == Some(object));
        if !object_used {
            self.objects.retain(|(name, _)| name != object);
        }
        true
    }

    pub fn key_near(&self, track: usize, frame: u32) -> Option<u32> {
        let keys = &self.tracks.get(track)?.keys;
        keys.iter()
            .map(|key| key.frame)
            .min_by_key(|key_frame| key_frame.abs_diff(frame))
    }
}

pub fn parse_panim(text: &str) -> PanimDoc {
    let mut doc = PanimDoc {
        name: String::new(),
        ..PanimDoc::default()
    };
    let mut section = Section::Top;
    let mut lines = LogicalLines::new(text);
    while let Some(line) = lines.next_line() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        match section {
            Section::Top => {
                if trimmed == "[Animation]" {
                    section = Section::Animation;
                } else if trimmed == "[Objects]" {
                    section = Section::Objects;
                } else if let Some((frame, open)) = parse_frame_header(trimmed) {
                    section = Section::Frame { frame, open };
                } else if trimmed.starts_with('@') && trimmed.contains('=') {
                    doc.vars.push(trimmed.to_string());
                }
            }
            Section::Animation => {
                if trimmed == "[/Animation]" {
                    section = Section::Top;
                } else if let Some((key, value)) = split_kv(trimmed) {
                    match key {
                        "name" => doc.name = unquote(value).to_string(),
                        "fps" => {
                            if let Ok(fps) = value.parse::<f32>()
                                && fps > 0.0
                            {
                                doc.fps = fps;
                            }
                        }
                        "default_interp" | "default_interpolation" => {
                            doc.default_interp = unquote(value).to_string();
                        }
                        "default_ease" | "default_easing" => {
                            doc.default_ease = unquote(value).to_string();
                        }
                        _ => {}
                    }
                }
            }
            Section::Objects => {
                if trimmed == "[/Objects]" {
                    section = Section::Top;
                } else if let Some((key, value)) = split_kv(trimmed) {
                    doc.objects.push((key.to_string(), value.to_string()));
                }
            }
            Section::Frame { frame, open } => {
                if trimmed.starts_with("[/Frame") {
                    section = Section::Top;
                } else if let Some(object) = trimmed
                    .strip_prefix('@')
                    .and_then(|rest| rest.strip_suffix('{'))
                {
                    section = Section::FrameObject {
                        frame,
                        open,
                        object: object.trim().to_string(),
                    };
                } else if is_event_line(trimmed) {
                    doc.events.push(PanimEvent {
                        frame,
                        object: None,
                        line: trimmed.to_string(),
                    });
                }
            }
            Section::FrameObject {
                frame,
                open,
                ref object,
            } => {
                if trimmed == "}" {
                    section = Section::Frame { frame, open };
                } else if trimmed.starts_with("[/Frame") {
                    section = Section::Top;
                } else if is_event_line(trimmed) {
                    doc.events.push(PanimEvent {
                        frame,
                        object: Some(object.clone()),
                        line: trimmed.to_string(),
                    });
                } else if let Some((key, value)) = split_kv(trimmed) {
                    let object = object.clone();
                    if let Some(field) = key.strip_suffix(".interp") {
                        set_key_control(&mut doc, &object, field, frame, open, |k| {
                            k.interp = Some(unquote(value).to_string());
                        });
                    } else if let Some(field) = key.strip_suffix(".ease") {
                        set_key_control(&mut doc, &object, field, frame, open, |k| {
                            k.ease = Some(unquote(value).to_string());
                        });
                    } else {
                        let idx = doc.ensure_track(&object, key);
                        let keys = &mut doc.tracks[idx].keys;
                        match keys.binary_search_by_key(&frame, |k| k.frame) {
                            Ok(pos) => {
                                keys[pos].value = value.to_string();
                                keys[pos].open = open;
                            }
                            Err(pos) => keys.insert(
                                pos,
                                PanimKey {
                                    frame,
                                    open,
                                    value: value.to_string(),
                                    interp: None,
                                    ease: None,
                                },
                            ),
                        }
                    }
                }
            }
        }
    }
    if doc.name.is_empty() {
        doc.name = "Animation".to_string();
    }
    doc
}

pub fn serialize_panim(doc: &PanimDoc) -> String {
    let mut out = String::with_capacity(512);
    out.push_str("[Animation]\n");
    out.push_str(&format!("name = \"{}\"\n", doc.name));
    let fps = doc.fps;
    if (fps - fps.round()).abs() < 1.0e-4 {
        out.push_str(&format!("fps = {}\n", fps.round() as u32));
    } else {
        out.push_str(&format!("fps = {fps}\n"));
    }
    out.push_str(&format!("default_interp = \"{}\"\n", doc.default_interp));
    out.push_str(&format!("default_ease = \"{}\"\n", doc.default_ease));
    out.push_str("[/Animation]\n");
    if !doc.vars.is_empty() {
        out.push('\n');
        for var in &doc.vars {
            out.push_str(var);
            out.push('\n');
        }
    }
    out.push_str("\n[Objects]\n");
    for (name, node_type) in &doc.objects {
        out.push_str(&format!("{name} = {node_type}\n"));
    }
    out.push_str("[/Objects]\n");

    let mut frames: Vec<(u32, bool)> = Vec::new();
    for track in &doc.tracks {
        for key in &track.keys {
            if !frames.contains(&(key.frame, key.open)) {
                frames.push((key.frame, key.open));
            }
        }
    }
    for event in &doc.events {
        if !frames.iter().any(|(frame, open)| *frame == event.frame && !open) {
            frames.push((event.frame, false));
        }
    }
    // Closed block first when a frame has both closed and open keys.
    frames.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    for &(frame, open) in &frames {
        let marker = if open { "?" } else { "" };
        out.push_str(&format!("\n[Frame{frame}{marker}]\n"));
        for (object, _) in &doc.objects {
            let mut body = String::new();
            for track in &doc.tracks {
                if &track.object != object {
                    continue;
                }
                if let Ok(pos) = track.keys.binary_search_by_key(&frame, |k| k.frame)
                    && track.keys[pos].open == open
                {
                    let key = &track.keys[pos];
                    if let Some(interp) = &key.interp {
                        body.push_str(&format!("    {}.interp = \"{}\"\n", track.field, interp));
                    }
                    if let Some(ease) = &key.ease {
                        body.push_str(&format!("    {}.ease = \"{}\"\n", track.field, ease));
                    }
                    body.push_str(&format!("    {} = {}\n", track.field, key.value));
                }
            }
            if !open {
                for event in &doc.events {
                    if event.frame == frame && event.object.as_deref() == Some(object) {
                        body.push_str(&format!("    {}\n", event.line));
                    }
                }
            }
            if !body.is_empty() {
                out.push_str(&format!("@{object} {{\n{body}}}\n"));
            }
        }
        if !open {
            for event in &doc.events {
                if event.frame == frame && event.object.is_none() {
                    out.push_str(&format!("{}\n", event.line));
                }
            }
        }
        out.push_str(&format!("[/Frame{frame}]\n"));
    }
    out
}

// Default key value text when a field has no doc/scene value to copy.
pub fn default_field_value_text(object_type: &str, field: &str) -> &'static str {
    let two_d = object_type.contains("2D");
    // Bone pose sub-fields default like the matching transform component.
    if let Some((_, sub_field)) = parse_bone_field(field) {
        return default_field_value_text(object_type, sub_field);
    }
    match field {
        "position" => {
            if two_d {
                "(0, 0)"
            } else {
                "(0, 0, 0)"
            }
        }
        "rotation" => {
            if two_d {
                "0"
            } else {
                "(0, 0, 0, 1)"
            }
        }
        "rotation_deg" => {
            if two_d {
                "0"
            } else {
                "(0, 0, 0)"
            }
        }
        "scale" => {
            if two_d {
                "(1, 1)"
            } else {
                "(1, 1, 1)"
            }
        }
        "visible" | "active" | "cast_shadows" => "true",
        "z_index" => "0",
        "zoom" | "intensity" | "shadow_strength" => "1",
        "color" => "\"#FFFFFF\"",
        _ => "0",
    }
}

// Animatable fields offered by the track picker, by node type name.
pub fn animatable_fields(node_type: &str) -> Vec<&'static str> {
    let mut fields: Vec<&'static str> = Vec::new();
    let two_d = node_type.contains("2D") && node_type != "Light2D";
    if two_d {
        fields.extend(["position", "rotation", "scale", "visible", "z_index"]);
    } else {
        fields.extend(["position", "rotation", "scale", "visible"]);
    }
    match node_type {
        "Sprite2D" => fields.push("texture"),
        "MeshInstance3D" => fields.extend(["mesh", "material"]),
        "Camera3D" => fields.extend([
            "zoom",
            "perspective_fovy_degrees",
            "orthographic_size",
            "active",
        ]),
        "PointLight3D" => fields.extend(["color", "intensity", "range", "active"]),
        "SpotLight3D" => fields.extend([
            "color",
            "intensity",
            "range",
            "inner_angle_radians",
            "outer_angle_radians",
            "active",
        ]),
        "RayLight3D" | "AmbientLight3D" => fields.extend(["color", "intensity", "active"]),
        _ => {}
    }
    fields
}

// Bone pose sub-fields a skeleton track can key. Order matches the pose
// editor rows in the inspector.
pub const BONE_TRACK_SUBFIELDS: [&str; 3] = ["position", "rotation", "scale"];

// Build a .panim track field path for a bone pose sub-field, e.g.
// `bones["Spine"].position`. Field names stay opaque strings in the text
// model, so this round-trips through parse/serialize unchanged.
pub fn bone_track_field(bone_name: &str, sub_field: &str) -> String {
    format!("bones[\"{bone_name}\"].{sub_field}")
}

// Split a bone track field path back into (bone_name, sub_field). Returns
// None for plain (non-bone) fields so callers fall back to scene-doc capture.
pub fn parse_bone_field(field: &str) -> Option<(String, &'static str)> {
    let rest = field.strip_prefix("bones[\"")?;
    let (name, tail) = rest.split_once("\"]")?;
    let sub = tail.strip_prefix('.')?;
    let sub = BONE_TRACK_SUBFIELDS.iter().copied().find(|item| *item == sub)?;
    Some((name.to_string(), sub))
}

enum Section {
    Top,
    Animation,
    Objects,
    Frame { frame: u32, open: bool },
    FrameObject { frame: u32, open: bool, object: String },
}

fn set_key_control(
    doc: &mut PanimDoc,
    object: &str,
    field: &str,
    frame: u32,
    open: bool,
    apply: impl FnOnce(&mut PanimKey),
) {
    let idx = doc.ensure_track(object, field);
    let keys = &mut doc.tracks[idx].keys;
    let pos = match keys.binary_search_by_key(&frame, |k| k.frame) {
        Ok(pos) => pos,
        Err(pos) => {
            keys.insert(
                pos,
                PanimKey {
                    frame,
                    open,
                    value: String::new(),
                    interp: None,
                    ease: None,
                },
            );
            pos
        }
    };
    apply(&mut keys[pos]);
}

fn parse_frame_header(line: &str) -> Option<(u32, bool)> {
    let inner = line.strip_prefix("[Frame")?.strip_suffix(']')?;
    if let Some(number) = inner.strip_suffix('?') {
        Some((number.parse().ok()?, true))
    } else {
        Some((inner.parse().ok()?, false))
    }
}

fn split_kv(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn is_event_line(line: &str) -> bool {
    let Some((key, _)) = line.split_once('=') else {
        return false;
    };
    matches!(key.trim(), "emit_signal" | "set_var" | "call_method")
}

fn unquote(text: &str) -> &str {
    text.trim().trim_matches('"')
}

// Joins physical lines until braces/parens/brackets balance so multi-line
// object values (e.g. `emit_signal = {` ... `}`) become one logical line.
struct LogicalLines<'a> {
    lines: std::str::Lines<'a>,
}

impl<'a> LogicalLines<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines(),
        }
    }

    fn next_line(&mut self) -> Option<String> {
        let first = self.lines.next()?;
        let trimmed = first.trim();
        // `@Object {` opens a block, not a value: pass through unjoined.
        if trimmed.starts_with('@') && trimmed.ends_with('{') && !trimmed.contains('=') {
            return Some(first.to_string());
        }
        let mut depth = bracket_depth(first);
        if depth <= 0 {
            return Some(first.to_string());
        }
        let mut joined = first.to_string();
        for line in self.lines.by_ref() {
            joined.push(' ');
            joined.push_str(line.trim());
            depth += bracket_depth(line);
            if depth <= 0 {
                break;
            }
        }
        Some(joined)
    }
}

fn bracket_depth(line: &str) -> i32 {
    let mut depth = 0;
    let mut in_string = false;
    for ch in line.chars() {
        match ch {
            '"' => in_string = !in_string,
            '{' | '(' | '[' if !in_string => depth += 1,
            '}' | ')' | ']' if !in_string => depth -= 1,
            _ => {}
        }
    }
    depth
}

#[cfg(test)]
mod panim_model_tests {
    use super::*;

    const SAMPLE: &str = r#"[Animation]
name = "RunForward"
fps = 60
default_interp = "interpolate"
default_ease = "linear"
[/Animation]

[Objects]
Hero = Node3D
MainCam = Camera3D
[/Objects]

[Frame0]
@Hero {
    position.ease = "ease_in"
    position = (0, 0, 0)
}
@MainCam {
    position = (0, 2, -1)
}
[/Frame0]

[Frame10]
@Hero {
    position = (3, 0, 0)
    call_method = { name="step", params=[0] }
}
emit_signal = { name="footfall", params=[1] }
[/Frame10]
"#;

    #[test]
    fn parse_reads_header_objects_tracks_events() {
        let doc = parse_panim(SAMPLE);
        assert_eq!(doc.name, "RunForward");
        assert_eq!(doc.fps, 60.0);
        assert_eq!(doc.objects.len(), 2);
        assert_eq!(doc.tracks.len(), 2);
        let hero = &doc.tracks[doc.track_index("Hero", "position").unwrap()];
        assert_eq!(hero.keys.len(), 2);
        assert_eq!(hero.keys[0].value, "(0, 0, 0)");
        assert_eq!(hero.keys[0].ease.as_deref(), Some("ease_in"));
        assert_eq!(hero.keys[1].frame, 10);
        assert_eq!(doc.events.len(), 2);
        assert_eq!(doc.total_frames(), 11);
    }

    #[test]
    fn serialize_round_trips() {
        let doc = parse_panim(SAMPLE);
        let text = serialize_panim(&doc);
        let doc2 = parse_panim(&text);
        assert_eq!(doc, doc2);
    }

    #[test]
    fn set_and_remove_key_keep_sorted_order() {
        let mut doc = parse_panim(SAMPLE);
        doc.set_key("Hero", "position", 5, "(1, 0, 0)".to_string());
        let idx = doc.track_index("Hero", "position").unwrap();
        let frames: Vec<u32> = doc.tracks[idx].keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0, 5, 10]);
        assert!(doc.remove_key("Hero", "position", 5));
        assert!(!doc.remove_key("Hero", "position", 5));
        let text = serialize_panim(&doc);
        assert_eq!(parse_panim(&text), doc);
    }

    #[test]
    fn open_frames_round_trip() {
        let text = "[Animation]\nname = \"A\"\nfps = 30\n[/Animation]\n\n[Objects]\nHand = Node2D\n[/Objects]\n\n[Frame0?]\n@Hand {\n    rotation = 0\n}\n[/Frame0]\n\n[Frame20]\n@Hand {\n    rotation = 1.5\n}\n[/Frame20]\n";
        let doc = parse_panim(text);
        let idx = doc.track_index("Hand", "rotation").unwrap();
        assert!(doc.tracks[idx].keys[0].open);
        assert!(!doc.tracks[idx].keys[1].open);
        assert_eq!(parse_panim(&serialize_panim(&doc)), doc);
    }

    #[test]
    fn remove_track_drops_unused_object() {
        let mut doc = parse_panim(SAMPLE);
        assert!(doc.remove_track("MainCam", "position"));
        assert!(doc.object_type("MainCam").is_none());
        // Hero still has an event, so removing its only track keeps the object.
        assert!(doc.remove_track("Hero", "position"));
        assert!(doc.object_type("Hero").is_some());
    }

    // Bone paths (`bones["Name"].position`) are opaque field strings to the
    // text model, so parse/serialize must round-trip them unchanged and the
    // `.interp`/`.ease` control suffixes must not false-match the bracket path.
    #[test]
    fn bone_path_fields_round_trip() {
        let text = "[Animation]\nname = \"Rig\"\nfps = 30\n[/Animation]\n\n[Objects]\nHero = Skeleton3D\n[/Objects]\n\n[Frame0]\n@Hero {\n    bones[\"Spine\"].position = (0, 1, 0)\n    bones[\"Spine\"].rotation = (0, 0, 0, 1)\n    bones[\"Spine\"].scale = (1, 1, 1)\n}\n[/Frame0]\n";
        let doc = parse_panim(text);
        assert_eq!(doc.tracks.len(), 3);
        let pos = doc.track_index("Hero", "bones[\"Spine\"].position").unwrap();
        assert_eq!(doc.tracks[pos].keys[0].value, "(0, 1, 0)");
        let rot = doc.track_index("Hero", "bones[\"Spine\"].rotation").unwrap();
        assert_eq!(doc.tracks[rot].keys[0].value, "(0, 0, 0, 1)");
        assert!(doc.track_index("Hero", "bones[\"Spine\"].scale").is_some());
        assert_eq!(parse_panim(&serialize_panim(&doc)), doc);
    }

    #[test]
    fn bone_path_interp_ease_controls_bind_to_bracket_field() {
        let mut doc = parse_panim(
            "[Animation]\nname = \"Rig\"\nfps = 30\n[/Animation]\n\n[Objects]\nHero = Skeleton2D\n[/Objects]\n\n[Frame0]\n@Hero {\n    bones[\"Arm\"].rotation.ease = \"ease_in\"\n    bones[\"Arm\"].rotation = 0.5\n}\n[/Frame0]\n",
        );
        let idx = doc.track_index("Hero", "bones[\"Arm\"].rotation").unwrap();
        assert_eq!(doc.tracks[idx].keys[0].value, "0.5");
        assert_eq!(doc.tracks[idx].keys[0].ease.as_deref(), Some("ease_in"));
        // No stray track created from the `.ease` control line.
        assert_eq!(doc.tracks.len(), 1);
        doc.set_key("Hero", "bones[\"Arm\"].rotation", 10, "1.0".to_string());
        assert_eq!(parse_panim(&serialize_panim(&doc)), doc);
    }

    #[test]
    fn bone_track_field_round_trips_through_parse() {
        for sub in BONE_TRACK_SUBFIELDS {
            let field = bone_track_field("Spine.01", sub);
            let (name, parsed_sub) = parse_bone_field(&field).unwrap();
            assert_eq!(name, "Spine.01");
            assert_eq!(parsed_sub, sub);
        }
        // Plain fields are not bone paths.
        assert!(parse_bone_field("position").is_none());
        assert!(parse_bone_field("bones[\"X\"].mesh").is_none());
        assert!(parse_bone_field("bones[\"X\"].rotation.ease").is_none());
    }

    #[test]
    fn bone_field_default_value_matches_component() {
        assert_eq!(
            default_field_value_text("Skeleton3D", "bones[\"H\"].position"),
            "(0, 0, 0)"
        );
        assert_eq!(
            default_field_value_text("Skeleton3D", "bones[\"H\"].rotation"),
            "(0, 0, 0, 1)"
        );
        assert_eq!(
            default_field_value_text("Skeleton2D", "bones[\"H\"].rotation"),
            "0"
        );
        assert_eq!(
            default_field_value_text("Skeleton2D", "bones[\"H\"].scale"),
            "(1, 1)"
        );
    }
}
