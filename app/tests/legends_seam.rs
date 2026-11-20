//! The View menu's legends entry at the display seam (#191, spec #192): the standard set's Panels with a legend
//! (ARRI False Colour, Stops False Colour, the saturation heatmap) are all set on or off through the same Panel
//! settings the popover edits (ADR 0013), each remembered per Panel, and a Panel can still differ afterwards.
//! Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::display::DisplaySet;
use scopecreep::settings::PanelSettings;

const LEGEND_PANELS: [&str; 3] = [
    "ARRI False Colour",
    "Stops False Colour",
    "Saturation heatmap",
];

fn set(rig: &Rig) -> DisplaySet {
    DisplaySet::standard(&rig.device, &rig.queue, TARGET, false)
}

fn legend_of(set: &DisplaySet, settings: &PanelSettings, name: &str) -> String {
    let index = (0..set.len()).find(|i| set.name(*i) == name).unwrap();
    set.settings_of(index, settings)["legend"].clone()
}

#[test]
fn every_legend_is_on_by_default_so_the_entry_starts_marked() {
    let rig = Rig::new("legends-default");
    let set = set(&rig);
    assert!(set.legends_on(&PanelSettings::default()));
}

#[test]
fn the_entry_turns_every_panels_legend_off_and_on_and_each_is_remembered_per_panel() {
    let rig = Rig::new("legends-all");
    let set = set(&rig);
    let mut settings = PanelSettings::default();
    set.set_legends(&mut settings, false);
    assert!(!set.legends_on(&settings));
    for name in LEGEND_PANELS {
        assert_eq!(legend_of(&set, &settings, name), "off", "{name}");
    }
    // Remembered per Panel: the saved text names each one.
    let back = PanelSettings::from_text(&settings.to_text());
    for name in LEGEND_PANELS {
        assert_eq!(
            legend_of(&set, &back, name),
            "off",
            "{name} after a restart"
        );
    }
    set.set_legends(&mut settings, true);
    assert!(set.legends_on(&settings));
    for name in LEGEND_PANELS {
        assert_eq!(legend_of(&set, &settings, name), "on", "{name}");
    }
}

#[test]
fn a_panel_can_differ_afterwards_and_the_entry_then_shows_off() {
    let rig = Rig::new("legends-differ");
    let set = set(&rig);
    let mut settings = PanelSettings::default();
    set.set_legends(&mut settings, true);
    settings.set("Stops False Colour", "legend", "off");
    assert!(
        !set.legends_on(&settings),
        "one off, so not every legend is on"
    );
    assert_eq!(legend_of(&set, &settings, "ARRI False Colour"), "on");
    assert_eq!(legend_of(&set, &settings, "Stops False Colour"), "off");
    settings.set("Stops False Colour", "legend", "on");
    assert!(set.legends_on(&settings));
}

#[test]
fn only_panels_with_a_legend_get_the_setting_and_no_other_setting_changes() {
    let rig = Rig::new("legends-only");
    let set = set(&rig);
    let mut settings = PanelSettings::default();
    settings.set("Saturation heatmap", "scale", "120");
    set.set_legends(&mut settings, false);
    let text = settings.to_text();
    let legend_lines = text.lines().filter(|l| l.contains("\tlegend\t")).count();
    assert_eq!(legend_lines, LEGEND_PANELS.len(), "{text}");
    assert!(text.contains("Saturation heatmap\tscale\t120"));
}
