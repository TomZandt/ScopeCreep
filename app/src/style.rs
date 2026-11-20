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
/// A Panel's title bar, for one line of label.
pub const SPACE_TITLE_BAR: f32 = 22.0;
/// The title bar's text: room left of it.
pub const SPACE_TITLE_PAD: f32 = 8.0;
/// One row of the status line.
pub const SPACE_STATUS_ROW: f32 = 20.0;
/// How far from a Panel border's gap a click still grabs it.
pub const SPACE_GRAB: f32 = 3.0;
/// The least height a display keeps under the title bar, the least width of a Panel, and the least height of a
/// Panel (the display, the title bar and the border's three lines).
pub const SPACE_MIN_DISPLAY: f32 = 80.0;
pub const SPACE_MIN_PANEL_WIDTH: f32 = 120.0;
pub const SPACE_MIN_PANEL_HEIGHT: f32 = 105.0;
/// The window the saved border positions are checked against: the default window's Panel area and gap.
pub const SPACE_REFERENCE_AREA: [f32; 2] = [1256.0, 716.0];
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

// Components (#115). Surfaces, states, accents and status.
/// A Panel's frame fill, between the Window and a menu.
pub const SURFACE_PANEL: Rgb = [0.09, 0.09, 0.09];
/// A control that is off: a chip, a segment, a slider's track.
pub const SURFACE_CONTROL: Rgb = [0.15, 0.15, 0.15];
/// A control under a press.
pub const STATE_PRESSED: Rgb = [0.27, 0.27, 0.27];
/// The one quiet accent: a control that is on, and a slider's filled part.
pub const ACCENT: Rgb = [0.78, 0.78, 0.78];
/// The ring round the control that holds the keyboard focus.
pub const STATE_FOCUS: Rgb = [0.55, 0.55, 0.55];
/// How much brighter an "on" control is under the pointer.
pub const STATE_ON_HOVER_LIFT: f32 = 0.08;
/// Text on an accent fill.
pub const TEXT_ON_ACCENT: [u8; 3] = [24, 24, 24];
/// Text of a control that cannot be used.
pub const TEXT_DISABLED: [u8; 3] = [78, 78, 78];

// The channel colours of the R G B Y chips when on: the one place chrome is not neutral, so a chip tells which
// channel it shows. Muted, so they stay quieter than the picture.
pub const CHANNEL_R: Rgb = [0.86, 0.34, 0.34];
pub const CHANNEL_G: Rgb = [0.34, 0.74, 0.40];
pub const CHANNEL_B: Rgb = [0.40, 0.54, 0.92];
/// Luma shows as grey.
pub const CHANNEL_Y: Rgb = [0.78, 0.78, 0.78];

// Status.
pub const STATUS_WARNING: Rgb = [0.86, 0.66, 0.26];
pub const STATUS_ERROR: Rgb = [0.86, 0.36, 0.30];

// Component sizes, logical pixels.
/// A chip: height, least width, side padding, the gap between chips, and the width one character of its text is
/// laid out at (the pure layout cannot shape text).
pub const SPACE_CHIP_HEIGHT: f32 = 20.0;
pub const SPACE_CHIP_MIN_WIDTH: f32 = 26.0;
pub const SPACE_CHIP_PAD: f32 = 8.0;
pub const SPACE_CHIP_GAP: f32 = 4.0;
pub const SPACE_CHAR_WIDTH: f32 = 7.6;
/// A segmented control: height and side padding of a segment.
pub const SPACE_SEGMENT_HEIGHT: f32 = 22.0;
pub const SPACE_SEGMENT_PAD: f32 = 12.0;
/// A slider: its row, the track's length and thickness, the thumb, and the room for its number.
pub const SPACE_SLIDER_HEIGHT: f32 = 24.0;
pub const SPACE_SLIDER_TRACK: f32 = 120.0;
pub const SPACE_SLIDER_TRACK_THICKNESS: f32 = 4.0;
pub const SPACE_SLIDER_THUMB: f32 = 12.0;
pub const SPACE_SLIDER_NUMBER: f32 = 44.0;
/// A tooltip's padding and its gap from what it names.
pub const SPACE_TOOLTIP_PAD: f32 = 6.0;
pub const SPACE_TOOLTIP_GAP: f32 = 6.0;
/// The top bar and the bars' side padding (the title bar is SPACE_TITLE_BAR above).
pub const SPACE_TOP_BAR: f32 = 32.0;
pub const SPACE_BAR_PAD: f32 = 10.0;
/// The menu bar above the top bar (#147): its height, a title's side padding, a separator row's height in a menu,
/// and the least gap between an entry's name and its hotkey.
pub const SPACE_MENU_BAR: f32 = 24.0;
pub const SPACE_MENU_BAR_ITEM_PAD: f32 = 10.0;
pub const SPACE_MENU_SEPARATOR: f32 = 9.0;
pub const SPACE_MENU_HOTKEY_GAP: f32 = 28.0;
/// The settings button on a title bar (square), and the gap between a name and its tags.
pub const SPACE_SETTINGS_BUTTON: f32 = 20.0;
pub const SPACE_TAG_GAP: f32 = 10.0;
/// A legend strip: the colour bar's height and the whole strip's (bar, then labels beneath).
pub const SPACE_LEGEND_BAR: f32 = 8.0;
pub const SPACE_LEGEND_HEIGHT: f32 = 26.0;
/// The development showcase sheet (logical pixels): the row-label column, a state column, a row, the sheet's
/// height, the wide and narrow sample Panels and their height and row step, and the tooltip anchor's offset.
pub const SPACE_SHOWCASE_LABEL: f32 = 140.0;
pub const SPACE_SHOWCASE_COLUMN: f32 = 240.0;
pub const SPACE_SHOWCASE_ROW: f32 = 36.0;
pub const SPACE_SHOWCASE_SHEET_HEIGHT: f32 = 4000.0;
pub const SPACE_SHOWCASE_PANEL_WIDE: f32 = 460.0;
pub const SPACE_SHOWCASE_PANEL_NARROW: f32 = 250.0;
pub const SPACE_SHOWCASE_PANEL_HEIGHT: f32 = 110.0;
pub const SPACE_SHOWCASE_PANEL_STEP: f32 = 130.0;
pub const SPACE_SHOWCASE_ANCHOR_OFFSET: f32 = 40.0;

/// A text colour (rgb 0 to 255) as a rectangle colour (rgb 0 to 1).
pub fn rgb(c: [u8; 3]) -> Rgb {
    c.map(|v| v as f32 / 255.0)
}

/// A colour as rgba for the flat rectangles.
pub fn rgba(c: Rgb) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

/// A colour as the clear colour of a render pass.
pub fn clear(c: Rgb) -> wgpu::Color {
    wgpu::Color {
        r: c[0] as f64,
        g: c[1] as f64,
        b: c[2] as f64,
        a: 1.0,
    }
}
