//! The zoom model (spec #163, tickets #164 and #165): a Panel's view of its picture, and where the picture goes in
//! the Panel. Pure (no GPU), so every Picture View places its frame through it and it is tested on its own.
//!
//! Zoom is of *source* pixels: 100 % is one picture pixel to one device pixel. The range is Fit (the whole frame
//! fitted in the Panel) up to 800 %. Below 100 % zoom is free (each wheel notch scales by [`WHEEL_RATIO`], and a
//! step across 100 % lands on it); above 100 % it is whole steps (200, 300 ... 800 %), so every picture pixel is an
//! exact square. A frame that is already past 100 % at Fit starts at Fit and zooms on in whole steps; when Fit is
//! 800 % or more there is nothing beyond it.
use crate::display::{Pointer, PointerResult};
use crate::picture_stage::{Placement, place};
use std::sync::{Arc, Mutex};

/// The top of the zoom range, in percent of source pixels.
pub const MAX_PERCENT: f32 = 800.0;
/// What one wheel notch below 100 % scales the zoom by.
pub const WHEEL_RATIO: f32 = 1.25;
/// The key of the zoom chip's click (back to Fit). Zoom is display state of the window, never a saved
/// setting: every session starts at Fit.
pub const ZOOM_KEY: &str = "zoom";

/// The key of the link chip's click (#168), and where Link is remembered: Window-wide state beside the Panel
/// settings (ADR 0015), under the name [`LINK_STATE_PANEL`]. Zoom and pan are never saved; only Link is.
pub const LINK_KEY: &str = "link";
pub const LINK_STATE_PANEL: &str = "Window";

/// Whether Link is on: it opens on, so only a saved "off" turns it off (rubbish and nothing saved are on).
pub fn link_remembered(settings: &crate::settings::PanelSettings) -> bool {
    settings
        .state_of(LINK_STATE_PANEL, &[LINK_KEY])
        .get(LINK_KEY)
        .is_none_or(|v| v != crate::toolbar::TOGGLE_OFF)
}

/// Remember Link as `on` (the same "on" and "off" [`link_remembered`] reads). Whether the saved state changed.
pub fn remember_link(settings: &mut crate::settings::PanelSettings, on: bool) -> bool {
    settings.set_state(
        LINK_STATE_PANEL,
        LINK_KEY,
        if on { "on" } else { crate::toolbar::TOGGLE_OFF },
    )
}

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
    /// The long text: "Fit 23 %" or "200 %".
    pub text: String,
    /// The short text, for a title bar with no room for the word Fit: "23 %" or "200 %".
    pub short: String,
    /// The zoom now, in percent.
    pub percent: f32,
    /// What Fit is, in percent: the bottom of the slider.
    pub fit: f32,
    /// The top of the slider.
    pub max: f32,
}

/// Where a picture goes: what travels together through every zoom question.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    /// The Panel's content (left, top, width, height) in device pixels.
    pub area: [f32; 4],
    /// The picture's size in source pixels (width, height).
    pub picture: [u32; 2],
    /// The height kept free at the bottom of `area` for a legend.
    pub strip: f32,
}

impl Geometry {
    pub fn new(area: [f32; 4], picture: [u32; 2], strip: f32) -> Geometry {
        Geometry {
            area,
            picture,
            strip,
        }
    }

    /// The picture area: `area` less the `strip` kept free at the bottom.
    fn room(&self) -> [f32; 4] {
        let [left, top, width, height] = self.area;
        [left, top, width, height - self.strip]
    }

    /// The middle of the frame, in source pixels.
    fn middle(&self) -> [f32; 2] {
        [self.picture[0] as f32 / 2.0, self.picture[1] as f32 / 2.0]
    }

    /// What Fit is, in percent of source pixels, exactly. `None` when nothing is left to draw in.
    fn fit(&self) -> Option<f32> {
        let [_, _, w, h] = self.room();
        let [pw, ph] = self.picture;
        (w >= 1.0 && h >= 1.0 && pw != 0 && ph != 0)
            .then(|| (w / pw as f32).min(h / ph as f32) * 100.0)
    }
}

impl ZoomModel {
    /// The top of the range: 800 %, or Fit when that is more.
    fn top(fit: f32) -> f32 {
        MAX_PERCENT.max(fit)
    }

    /// The zoom now in percent of source pixels (Fit's own percent at Fit). `None` when nothing is left to draw in.
    pub fn percent(&self, g: Geometry) -> Option<f32> {
        let fit = g.fit()?;
        Some(match self.zoom {
            Zoom::Percent(p) if p > fit => p.min(Self::top(fit)),
            _ => fit,
        })
    }

    /// The zoom this model holds, which may be below this Panel's Fit: a shared view (Link) can sit under the Fit of
    /// a Panel that is drawn at its own Fit. Equal to [`percent`](Self::percent) otherwise.
    fn held_percent(&self, fit: f32) -> f32 {
        match self.zoom {
            Zoom::Percent(p) => p.min(Self::top(fit)),
            Zoom::Fit => fit,
        }
    }

    /// Whether the picture is at Fit (a zoom at or below Fit is Fit).
    fn at_fit(&self, fit: f32) -> bool {
        !matches!(self.zoom, Zoom::Percent(p) if p > fit)
    }

    /// Where the picture goes in the Panel (less the strip kept free at the bottom for a legend), in whole device
    /// pixels. At zoom it runs past the Panel: [`ZoomModel::view`] gives the part that is drawn. `None` when
    /// nothing is left to draw in.
    pub fn placement(&self, g: Geometry) -> Option<Placement> {
        let fit = g.fit()?;
        if self.at_fit(fit) {
            return place(g.area, g.picture, g.strip);
        }
        let k = self.percent(g)? / 100.0;
        let [left, top, width, height] = g.room();
        let picture = g.picture;
        let centre = self.centre.unwrap_or(g.middle());
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
    pub fn view(&self, g: Geometry) -> Option<View> {
        let p = self.placement(g)?;
        let fit = g.fit()?;
        if self.at_fit(fit) {
            // The whole frame, as it always was drawn.
            return Some(View {
                placement: p,
                visible: [p.x, p.y, p.w, p.h],
                window: [0.0, 0.0, 1.0, 1.0],
                nearest: false,
            });
        }
        let [left, top, width, height] = g.room();
        let (x0, y0) = (p.x.max(left), p.y.max(top));
        let (x1, y1) = ((p.x + p.w).min(left + width), (p.y + p.h).min(top + height));
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        Some(View {
            placement: p,
            visible: [x0, y0, x1 - x0, y1 - y0],
            window: [
                (x0 - p.x) / p.w,
                (y0 - p.y) / p.h,
                (x1 - p.x) / p.w,
                (y1 - p.y) / p.h,
            ],
            nearest: self.percent(g)? > 100.0,
        })
    }

    /// What Fit is: the whole percent of a source pixel's size the frame is drawn at when fitted (23 for "Fit 23 %").
    pub fn fit_percent(&self, g: Geometry) -> Option<u32> {
        Some(g.fit()?.round() as u32)
    }

    /// The next zoom in from `s` percent: free below 100 %, landing on it, then whole steps; never past `top`.
    fn step_in(s: f32, top: f32) -> f32 {
        let next = if s < 100.0 {
            (s * WHEEL_RATIO).min(100.0)
        } else {
            (s / 100.0).floor() * 100.0 + 100.0
        };
        next.min(top)
    }

    /// The next zoom out from `s` percent.
    fn step_out(s: f32) -> f32 {
        if s > 100.0 {
            ((s / 100.0).ceil() - 1.0) * 100.0
        } else {
            s / WHEEL_RATIO
        }
    }

    /// Zoom to `percent` keeping the picture pixel under the device point `at` there.
    fn zoom_about(&mut self, percent: f32, keep: bool, fit: f32, g: Geometry, at: [f32; 2]) {
        let Some(before) = self.placement(g) else {
            return;
        };
        let (k0, [left, top, width, height]) = (before.w / g.picture[0] as f32, g.room());
        let under = [(at[0] - before.x) / k0, (at[1] - before.y) / k0];
        // A shared zoom already at or below this Panel's Fit (Link) stays what it is (`keep`); else at or below Fit is Fit.
        self.zoom = if percent <= fit + 1e-3 && !keep {
            Zoom::Fit
        } else {
            Zoom::Percent(percent)
        };
        let k = if self.zoom == Zoom::Fit {
            percent.max(fit)
        } else {
            percent
        } / 100.0;
        self.centre = Some([
            under[0] - (at[0] - (left + width / 2.0)) / k,
            under[1] - (at[1] - (top + height / 2.0)) / k,
        ]);
        self.hold(g);
    }

    /// Keep the centre inside the picture, so a pan can never leave the Panel empty.
    fn hold(&mut self, g: Geometry) {
        let (Some(fit), Some(c)) = (g.fit(), self.centre) else {
            return;
        };
        let k = self.held_percent(fit) / 100.0;
        let [_, _, width, height] = g.room();
        let picture = g.picture;
        // Under this Panel's own Fit (a shared view, Link) it has no edge to stop at: the other Panels' placement does
        // that, so only keep the centre in the picture.
        let under_fit = self.held_percent(fit) < fit;
        let hold = |c: f32, room: f32, size: u32| {
            let half = if under_fit {
                0.0
            } else {
                (room / 2.0 / k).min(size as f32 / 2.0)
            };
            c.clamp(half, size as f32 - half)
        };
        self.centre = Some([
            hold(c[0], width, picture[0]),
            hold(c[1], height, picture[1]),
        ]);
    }

    /// The picture was dragged by `delta` device pixels (spec #163, #166): it moves exactly as far as the pointer, and
    /// is held so the Panel is never left empty (a picture smaller than the Panel on an axis stays centred). At Fit
    /// there is nothing to pan. Returns whether the picture moved.
    pub fn pan(&mut self, g: Geometry, delta: [f32; 2]) -> bool {
        let Some(fit) = g.fit() else { return false };
        if self.zoom == Zoom::Fit {
            return false;
        }
        let k = self.held_percent(fit) / 100.0;
        let before = self.centre;
        let c = before.unwrap_or(g.middle());
        self.centre = Some([c[0] - delta[0] / k, c[1] - delta[1] / k]);
        self.hold(g);
        self.centre != before
    }

    /// The scroll wheel turned `lines` over the device point `at` (positive zooms in): whole notches step the
    /// zoom, parts of one wait for the rest. The picture pixel under `at` stays under it. Returns whether the zoom
    /// changed.
    pub fn wheel(&mut self, g: Geometry, at: [f32; 2], lines: f32) -> bool {
        let Some(fit) = g.fit() else { return false };
        self.pending += lines;
        let notches = self.pending.trunc();
        self.pending -= notches;
        let before = self.zoom;
        for _ in 0..notches.abs() as u32 {
            if notches < 0.0 && self.zoom == Zoom::Fit {
                break;
            }
            let now = self.held_percent(fit);
            let next = if notches > 0.0 {
                Self::step_in(now, Self::top(fit))
            } else {
                Self::step_out(now)
            };
            if (next - now).abs() > 1e-3 {
                self.zoom_about(next, now <= fit + 1e-3, fit, g, at);
            }
        }
        self.zoom != before
    }

    /// Set the zoom from the slider (Fit to 800 %): free below 100 %, whole steps above, nothing beyond the top.
    /// At or below Fit is Fit.
    pub fn set_percent(&mut self, percent: f32, g: Geometry) {
        let Some(fit) = g.fit() else { return };
        let snapped = if (99.5..100.5).contains(&percent) {
            100.0
        } else if percent < 100.0 {
            percent
        } else {
            ((percent / 100.0).round() * 100.0).min(Self::top(fit))
        };
        let [left, top, width, height] = g.room();
        // About the middle of the Panel's picture area (the slider has no pointer).
        self.zoom_about(
            snapped.max(0.0),
            false,
            fit,
            g,
            [left + width / 2.0, top + height / 2.0],
        );
    }

    /// What the zoom chip and its slider say. `None` when nothing is left to draw in.
    pub fn readout(&self, g: Geometry) -> Option<Readout> {
        let fit = g.fit()?;
        let percent = self.percent(g)?;
        let short = if self.at_fit(fit) {
            self.fit_percent(g)?
        } else {
            percent.round() as u32
        };
        let short = format!("{short} %");
        let text = if self.at_fit(fit) {
            format!("Fit {short}")
        } else {
            short.clone()
        };
        Some(Readout {
            text,
            short,
            percent,
            fit,
            max: Self::top(fit),
        })
    }
}

/// The Window's one Link (#168): whether picture Panels zoom together, and the one view they share while it is on
/// (a zoom and a centre point in picture coordinates, so Panels of different sizes look at the same pixels and each
/// fits its own Panel at Fit). Cheap to clone (shared state).
#[derive(Clone, Default)]
pub struct Link(Arc<Mutex<LinkState>>);

#[derive(Default)]
struct LinkState {
    on: bool,
    /// The shared view while on; the view every Panel keeps when it goes off.
    model: ZoomModel,
    /// Counts the times Link was switched, so a Panel can tell it must take the shared view when Link goes off.
    generation: u64,
}

impl Link {
    pub fn on(&self) -> bool {
        self.0.lock().unwrap().on
    }

    /// Switch Link. Turning it on takes `from` (the view of the Panel it was clicked on; Fit when `None`) as the
    /// view of every Panel; turning it off leaves each Panel at the shared view it had.
    pub fn set(&self, on: bool, from: Option<ZoomModel>) {
        let mut state = self.0.lock().unwrap();
        if on && !state.on {
            state.model = from.unwrap_or_default();
        }
        state.on = on;
        state.generation += 1;
    }
}

/// A picture Panel's zoom as a display holds it (#167): the [`ZoomModel`] and a middle-drag in progress, with the
/// wheel, middle-button and slider handling every picture Panel shares. While the Window's [`Link`] is on it works
/// on the shared view instead of its own. Cheap to clone (shared state).
#[derive(Clone, Default)]
pub struct PanelZoom {
    model: Arc<Mutex<ZoomModel>>,
    /// Where the pointer was at the last step of a middle-drag, while one is going.
    pan_from: Arc<Mutex<Option<[f32; 2]>>>,
    link: Arc<Mutex<Option<Link>>>,
    /// The [`Link`] generation this Panel's own view last took the shared one at.
    seen: Arc<Mutex<u64>>,
}

impl PanelZoom {
    /// Join the Window's Link (the display set does this on registering).
    pub fn link_to(&self, link: &Link) {
        *self.link.lock().unwrap() = Some(link.clone());
    }

    /// Run `f` on the view this Panel draws and zooms with: the shared one while Link is on, else its own (which
    /// first takes the shared view if Link was just turned off, so each Panel keeps where it was).
    fn with<R>(&self, f: impl FnOnce(&mut ZoomModel) -> R) -> R {
        let link = self.link.lock().unwrap().clone();
        let Some(link) = link else {
            return f(&mut self.model.lock().unwrap());
        };
        let mut shared = link.0.lock().unwrap();
        if shared.on {
            return f(&mut shared.model);
        }
        let (mut own, mut seen) = (self.model.lock().unwrap(), self.seen.lock().unwrap());
        if *seen != shared.generation {
            *own = shared.model;
            *seen = shared.generation;
        }
        f(&mut own)
    }

    /// The model, for drawing through [`crate::picture_stage::PictureStage::draw_zoomed`].
    pub fn model(&self) -> ZoomModel {
        self.with(|m| *m)
    }

    /// The wheel (zooms toward the pointer) and the middle button (pans) over a Panel showing a picture (`geometry`
    /// is `None` before a frame has arrived). `None` for any other event (the display's own), or when there is no
    /// picture.
    pub fn pointer(&self, geometry: Option<Geometry>, event: Pointer) -> Option<PointerResult> {
        let used = PointerResult {
            used: true,
            remember: Vec::new(),
        };
        match event {
            Pointer::Wheel { at, lines } => {
                let g = geometry?;
                self.with(|m| m.wheel(g, at, lines));
                Some(used)
            }
            Pointer::MiddleDown(at) => {
                *self.pan_from.lock().unwrap() = Some(at);
                Some(used)
            }
            Pointer::MiddleUp => Some(PointerResult {
                used: self.pan_from.lock().unwrap().take().is_some(),
                remember: Vec::new(),
            }),
            Pointer::Move(at) => {
                let mut from = self.pan_from.lock().unwrap();
                let (last, g) = (from.as_ref().copied()?, geometry?);
                *from = Some(at);
                self.with(|m| m.pan(g, [at[0] - last[0], at[1] - last[1]]));
                Some(used)
            }
            _ => None,
        }
    }

    /// What the zoom chip says (see [`ZoomModel::readout`]); `None` before a frame has arrived.
    pub fn readout(&self, geometry: Option<Geometry>) -> Option<Readout> {
        self.with(|m| m.readout(geometry?))
    }

    /// Set the zoom from the chip's slider (see [`ZoomModel::set_percent`]).
    pub fn set_percent(&self, percent: f32, geometry: Option<Geometry>) {
        if let Some(g) = geometry {
            self.with(|m| m.set_percent(percent, g));
        }
    }
}

/// What a picture display gives the [`crate::display::Display`] trait so that zooming needs no code of its own: its
/// [`PanelZoom`], the size of its picture and the strip it keeps free under it. A display implements this and returns
/// itself from `Display::picture_zoom`; the wheel, middle-drag, chip, slider and Link then all follow.
pub trait PictureZoom {
    /// The Panel's zoom.
    fn panel_zoom(&self) -> &PanelZoom;
    /// The picture's size in source pixels; `None` before a frame has arrived (or while a message fills the Panel).
    fn picture_size(&self) -> Option<[u32; 2]>;
    /// The height the legend takes under the picture in a Panel of content `area`. No strip by default.
    fn strip(&self, _area: [f32; 4], _scale: f32) -> f32 {
        0.0
    }
    /// Where the picture goes in a Panel of content `area`; `None` before a frame has arrived.
    fn geometry(&self, area: [f32; 4], scale: f32) -> Option<Geometry> {
        Some(Geometry::new(
            area,
            self.picture_size()?,
            self.strip(area, scale),
        ))
    }
}
