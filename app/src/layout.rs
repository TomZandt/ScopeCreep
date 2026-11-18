//! Sizes shared by the Window's parts. The Panels' grid is in [`crate::panel_grid`].

/// Logical pixels: around the window's edge and between the Panels.
pub const MARGIN: f32 = crate::style::SPACE_MARGIN;
/// The gutter for the waveform scale's numbers ([`crate::waveform::PlotLayout`] uses the same width).
pub const GUTTER: f32 = crate::style::SPACE_GUTTER;
