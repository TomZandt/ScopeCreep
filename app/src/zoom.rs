//! The zoom model (spec #163, ticket #164): a Panel's view of its picture, and where the picture goes in the
//! Panel. Pure (no GPU), so every Picture View places its frame through it and it is tested on its own.
//!
//! Fit, the whole frame fitted in the Panel, is the only zoom so far; the ticket that adds zooming and panning
//! (#165) grows this module.
use crate::picture_stage::{Placement, place};

/// How much of the picture a Panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Zoom {
    /// The whole frame, fitted in the Panel.
    #[default]
    Fit,
}

/// A Panel's view of its picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoomModel {
    pub zoom: Zoom,
}

impl ZoomModel {
    /// Where the `picture` (width, height) goes in `area` (left, top, width, height) less a `strip` kept free at
    /// the bottom for a legend, in whole device pixels. `None` when nothing is left to draw in.
    pub fn placement(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<Placement> {
        match self.zoom {
            Zoom::Fit => place(area, picture, strip),
        }
    }

    /// What Fit is: the whole percent of a source pixel's size the frame is drawn at when fitted (23 for "Fit 23 %").
    pub fn fit_percent(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<u32> {
        let placed = place(area, picture, strip)?;
        Some((placed.w / picture[0] as f32 * 100.0).round() as u32)
    }
}
