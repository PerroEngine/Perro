// Shared editor palette. One place for every chrome color so panels,
// rows, and widgets stay consistent. Values are hex strings because the
// scene files and runtime style setters both consume hex.

// Base surfaces, darkest to lightest.
pub const BG_CATEGORY: &str = "#1B1F26"; // top-level inspector category bar
pub const BG_SECTION: &str = "#252B34"; // nested section header bar
pub const BG_ROW_ALT: &str = "#FFFFFF08"; // zebra stripe on odd property rows
pub const BG_ROW_NESTED: &str = "#262B3280"; // rows inside expanded structs/arrays
pub const BG_WIDGET: &str = "#2A3039"; // text boxes, buttons, dropdowns
pub const BG_WIDGET_HOVER: &str = "#353D48";
pub const BG_WIDGET_PRESSED: &str = "#3B4552";

// Strokes.
pub const STROKE: &str = "#3B4450";
pub const STROKE_SOFT: &str = "#2D343E";

// Accent (selection, focus, active toggles).
pub const ACCENT: &str = "#4D84D1";
pub const ACCENT_SOFT: &str = "#6BA0EA";

// Changed-from-default property rows.
pub const CHANGED_FILL: &str = "#2C3A4D66";

// Text.
pub const TEXT: &str = "#D7DBE0";
pub const TEXT_DIM: &str = "#B0B8C3";
pub const TEXT_FAINT: &str = "#8993A0";

// Axis component tints (matches Godot's x/y/z/w coloring).
pub const AXIS_X: &str = "#D95F5F";
pub const AXIS_Y: &str = "#5EA868";
pub const AXIS_Z: &str = "#4D84D1";
pub const AXIS_W: &str = "#D9A24A";

// Revert-to-default affordance.
pub const REVERT: &str = "#D9A24A";
pub const REVERT_TEXT: &str = "#F0C96D";
pub const REVERT_HOVER_FILL: &str = "#3A3020";
pub const REVERT_PRESSED_FILL: &str = "#4A3A24";
pub const REVERT_PRESSED_STROKE: &str = "#E2B45E";
