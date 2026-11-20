//! Page templates (#215, spec #196, ADR 0041): the pages the Page bar's **+** menu can make besides Empty, Default
//! and Copy. A template names the displays its page holds; the page is arranged once with Auto layout for the
//! footage's aspect, so each starts well for the picture in front of the colourist.

use crate::auto_layout::Item;
use crate::layout_tree::LayoutTree;

/// A template for a new page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageTemplate {
    /// All 7 Panels, arranged for the footage (the Default layout is the fixed arrangement of the same Panels).
    AllSeven,
    /// The clean picture with the four other picture views.
    PictureViews,
    /// The clean picture with the luma waveform and the Vectorscope.
    Scopes,
    /// The clean picture alone.
    PictureOnly,
}

impl PageTemplate {
    pub const ALL: [PageTemplate; 4] = [
        PageTemplate::AllSeven,
        PageTemplate::PictureViews,
        PageTemplate::Scopes,
        PageTemplate::PictureOnly,
    ];

    /// The menu entry and the name of a page made from it.
    pub fn name(self) -> &'static str {
        match self {
            PageTemplate::AllSeven => "All 7 Panels",
            PageTemplate::PictureViews => "Picture views",
            PageTemplate::Scopes => "Scopes",
            PageTemplate::PictureOnly => "Picture only",
        }
    }

    /// The displays the page holds, in reading order.
    pub fn displays(self) -> &'static [&'static str] {
        match self {
            PageTemplate::AllSeven => &[
                "Channel Viewer",
                "Waveform",
                "False Colour - Arri",
                "Vectorscope",
                "Saturation",
                "Skin Tone",
                "False Colour - Stops",
            ],
            PageTemplate::PictureViews => &[
                "Channel Viewer",
                "False Colour - Arri",
                "False Colour - Stops",
                "Saturation",
                "Skin Tone",
            ],
            PageTemplate::Scopes => &["Channel Viewer", "Waveform", "Vectorscope"],
            PageTemplate::PictureOnly => &["Channel Viewer"],
        }
    }

    /// The page's layout for `panels` (the Panel each of [`Self::displays`] is, in order), arranged by Auto layout
    /// for `aspect` in `area`. `items` are by Panel, for every Panel of the Window. When the Window has no room
    /// for an arrangement that keeps every Panel at its least size, the plain halving of the Panels is kept.
    pub fn tree(
        self,
        panels: &[usize],
        items: &[Item],
        aspect: f32,
        area: [f32; 4],
        gap: f32,
        scale: f32,
    ) -> LayoutTree {
        let mut tree = LayoutTree::automatic(panels.len());
        tree.remap(|p| panels.get(p).copied().unwrap_or(p));
        let _ = tree.auto_layout(items, aspect, area, gap, scale);
        tree
    }
}
