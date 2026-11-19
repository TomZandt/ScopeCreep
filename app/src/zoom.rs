//! The zoom model (spec #163, tickets #164 and #165): a Panel's view of its picture, and where the picture goes in
//! the Panel. Pure (no GPU), so every Picture View places its frame through it and it is tested on its own.
//!
//! Zoom is of *source* pixels: 100 % is one picture pixel to one device pixel. The range is Fit (the whole frame
//! fitted in the Panel) up to 400 %. Below 100 % zoom is free (each wheel notch scales by [`WHEEL_RATIO`], and a
//! step across 100 % lands on it); above 100 % it is whole steps (200, 300, 400 %), so every picture pixel is an
//! exact square. A frame that is already past 100 % at Fit starts at Fit and zooms on in whole steps; when Fit is
//! 400 % or more there is nothing beyond it.
use crate::picture_stage::{Placement, place};

/// The top of the zoom range, in percent of source pixels.
pub const MAX_PERCENT: f32 = 400.0;
/// What one wheel notch below 100 % scales the zoom by.
pub const WHEEL_RATIO: f32 = 1.25;
/// The key of the zoom slider in the Panel's zoom popover. Zoom is display state of the window, never a saved
/// setting: every session starts at Fit.
pub const ZOOM_KEY: &str = "zoom";

/// How much of the picture a Panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Zoom {
    /// The whole frame, fitted in the Panel.
    #[default]
    Fit,
    /// This many percent of source pixels, above Fit.
    Percent(f32),
}

/// A Panel's view of its picture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ZoomModel {
    pub zoom: Zoom,
    /// The picture point (in source pixels) at the middle of the Panel's picture area; the middle of the frame until
    /// the first zoom. Held inside the picture whenever it is used.
    pub centre: Option<[f32; 2]>,
    /// The part of a wheel notch turned so far.
    pending: f32,
}

/// Where the picture is drawn in its Panel and which part of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The whole picture's rectangle in device pixels; at zoom it runs past the Panel.
    pub placement: Placement,
    /// The part of it inside the Panel's picture area (left, top, width, height): what is drawn.
    pub visible: [f32; 4],
    /// The part of the picture that is drawn, in picture coordinates (left, top, right, bottom, 0 to 1, top first).
    pub window: [f32; 4],
    /// Read the exact texel (above 100 %), so every picture pixel is a square.
    pub nearest: bool,
}

/// What the zoom chip and its slider say.
#[derive(Clone, Debug, PartialEq)]
pub struct Readout {
    /// "Fit 23 %" or "200 %".
    pub text: String,
    /// The zoom now, in percent.
    pub percent: f32,
    /// What Fit is, in percent: the bottom of the slider.
    pub fit: f32,
    /// The top of the slider.
    pub max: f32,
}

impl ZoomModel {
    /// The picture area: `area` less the `strip` kept free at the bottom.
    fn room(area: [f32; 4], strip: f32) -> [f32; 4] {
        [area[0], area[1], area[2], area[3] - strip]
    }

    /// What Fit is, in percent of source pixels, exactly. `None` when nothing is left to draw in.
    fn fit(area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<f32> {
        let [_, _, w, h] = Self::room(area, strip);
        (w >= 1.0 && h >= 1.0 && picture[0] != 0 && picture[1] != 0).then(|| (w / picture[0] as f32).min(h / picture[1] as f32) * 100.0)
    }

    /// The top of the range: 400 %, or Fit when that is more.
    fn top(fit: f32) -> f32 {
        MAX_PERCENT.max(fit)
    }

    /// The zoom now in percent of source pixels (Fit's own percent at Fit). `None` when nothing is left to draw in.
    pub fn percent(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<f32> {
        let fit = Self::fit(area, picture, strip)?;
        Some(match self.zoom {
            Zoom::Percent(p) if p > fit => p.min(Self::top(fit)),
            _ => fit,
        })
    }

    /// Whether the picture is at Fit (a zoom at or below Fit is Fit).
    fn at_fit(&self, fit: f32) -> bool {
        !matches!(self.zoom, Zoom::Percent(p) if p > fit)
    }

    /// Where the `picture` (width, height) goes in `area` (left, top, width, height) less a `strip` kept free at
    /// the bottom for a legend, in whole device pixels. At zoom it runs past the Panel: [`ZoomModel::view`] gives
    /// the part that is drawn. `None` when nothing is left to draw in.
    pub fn placement(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<Placement> {
        let fit = Self::fit(area, picture, strip)?;
        if self.at_fit(fit) {
            return place(area, picture, strip);
        }
        let k = self.percent(area, picture, strip)? / 100.0;
        let [left, top, width, height] = Self::room(area, strip);
        let centre = self.centre.unwrap_or([picture[0] as f32 / 2.0, picture[1] as f32 / 2.0]);
        let axis = |from: f32, room: f32, size: u32, c: f32| {
            let drawn = (size as f32 * k).round();
            let origin = if drawn <= room {
                from + (room - drawn) / 2.0
            } else {
                // The picture's edge stops at the Panel's edge.
                from + room / 2.0 - c.clamp(room / 2.0 / k, size as f32 - room / 2.0 / k) * k
            };
            (origin.round(), drawn)
        };
        let (x, w) = axis(left, width, picture[0], centre[0]);
        let (y, h) = axis(top, height, picture[1], centre[1]);
        Some(Placement { x, y, w, h })
    }

    /// Where the picture goes and which part of it is drawn: the [`placement`](Self::placement) clipped to the
    /// Panel's picture area. `None` when nothing is left to draw.
    pub fn view(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<View> {
        let p = self.placement(area, picture, strip)?;
        let fit = Self::fit(area, picture, strip)?;
        if self.at_fit(fit) {
            // The whole frame, as it always was drawn.
            return Some(View { placement: p, visible: [p.x, p.y, p.w, p.h], window: [0.0, 0.0, 1.0, 1.0], nearest: false });
        }
        let [left, top, width, height] = Self::room(area, strip);
        let (x0, y0) = (p.x.max(left), p.y.max(top));
        let (x1, y1) = ((p.x + p.w).min(left + width), (p.y + p.h).min(top + height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(View {
            placement: p,
            visible: [x0, y0, x1 - x0, y1 - y0],
            window: [(x0 - p.x) / p.w, (y0 - p.y) / p.h, (x1 - p.x) / p.w, (y1 - p.y) / p.h],
            nearest: self.percent(area, picture, strip)? > 100.0,
        })
    }

    /// What Fit is: the whole percent of a source pixel's size the frame is drawn at when fitted (23 for "Fit 23 %").
    pub fn fit_percent(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<u32> {
        let placed = place(area, picture, strip)?;
        Some((placed.w / picture[0] as f32 * 100.0).round() as u32)
    }

    /// The next zoom in from `s` percent: free below 100 %, landing on it, then whole steps; never past `top`.
    fn step_in(s: f32, top: f32) -> f32 {
        let next = if s < 100.0 { (s * WHEEL_RATIO).min(100.0) } else { (s / 100.0).floor() * 100.0 + 100.0 };
        next.min(top)
    }

    /// The next zoom out from `s` percent.
    fn step_out(s: f32) -> f32 {
        if s > 100.0 { ((s / 100.0).ceil() - 1.0) * 100.0 } else { s / WHEEL_RATIO }
    }

    /// Zoom to `percent` keeping the picture pixel under the device point `at` there.
    fn zoom_about(&mut self, percent: f32, fit: f32, area: [f32; 4], picture: [u32; 2], strip: f32, at: [f32; 2]) {
        let Some(before) = self.placement(area, picture, strip) else { return };
        let (k0, [left, top, width, height]) = (before.w / picture[0] as f32, Self::room(area, strip));
        let under = [(at[0] - before.x) / k0, (at[1] - before.y) / k0];
        self.zoom = if percent <= fit + 1e-3 { Zoom::Fit } else { Zoom::Percent(percent) };
        let k = percent.max(fit) / 100.0;
        self.centre = Some([under[0] - (at[0] - (left + width / 2.0)) / k, under[1] - (at[1] - (top + height / 2.0)) / k]);
        self.hold(area, picture, strip);
    }

    /// Keep the centre inside the picture, so a pan can never leave the Panel empty.
    fn hold(&mut self, area: [f32; 4], picture: [u32; 2], strip: f32) {
        let (Some(percent), Some(c)) = (self.percent(area, picture, strip), self.centre) else { return };
        let k = percent / 100.0;
        let [_, _, width, height] = Self::room(area, strip);
        let hold = |c: f32, room: f32, size: u32| {
            let half = (room / 2.0 / k).min(size as f32 / 2.0);
            c.clamp(half, size as f32 - half)
        };
        self.centre = Some([hold(c[0], width, picture[0]), hold(c[1], height, picture[1])]);
    }

    /// The scroll wheel turned `lines` over the device point `at` (positive zooms in): whole notches step the
    /// zoom, parts of one wait for the rest. The picture pixel under `at` stays under it. Returns whether the zoom
    /// changed.
    pub fn wheel(&mut self, area: [f32; 4], picture: [u32; 2], strip: f32, at: [f32; 2], lines: f32) -> bool {
        let Some(fit) = Self::fit(area, picture, strip) else { return false };
        self.pending += lines;
        let notches = self.pending.trunc();
        self.pending -= notches;
        let before = self.zoom;
        for _ in 0..notches.abs() as u32 {
            let now = self.percent(area, picture, strip).unwrap_or(fit);
            let next = if notches > 0.0 { Self::step_in(now, Self::top(fit)) } else { Self::step_out(now).max(fit) };
            if (next - now).abs() > 1e-3 {
                self.zoom_about(next, fit, area, picture, strip, at);
            }
        }
        self.zoom != before
    }

    /// Set the zoom from the slider (Fit to 400 %): free below 100 %, whole steps above, nothing beyond the top.
    /// At or below Fit is Fit.
    pub fn set_percent(&mut self, percent: f32, area: [f32; 4], picture: [u32; 2], strip: f32) {
        let Some(fit) = Self::fit(area, picture, strip) else { return };
        let snapped = if (99.5..100.5).contains(&percent) {
            100.0
        } else if percent < 100.0 { percent } else { ((percent / 100.0).round() * 100.0).min(Self::top(fit)) };
        let [left, top, width, height] = Self::room(area, strip);
        // About the middle of the Panel's picture area (the slider has no pointer).
        self.zoom_about(snapped.max(0.0), fit, area, picture, strip, [left + width / 2.0, top + height / 2.0]);
    }

    /// What the zoom chip and its slider say. `None` when nothing is left to draw in.
    pub fn readout(&self, area: [f32; 4], picture: [u32; 2], strip: f32) -> Option<Readout> {
        let fit = Self::fit(area, picture, strip)?;
        let percent = self.percent(area, picture, strip)?;
        let text = if self.at_fit(fit) { format!("Fit {} %", fit.round() as u32) } else { format!("{} %", percent.round() as u32) };
        Some(Readout { text, percent, fit, max: Self::top(fit) })
    }
}
