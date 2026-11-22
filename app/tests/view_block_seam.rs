//! Which Panels have the view block, at the Feed seam (spec #196, #226, ADR 0044): the pictures do, the Scopes do
//! not, whatever the Feed is doing.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;

#[test]
fn only_the_picture_displays_have_a_view_block() {
    let rig = Rig::new("view-block-pictures");
    let (set, _) = DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
    let names = set.names();
    let pictures = [
        "Channel Viewer",
        "False Colour - Arri",
        "False Colour - Stops",
        "Skin Tone",
        "Saturation",
    ];
    let scopes = ["Waveform", "Vectorscope"];
    for (i, name) in names.iter().enumerate() {
        if pictures.contains(name) {
            assert!(set.is_picture(i), "{name} has the block");
        }
        if scopes.contains(name) {
            assert!(!set.is_picture(i), "{name} has none");
        }
    }
    assert!(names.iter().any(|n| pictures.contains(n)));
    assert!(names.iter().any(|n| scopes.contains(n)));
    assert!(!set.is_picture(999), "no such Panel");
}
