//! The view block (#224, spec #196, ADR 0042): Fit, the Panel's zoom readout and Link, at the top right of every
//! Panel's title bar, between the Panel's own chips and the settings button. Pure: no GPU, no window. The block is on
//! every Panel, scopes included, so it is always in the same place; where nothing zooms (a Scope, or a picture
//! before its first frame) it is drawn disabled and greyed and hits nothing. Its layout depends on the title bar's
//! right edge and the display scale alone, never on what the readout says.
use crate::components::{Align, Chip, Face, Prim, State, contains, text, text_width};
use crate::style;

/// The block's buttons' texts and tips.
pub const FIT_CHIP: &str = "Fit";
pub const FIT_TOOLTIP: &str = "Fit the picture (every picture Panel while Link is on)";
pub const LINK_CHIP: &str = "Link";
pub const LINK_TOOLTIP: &str = "Link zoom across Panels";

/// The texts the readout can show; the widest sets the readout's slot.
const READOUTS: [&str; 1] = ["800 %"];

/// What a click on the block can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockPart {
    Fit,
    Link,
}

/// Where the block's parts are (device pixels): also the hit areas of Fit and Link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockLayout {
    pub fit: [f32; 4],
    pub readout: [f32; 4],
    pub link: [f32; 4],
}

/// What the block says now.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewBlock {
    /// The Panel zooms: Fit and Link work.
    pub enabled: bool,
    /// The Panel's zoom as a percentage ("62 %", "200 %"), empty while disabled. "Fit" is only the button.
    pub readout: String,
    /// Link is on (the one Window-wide switch, lit on every Panel).
    pub link: bool,
}

impl ViewBlock {
    /// A disabled block: nothing zooms in this Panel.
    pub fn disabled(link: bool) -> ViewBlock {
        ViewBlock {
            enabled: false,
            readout: String::new(),
            link,
        }
    }

    fn readout_width(scale: f32) -> f32 {
        READOUTS
            .iter()
            .map(|t| text_width(t, scale))
            .fold(0.0, f32::max)
    }

    /// The block's width at `scale`: Fit, the readout's slot and Link with a gap between.
    pub fn width(scale: f32) -> f32 {
        Chip::new(FIT_CHIP).size(scale)[0]
            + Self::readout_width(scale)
            + Chip::new(LINK_CHIP).size(scale)[0]
            + 2.0 * style::SPACE_CHIP_GAP * scale
    }

    /// Where the parts are for a block whose right edge is at `right`, centred in the title `bar`.
    pub fn layout(right: f32, bar: [f32; 4], scale: f32) -> BlockLayout {
        let gap = style::SPACE_CHIP_GAP * scale;
        let top = bar[1] + (bar[3] - style::SPACE_CHIP_HEIGHT * scale) / 2.0;
        let left = right - Self::width(scale);
        let fit = Chip::new(FIT_CHIP).rect([left, top], scale);
        let readout = [
            fit[0] + fit[2] + gap,
            bar[1],
            Self::readout_width(scale),
            bar[3],
        ];
        let link = Chip::new(LINK_CHIP).rect([readout[0] + readout[2] + gap, top], scale);
        BlockLayout { fit, readout, link }
    }

    /// The button under `point`; none while the block is disabled.
    pub fn hit(&self, layout: &BlockLayout, point: [f32; 2]) -> Option<BlockPart> {
        if !self.enabled {
            return None;
        }
        if contains(layout.fit, point) {
            Some(BlockPart::Fit)
        } else if contains(layout.link, point) {
            Some(BlockPart::Link)
        } else {
            None
        }
    }

    /// The block, with the button under the pointer lit.
    pub fn draw(&self, layout: &BlockLayout, hover: Option<BlockPart>, scale: f32) -> Vec<Prim> {
        let state = |part: BlockPart| {
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
        let mut prims = Chip::new(FIT_CHIP).draw(
            [layout.fit[0], layout.fit[1]],
            false,
            state(BlockPart::Fit),
            scale,
        );
        prims.push(text(
            &self.readout,
            layout.readout,
            Face::Label,
            ink,
            Align::Centre,
        ));
        prims.extend(Chip::new(LINK_CHIP).draw(
            [layout.link[0], layout.link[1]],
            self.link && self.enabled,
            state(BlockPart::Link),
            scale,
        ));
        prims
    }
}
