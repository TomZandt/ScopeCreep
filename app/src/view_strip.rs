//! The view strip (#205, spec #196, ADR 0040): Fit, the shared zoom readout and Link, at the right end of the
//! menu bar row. Pure: no GPU, no window. The strip is always in the same place (its layout depends on the Window's
//! width and the display scale alone, never on what the readout says), is disabled while no picture Panel is shown
//! and never moves into a popover; the Window's minimum width counts it ([`ViewStrip::width`]).
use crate::components::{Align, Chip, Face, MenuBar, Prim, State, contains, text, text_width};
use crate::style;

/// The strip's buttons' texts and tips.
pub const FIT_CHIP: &str = "Fit";
pub const FIT_TOOLTIP: &str = "Fit every picture Panel";
pub const LINK_CHIP: &str = "Link";
pub const LINK_TOOLTIP: &str = "Link zoom across Panels";

/// The texts the readout can show; the widest sets the readout's slot.
const READOUTS: [&str; 3] = ["Fit", "800 %", "Mixed"];

/// What a click on the strip can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripPart {
    Fit,
    Link,
}

/// Where the strip's parts are (device pixels): also the hit areas of Fit and Link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StripLayout {
    pub fit: [f32; 4],
    pub readout: [f32; 4],
    pub link: [f32; 4],
}

/// What the strip says now.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewStrip {
    /// A picture Panel is shown: Fit and Link work.
    pub enabled: bool,
    /// "Fit", "200 %" or "Mixed" (empty while disabled).
    pub readout: String,
    /// Link is on.
    pub link: bool,
}

impl ViewStrip {
    fn readout_width(scale: f32) -> f32 {
        READOUTS
            .iter()
            .map(|t| text_width(t, scale))
            .fold(0.0, f32::max)
    }

    /// The strip's width at `scale`: Fit, the readout's slot and Link with a gap between.
    pub fn width(scale: f32) -> f32 {
        Chip::new(FIT_CHIP).size(scale)[0]
            + Self::readout_width(scale)
            + Chip::new(LINK_CHIP).size(scale)[0]
            + 2.0 * style::SPACE_STRIP_GAP * scale
    }

    /// Where the parts are in a Window `window_width` wide: right-aligned in the menu bar row, whatever the
    /// layout of the Panels below.
    pub fn layout(window_width: f32, scale: f32) -> StripLayout {
        let gap = style::SPACE_STRIP_GAP * scale;
        let row = MenuBar::height(scale);
        let top = (row - style::SPACE_CHIP_HEIGHT * scale) / 2.0;
        let left = window_width - style::SPACE_BAR_PAD * scale - Self::width(scale);
        let fit = Chip::new(FIT_CHIP).rect([left, top], scale);
        let readout = [fit[0] + fit[2] + gap, 0.0, Self::readout_width(scale), row];
        let link = Chip::new(LINK_CHIP).rect([readout[0] + readout[2] + gap, top], scale);
        StripLayout { fit, readout, link }
    }

    /// The parts of this strip: the same for every strip (see [`ViewStrip::layout`]).
    pub fn parts(&self, window_width: f32, scale: f32) -> StripLayout {
        Self::layout(window_width, scale)
    }

    /// The button under `point`; none while the strip is disabled.
    pub fn hit(&self, window_width: f32, point: [f32; 2], scale: f32) -> Option<StripPart> {
        if !self.enabled {
            return None;
        }
        let l = self.parts(window_width, scale);
        if contains(l.fit, point) {
            Some(StripPart::Fit)
        } else if contains(l.link, point) {
            Some(StripPart::Link)
        } else {
            None
        }
    }

    /// The strip, with the button under the pointer lit.
    pub fn draw(&self, window_width: f32, hover: Option<StripPart>, scale: f32) -> Vec<Prim> {
        let l = self.parts(window_width, scale);
        let state = |part: StripPart| {
            if !self.enabled {
                State::Disabled
            } else if hover == Some(part) {
                State::Hover
            } else {
                State::Normal
            }
        };
        let ink = if self.enabled {
            style::TEXT_SECONDARY
        } else {
            style::TEXT_DISABLED
        };
        let mut prims =
            Chip::new(FIT_CHIP).draw([l.fit[0], l.fit[1]], false, state(StripPart::Fit), scale);
        prims.push(text(
            &self.readout,
            l.readout,
            Face::Label,
            ink,
            Align::Centre,
        ));
        prims.extend(Chip::new(LINK_CHIP).draw(
            [l.link[0], l.link[1]],
            self.link && self.enabled,
            state(StripPart::Link),
            scale,
        ));
        prims
    }
}
