//! Where the window's parts go. The Picture View on the left and the waveform on the right take the Feed frame's
//! shape and the same size, each letterboxed in its half of the window, so a column of the picture sits level with
//! its trace in the waveform (one waveform column is one picture column). The scale's numbers keep a gutter of
//! their own to the waveform's left, and the labels sit in a strip directly under the panels; the status line
//! stays along the bottom of the window.
//!
//! Sizes are in device pixels; rectangles are left, top, width, height.

/// Logical pixels: around the window's edge and between the halves.
pub const MARGIN: f32 = 12.0;
/// The gutter for the waveform scale's numbers ([`crate::waveform::PlotLayout`] uses the same width).
pub const GUTTER: f32 = 44.0;
/// Until the first frame arrives the panels are laid out for 16:9.
const DEFAULT_FRAME: [u32; 2] = [1920, 1080];

/// What the window shows besides the panels, in device pixels: the label strip under the panels (taller when a
/// label wraps) and the status line along the bottom (taller with a notice in it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chrome {
    pub label_strip: f32,
    pub status: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowLayout {
    /// The Picture View, in the frame's shape.
    pub picture: [f32; 4],
    /// The waveform's plot and edge rows: the same size as the picture and level with it.
    pub waveform: [f32; 4],
    /// The scale's numbers, directly left of the waveform.
    pub gutter: [f32; 4],
    /// The left half, where the picture is letterboxed (and a notice is centred while there is no picture).
    pub picture_half: [f32; 4],
    /// The top of the label strip, directly under the panels.
    pub label_top: f32,
    /// How wide each panel's label may run: from the panel's left edge to the end of its half.
    pub picture_label_width: f32,
    pub waveform_label_width: f32,
}

impl WindowLayout {
    /// The layout of a `window` (width, height) at `scale` device pixels per logical pixel, for a Feed frame of
    /// `frame` (width, height; `None` before the first frame).
    pub fn new([width, height]: [f32; 2], scale: f32, frame: Option<[u32; 2]>, chrome: Chrome) -> WindowLayout {
        let [frame_width, frame_height] = frame.filter(|[w, h]| *w > 0 && *h > 0).unwrap_or(DEFAULT_FRAME).map(|v| v as f32);
        let margin = MARGIN * scale;
        let gutter = (GUTTER * scale).round();
        let half = (width / 2.0).round();
        // Each half less the margins; the right half also less the gutter. Both panels get the smaller fit.
        let left_area = [margin, half - margin / 2.0];
        let right_area = [half + margin / 2.0 + gutter, width - margin];
        let room_width = (left_area[1] - left_area[0]).min(right_area[1] - right_area[0]).max(1.0);
        let bottom = height - chrome.status;
        let room_height = (bottom - margin - chrome.label_strip).max(1.0);
        let fit = (room_width / frame_width).min(room_height / frame_height);
        let (w, h) = ((frame_width * fit).floor().max(1.0), (frame_height * fit).floor().max(1.0));
        // The panels and the label strip under them, centred in the height above the status line.
        let top = (margin + (room_height - h) / 2.0).round().max(margin.round());
        let centred = |[from, to]: [f32; 2]| (from + ((to - from) - w) / 2.0).round().max(from.round());
        let (picture_left, waveform_left) = (centred(left_area), centred(right_area));
        WindowLayout {
            picture: [picture_left, top, w, h],
            waveform: [waveform_left, top, w, h],
            gutter: [waveform_left - gutter, top, gutter, h],
            picture_half: [margin, margin, (left_area[1] - left_area[0]).max(1.0), room_height],
            label_top: top + h,
            picture_label_width: (left_area[1] - picture_left).max(1.0),
            waveform_label_width: (right_area[1] - waveform_left).max(1.0),
        }
    }

    /// The waveform with its gutter: the area [`crate::waveform::PlotLayout::new`] and
    /// [`crate::waveform::Waveform::draw`] take.
    pub fn waveform_area(&self) -> [f32; 4] {
        let [_, top, width, height] = self.waveform;
        [self.gutter[0], top, width + self.gutter[2], height]
    }
}
