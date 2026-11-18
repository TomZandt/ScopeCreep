//! The style system's tokens (spec #113, ADR 0026): every colour and size the Window's chrome (the menu, labels,
//! scale numbers, the status line, the borders and gaps) uses, named by role and not by value. Dark and neutral
//! only, so nothing in the chrome tints the colourist's eye. Chrome code draws only through these; the style check
//! (`tests/style_check.rs`) fails on a raw colour or size literal there.
//!
//! Measurement colours (false colour bands, heatmap palette, skin tints, trace colours, a legend's bars) are data,
//! not style. They live with their displays, under that named exemption, and are never listed here.
//!
//! Colours are rgb in 0 to 1 for rectangles and rgb in 0 to 255 for text. Sizes are logical pixels.

/// Red, green and blue, 0 to 1.
pub type Rgb = [f32; 3];

// Surfaces.
/// The Window's background, behind the Panels.
pub const SURFACE_WINDOW: Rgb = [0.071, 0.071, 0.071];
/// A popover or menu's fill.
pub const SURFACE_MENU: Rgb = [0.11, 0.11, 0.11];

// Lines.
/// A menu's (or any frame's) border.
pub const LINE_BORDER: Rgb = [0.30, 0.30, 0.30];

// States.
/// The row under the cursor.
pub const STATE_HOVER: Rgb = [0.20, 0.20, 0.20];
/// The mark of a choice that is on.
pub const STATE_ON: Rgb = [0.86, 0.86, 0.86];

// Text, rgb 0 to 255.
/// Notices and menu choices: the brightest chrome text.
pub const TEXT_PRIMARY: [u8; 3] = [176, 176, 176];
/// Labels, the status line and menu headings.
pub const TEXT_SECONDARY: [u8; 3] = [138, 138, 138];
/// Scale numbers.
pub const TEXT_MUTED: [u8; 3] = [112, 112, 112];

// The type scale (size, then line height as a multiple of it).
pub const TYPE_NOTICE: f32 = 15.0;
pub const TYPE_NOTICE_LINE: f32 = 1.5;
/// Labels, the status line and the menu.
pub const TYPE_LABEL: f32 = 12.5;
pub const TYPE_LABEL_LINE: f32 = 1.4;
/// The numbers on a display's scale.
pub const TYPE_SCALE: f32 = 11.0;
pub const TYPE_SCALE_LINE: f32 = 1.3;

// Spacing.
/// Around the Window's edge and between the Panels.
pub const SPACE_MARGIN: f32 = 12.0;
/// The gutter for a scale's numbers.
pub const SPACE_GUTTER: f32 = 44.0;
/// A Panel's label strip, for one line.
pub const SPACE_LABEL_STRIP: f32 = 30.0;
/// One row of the status line.
pub const SPACE_STATUS_ROW: f32 = 20.0;
/// How far from a Panel border's gap a click still grabs it.
pub const SPACE_GRAB: f32 = 3.0;
/// The least height a display keeps above its label, and the least width of a Panel.
pub const SPACE_MIN_DISPLAY: f32 = 80.0;
pub const SPACE_MIN_PANEL_WIDTH: f32 = 120.0;
/// The window the saved border positions are checked against: the default window's Panel area and gap.
pub const SPACE_REFERENCE_AREA: [f32; 2] = [1256.0, 700.0];
pub const SPACE_REFERENCE_GAP: f32 = 12.0;
/// The menu: width, row height, padding.
pub const SPACE_MENU_WIDTH: f32 = 230.0;
pub const SPACE_MENU_ROW: f32 = 24.0;
pub const SPACE_MENU_PAD: f32 = 6.0;
/// Inside a menu row: the on-mark's left edge and side, the text's left edge (the heading's is nearer), and the
/// room the text gives up on the right.
pub const SPACE_MENU_MARK_LEFT: f32 = 12.0;
pub const SPACE_MENU_MARK: f32 = 6.0;
pub const SPACE_MENU_TEXT_LEFT: f32 = 26.0;
pub const SPACE_MENU_HEADING_LEFT: f32 = 10.0;
pub const SPACE_MENU_TEXT_RIGHT: f32 = 34.0;

// Line weights and corners.
/// A border's weight (the frame is drawn this much outside its fill).
pub const LINE_WEIGHT: f32 = 1.0;
/// Corner radius: square, as every rectangle the app draws is flat.
pub const RADIUS: f32 = 0.0;

/// A colour as rgba for the flat rectangles.
pub fn rgba(c: Rgb) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// A colour as the clear colour of a render pass.
pub fn clear(c: Rgb) -> wgpu::Color {
    wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 }
}
