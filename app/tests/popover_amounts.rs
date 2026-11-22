//! The Panel settings popover's amounts (#225), on the pure seam: one row each (label, slider, value box with
//! arrows, circular reset icon), a slider that follows the drag, typing into the value box, and the Panel reset.
//! No GPU, no window.
use scopecreep::components::{Face, Icon, Prim, contains, popover, popover_tip};
use scopecreep::settings::{
    Amount, AmountHit, Choice, EditKey, Menu, MenuRow, PanelSettings, SettingDef,
};
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];

const BRIGHTNESS: Amount = Amount {
    min: 0.0,
    max: 200.0,
    step: 5.0,
    default: 100.0,
    unit: " %",
    decimals: 0,
    legacy: &[],
};
const SPREAD: Amount = Amount {
    min: 0.0,
    max: 4.0,
    step: 0.05,
    default: 1.0,
    unit: " px",
    decimals: 2,
    legacy: &[],
};

const DEFS: [SettingDef; 4] = [
    SettingDef {
        key: "weights",
        label: "Luma weights",
        amount: None,
        choices: &[
            Choice {
                value: "a",
                text: "Alpha",
            },
            Choice {
                value: "b",
                text: "Beta",
            },
        ],
    },
    SettingDef {
        key: "brightness",
        label: "Trace brightness",
        amount: Some(BRIGHTNESS),
        choices: &[],
    },
    SettingDef {
        key: "spread",
        label: "Soft spread",
        amount: Some(SPREAD),
        choices: &[],
    },
    SettingDef {
        key: "units",
        label: "Scale units",
        amount: None,
        choices: &[
            Choice {
                value: "codes",
                text: "10-bit codes",
            },
            Choice {
                value: "percent",
                text: "Percent",
            },
        ],
    },
];

fn menu_for(settings: &PanelSettings, panel: &str) -> Menu {
    Menu::for_panel(0, &DEFS, &settings.resolve(panel, &DEFS))
}

fn row_of(menu: &Menu, key: &str) -> usize {
    menu.rows
        .iter()
        .position(|r| matches!(r, MenuRow::Slider { key: k, .. } if *k == key))
        .expect("an amount row")
}

fn middle(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

fn value_in(settings: &PanelSettings, panel: &str, key: &str) -> f32 {
    settings.resolve(panel, &DEFS)[key].parse().unwrap()
}

/// What the app does with a change: remember it as the Panel's setting.
fn remember(settings: &mut PanelSettings, panel: &str, key: &str, value: f32) {
    settings.set(panel, key, &Amount::store(value));
}

#[test]
fn dragging_a_slider_moves_the_readout_and_the_panels_setting_on_every_move() {
    for scale in SCALES {
        let mut settings = PanelSettings::default();
        let mut menu = menu_for(&settings, "P");
        let rect = menu.rect([100.0, 100.0], [1600.0, 1200.0], scale);
        let row = row_of(&menu, "brightness");
        let track = menu.slider_layout(rect, row, scale).track;
        let y = menu.own_rect(rect, row, scale)[1] + 4.0 * scale;
        let press = menu.press(rect, [track[0] + track[2] * 0.25, y], scale, false);
        assert!(press.consumed && menu.dragging());
        for c in &press.changes {
            remember(&mut settings, "P", c.key, c.value);
        }
        let mut seen = vec![menu.value_of("brightness").unwrap()];
        for fraction in [0.4, 0.55, 0.7, 0.9, 1.3] {
            let x = track[0] + track[2] * fraction;
            let change = menu.drag_to(rect, x, scale).expect("every move changes it");
            remember(&mut settings, "P", change.key, change.value);
            // The popover's own readout and the setting both follow the move.
            let shown = menu.value_of("brightness").unwrap();
            assert_eq!(shown, change.value);
            assert_eq!(value_in(&settings, "P", "brightness"), shown);
            let prims = popover(&menu, rect, [-1.0, -1.0], scale);
            let said = BRIGHTNESS.text(shown);
            assert!(
                prims
                    .iter()
                    .any(|p| matches!(p, Prim::Text { text, .. } if *text == said)),
                "the number on screen says {said}"
            );
            seen.push(shown);
        }
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
        assert_eq!(*seen.last().unwrap(), BRIGHTNESS.max, "clamped at the end");
        menu.release();
        assert!(!menu.dragging());
        assert_eq!(menu.drag_to(rect, track[0], scale), None, "let go");
    }
}

#[test]
fn an_amount_is_one_row_of_label_slider_value_box_and_reset_inside_the_popover() {
    for scale in SCALES {
        let menu = menu_for(&PanelSettings::default(), "P");
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        for key in ["brightness", "spread"] {
            let row = row_of(&menu, key);
            let r = menu.row_rect(rect, row, scale);
            let l = menu.slider_layout(rect, row, scale);
            let label = menu.label_rect(rect, row, scale);
            // Left to right, none crowding the next.
            let gap = 1.0;
            assert!(label[0] + label[2] <= l.track[0] - l.thumb[2] / 2.0 + gap);
            assert!(l.track[0] + l.track[2] < l.value_box[0]);
            assert!(l.value_box[0] + l.value_box[2] < l.reset[0]);
            for part in [l.value_box, l.number, l.up, l.down, l.reset, label] {
                assert!(part[0] >= rect[0] && part[0] + part[2] <= rect[0] + rect[2]);
                assert!(part[1] >= r[1] && part[1] + part[3] <= r[1] + r[3] + 0.01);
            }
            // Hit targets: the box, each arrow and the reset icon are readable sizes.
            for button in [l.up, l.down, l.reset] {
                assert!(button[2] >= 20.0 * scale && button[3] >= 20.0 * scale);
            }
            assert!(l.reset[2] >= 20.0 * scale && l.value_box[3] >= 24.0 * scale);
            assert!(r[3] >= style::SPACE_SLIDER_HEIGHT * scale - 0.01);
        }
        // Groups are apart: a heading after the first has room above its text, and the Panel section more.
        let units = menu
            .rows
            .iter()
            .position(|r| matches!(r, MenuRow::Heading(t) if t == "Scale units"))
            .unwrap();
        let section = menu
            .rows
            .iter()
            .position(|r| matches!(r, MenuRow::Heading(t) if t == "Panel"))
            .unwrap();
        let first = menu.own_rect(rect, 0, scale);
        let first_full = menu.row_rect(rect, 0, scale);
        assert_eq!(first, first_full, "the first heading has no room above it");
        let (u, u_full) = (
            menu.own_rect(rect, units, scale),
            menu.row_rect(rect, units, scale),
        );
        let (p, p_full) = (
            menu.own_rect(rect, section, scale),
            menu.row_rect(rect, section, scale),
        );
        assert!(u[1] - u_full[1] >= style::SPACE_POPOVER_GROUP_GAP * scale - 0.01);
        assert!(
            p[1] - p_full[1] > u[1] - u_full[1],
            "the Panel section is set further apart"
        );
        // Everything sits inside the popover.
        for prim in popover(&menu, rect, [-1.0, -1.0], scale) {
            let (Prim::Rect { rect: r, .. } | Prim::Text { rect: r, .. }) = prim;
            assert!(
                r[0] >= rect[0] - 2.0 * scale && r[0] + r[2] <= rect[0] + rect[2] + 2.0 * scale
            );
            assert!(
                r[1] >= rect[1] - 2.0 * scale && r[1] + r[3] <= rect[1] + rect[3] + 2.0 * scale
            );
        }
    }
}

#[test]
fn the_parts_of_an_amount_row_are_told_apart_by_where_a_press_lands() {
    for scale in SCALES {
        let menu = menu_for(&PanelSettings::default(), "P");
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let row = row_of(&menu, "brightness");
        let l = menu.slider_layout(rect, row, scale);
        let hit = |r: [f32; 4]| menu.amount_hit(rect, middle(r), scale);
        assert_eq!(hit(l.thumb), Some(("brightness", AmountHit::Track)));
        assert_eq!(hit(l.number), Some(("brightness", AmountHit::Field)));
        assert_eq!(hit(l.up), Some(("brightness", AmountHit::Up)));
        assert_eq!(hit(l.down), Some(("brightness", AmountHit::Down)));
        assert_eq!(hit(l.reset), Some(("brightness", AmountHit::Reset)));
        let label = menu.label_rect(rect, row, scale);
        assert_eq!(hit(label), None, "the label is not a control");
        // The arrows sit side by side at the box's right, each as tall as the box, and neither reaches the other.
        assert!(contains(l.up, [l.up[0] + 1.0, l.up[1] + l.up[3] - 1.0]));
        assert!(!contains(l.up, [l.down[0] + 1.0, l.down[1] + 1.0]));
        assert!(!contains(l.down, [l.up[0] + 1.0, l.up[1] + 1.0]));
        // Not a hit above the first amount row or on a choice row.
        assert_eq!(
            menu.amount_hit(rect, [l.number[0], rect[1] + 1.0], scale),
            None
        );
    }
}

#[test]
fn the_arrows_step_by_the_slider_step_and_ten_steps_with_shift_and_clamp_at_the_ends() {
    for scale in SCALES {
        let mut menu = menu_for(&PanelSettings::default(), "P");
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let l = menu.slider_layout(rect, row_of(&menu, "brightness"), scale);
        let press =
            |menu: &mut Menu, part: [f32; 4], big: bool| menu.press(rect, middle(part), scale, big);
        let up = press(&mut menu, l.up, false);
        assert!(up.consumed);
        assert_eq!(up.changes[0].value, 105.0);
        assert_eq!(press(&mut menu, l.down, false).changes[0].value, 100.0);
        assert_eq!(press(&mut menu, l.up, true).changes[0].value, 150.0);
        assert_eq!(press(&mut menu, l.up, true).changes[0].value, 200.0);
        assert!(
            press(&mut menu, l.up, true).changes.is_empty(),
            "already at the top: nothing changes"
        );
        assert_eq!(menu.value_of("brightness"), Some(200.0));
        for _ in 0..5 {
            press(&mut menu, l.down, true);
        }
        assert_eq!(menu.value_of("brightness"), Some(0.0));
        // A fine step keeps its decimals clean.
        let s = menu.slider_layout(rect, row_of(&menu, "spread"), scale);
        for _ in 0..3 {
            press(&mut menu, s.up, false);
        }
        assert_eq!(menu.value_of("spread"), Some(1.15));
    }
}

#[test]
fn typing_in_the_value_box_accepts_on_enter_clamps_and_escape_cancels() {
    for scale in SCALES {
        let mut menu = menu_for(&PanelSettings::default(), "P");
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let l = menu.slider_layout(rect, row_of(&menu, "brightness"), scale);
        let type_in = |menu: &mut Menu, text: &str| {
            for c in text.chars() {
                assert_eq!(
                    menu.key(EditKey::Char(c)),
                    None,
                    "nothing applies until Enter"
                );
            }
        };
        // A press on the number starts typing; the first key replaces the number.
        assert!(menu.press(rect, middle(l.number), scale, false).consumed);
        assert_eq!(menu.editing(), Some(("brightness", "100")));
        type_in(&mut menu, "150");
        assert_eq!(menu.editing(), Some(("brightness", "150")));
        assert_eq!(menu.value_of("brightness"), Some(100.0));
        let shown: Vec<String> = popover(&menu, rect, [-1.0, -1.0], scale)
            .into_iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect();
        assert!(shown.contains(&"150|".to_string()), "{shown:?}");
        let accepted = menu.key(EditKey::Enter).expect("Enter accepts");
        assert_eq!(accepted.value, 150.0);
        assert_eq!(menu.editing(), None);
        // Out of range clamps to the slider's range.
        menu.press(rect, middle(l.number), scale, false);
        type_in(&mut menu, "9999");
        assert_eq!(menu.key(EditKey::Enter).unwrap().value, 200.0);
        menu.press(rect, middle(l.number), scale, false);
        type_in(&mut menu, "-40");
        assert_eq!(menu.key(EditKey::Enter).unwrap().value, 0.0);
        // Letters are ignored; Backspace edits; a comma is a decimal point.
        menu.press(rect, middle(l.number), scale, false);
        type_in(&mut menu, "7a5x");
        assert_eq!(menu.editing().unwrap().1, "75");
        menu.key(EditKey::Backspace);
        assert_eq!(menu.editing().unwrap().1, "7");
        menu.key(EditKey::Char(','));
        menu.key(EditKey::Char('5'));
        assert_eq!(menu.editing().unwrap().1, "7.5");
        // Escape cancels: the value from before typing is back.
        menu.key(EditKey::Enter);
        let before = menu.value_of("brightness").unwrap();
        menu.press(rect, middle(l.number), scale, false);
        type_in(&mut menu, "120");
        assert_eq!(menu.key(EditKey::Escape), None);
        assert_eq!(menu.editing(), None);
        assert_eq!(menu.value_of("brightness"), Some(before));
        // Text that is not a number keeps the old value.
        menu.press(rect, middle(l.number), scale, false);
        type_in(&mut menu, "-");
        assert_eq!(menu.key(EditKey::Enter), None);
        assert_eq!(menu.value_of("brightness"), Some(before));
        // Keys with nothing being typed do nothing.
        assert_eq!(menu.key(EditKey::Up(false)), None);
    }
}

#[test]
fn up_and_down_step_the_value_being_typed_shift_for_big_steps_and_escape_puts_it_back() {
    let mut menu = menu_for(&PanelSettings::default(), "P");
    let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], 1.0);
    let l = menu.slider_layout(rect, row_of(&menu, "brightness"), 1.0);
    menu.press(rect, middle(l.number), 1.0, false);
    assert_eq!(menu.key(EditKey::Up(false)).unwrap().value, 105.0);
    assert_eq!(menu.key(EditKey::Up(true)).unwrap().value, 155.0);
    assert_eq!(menu.key(EditKey::Down(false)).unwrap().value, 150.0);
    assert_eq!(menu.editing(), Some(("brightness", "150")));
    // Escape puts back the value from before typing began, as one change.
    let back = menu
        .key(EditKey::Escape)
        .expect("the stepped value is undone");
    assert_eq!(back.value, 100.0);
    assert_eq!(menu.value_of("brightness"), Some(100.0));
    // Enter after a step keeps the stepped value.
    menu.press(rect, middle(l.number), 1.0, false);
    menu.key(EditKey::Up(true));
    assert_eq!(menu.key(EditKey::Enter), None, "already applied");
    assert_eq!(menu.value_of("brightness"), Some(150.0));
}

#[test]
fn a_press_elsewhere_accepts_what_was_typed() {
    let mut menu = menu_for(&PanelSettings::default(), "P");
    let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], 1.0);
    let l = menu.slider_layout(rect, row_of(&menu, "brightness"), 1.0);
    menu.press(rect, middle(l.number), 1.0, false);
    for c in "30".chars() {
        menu.key(EditKey::Char(c));
    }
    let pressed = menu.press(rect, [rect[0] + 2.0, rect[1] + rect[3] - 2.0], 1.0, false);
    assert!(!pressed.consumed);
    assert_eq!(pressed.changes.len(), 1);
    assert_eq!(pressed.changes[0].value, 30.0);
    assert_eq!(menu.editing(), None);
}

#[test]
fn the_reset_icon_puts_one_amount_back_and_is_dim_at_its_default() {
    for scale in SCALES {
        let mut settings = PanelSettings::default();
        remember(&mut settings, "P", "brightness", 150.0);
        let mut menu = menu_for(&settings, "P");
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let (b, s) = (row_of(&menu, "brightness"), row_of(&menu, "spread"));
        let reset_b = middle(menu.slider_layout(rect, b, scale).reset);
        let reset_s = middle(menu.slider_layout(rect, s, scale).reset);
        // Away from its default the icon is lit, the tooltip names the default, and a press goes back.
        assert_eq!(
            menu.reset_at(rect, reset_b, scale),
            Some(("brightness", 100.0))
        );
        let (anchor, words) = popover_tip(&menu, rect, reset_b, scale).unwrap();
        assert_eq!(words, "Reset to 100 %");
        assert!(contains(anchor, reset_b));
        // At its default it is dim and does nothing.
        assert_eq!(menu.reset_at(rect, reset_s, scale), None);
        assert!(menu.press(rect, reset_s, scale, false).changes.is_empty());
        assert_eq!(
            popover_tip(&menu, rect, reset_s, scale).unwrap().1,
            "Reset to 1.00 px"
        );
        let pressed = menu.press(rect, reset_b, scale, false);
        assert!(pressed.consumed);
        assert_eq!(pressed.changes.len(), 1);
        assert_eq!(pressed.changes[0].value, 100.0);
        assert_eq!(menu.value_of("brightness"), Some(100.0));
        assert_eq!(menu.reset_at(rect, reset_b, scale), None, "now dim");
        // Nowhere else shows a tooltip.
        assert_eq!(
            popover_tip(&menu, rect, [rect[0] + 1.0, rect[1] + 1.0], scale),
            None
        );
    }
}

#[test]
fn the_panel_reset_sits_in_the_panel_sections_corner_and_is_dim_when_all_is_default() {
    for scale in SCALES {
        let mut settings = PanelSettings::default();
        let menu = menu_for(&settings, "P");
        assert!(menu.panel_at_default);
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let icon = menu.panel_reset_rect(rect, scale).expect("a Panel section");
        let section = menu
            .rows
            .iter()
            .position(|r| matches!(r, MenuRow::Heading(t) if t == "Panel"))
            .unwrap();
        let heading = menu.own_rect(rect, section, scale);
        assert!(icon[1] >= heading[1] && icon[1] + icon[3] <= heading[1] + heading[3] + 0.01);
        assert!(
            icon[0] + icon[2] <= rect[0] + rect[2],
            "in the corner, inside"
        );
        assert!(icon[2] >= 20.0 * scale);
        assert_eq!(
            popover_tip(&menu, rect, middle(icon), scale).unwrap().1,
            "Reset this Panel's settings"
        );
        // Dim at default: a press is taken but resets nothing.
        let mut dim = menu.clone();
        let pressed = dim.press(rect, middle(icon), scale, false);
        assert!(pressed.consumed && !pressed.reset_panel);
        // Away from default it is lit and a press asks for the reset.
        remember(&mut settings, "P", "spread", 2.0);
        settings.set("P", "units", "percent");
        let mut menu = menu_for(&settings, "P");
        assert!(!menu.panel_at_default);
        let pressed = menu.press(rect, middle(icon), scale, false);
        assert!(pressed.consumed && pressed.reset_panel && pressed.changes.is_empty());
        // A lit icon is not drawn the same as a dim one.
        let ink = |m: &Menu| {
            popover(m, rect, [-1.0, -1.0], scale)
                .into_iter()
                .find_map(|p| match p {
                    Prim::Text {
                        text,
                        colour,
                        rect: r,
                        ..
                    } if text == "\u{E148}" && r == icon => Some(colour),
                    _ => None,
                })
                .expect("the Panel reset is drawn")
        };
        assert_ne!(ink(&dim), ink(&menu));
    }
}

#[test]
fn resetting_a_panel_puts_its_settings_back_and_touches_no_other_instance_or_state() {
    let mut settings = PanelSettings::default();
    for panel in ["Waveform", "Waveform 2"] {
        settings.set(panel, "units", "percent");
        remember(&mut settings, panel, "brightness", 150.0);
        settings.set_state(panel, "zoom", "2");
    }
    settings.set("Vectorscope", "units", "percent");
    let before_version = settings.version();
    settings.reset_settings("Waveform");
    assert_ne!(settings.version(), before_version, "readers see it change");
    let reset = settings.resolve("Waveform", &DEFS);
    assert_eq!(reset["units"], "codes");
    assert_eq!(reset["brightness"], Amount::store(100.0));
    // The other instance of the same display, and another display, are as they were.
    let other = settings.resolve("Waveform 2", &DEFS);
    assert_eq!(other["units"], "percent");
    assert_eq!(other["brightness"], Amount::store(150.0));
    assert_eq!(settings.resolve("Vectorscope", &DEFS)["units"], "percent");
    // What the display remembers for itself (a zoom) is not a setting.
    assert_eq!(settings.state_of("Waveform", &["zoom"])["zoom"], "2");
    // Nothing to reset: no change is reported.
    let version = settings.version();
    settings.reset_settings("Waveform");
    assert_eq!(settings.version(), version);
    // The popover reads as all-default again.
    let menu = menu_for(&settings, "Waveform");
    assert!(menu.panel_at_default);
    assert!(!menu_for(&settings, "Waveform 2").panel_at_default);
}

#[test]
fn syncing_shows_the_settings_again() {
    let mut settings = PanelSettings::default();
    let mut menu = menu_for(&settings, "P");
    remember(&mut settings, "P", "brightness", 175.0);
    settings.set("P", "weights", "b");
    menu.sync(&settings.resolve("P", &DEFS));
    assert_eq!(menu.value_of("brightness"), Some(175.0));
    assert!(menu.rows.iter().any(|r| matches!(
        r,
        MenuRow::Choice {
            value: "b",
            on: true,
            ..
        }
    )));
}

#[test]
fn a_panel_with_no_settings_has_a_panel_reset_that_is_dim() {
    let menu = Menu::for_panel(0, &[], &Default::default());
    let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], 1.0);
    assert!(menu.panel_at_default);
    let icon = menu.panel_reset_rect(rect, 1.0).unwrap();
    let mut menu = menu;
    assert!(!menu.press(rect, middle(icon), 1.0, false).reset_panel);
}

#[test]
fn resetting_all_panels_puts_every_instance_back_and_keeps_state_and_link() {
    let mut settings = PanelSettings::default();
    // Two instances of one display (on two pages) and another display, all set up.
    for panel in ["Waveform", "Waveform 2", "Vectorscope"] {
        settings.set(panel, "units", "percent");
        remember(&mut settings, panel, "brightness", 150.0);
        settings.set_state(panel, "zoom", "2");
    }
    scopecreep::zoom::remember_link(&mut settings, true);
    let before = settings.version();
    settings.reset_all_settings();
    assert_ne!(settings.version(), before, "readers see it change");
    for panel in ["Waveform", "Waveform 2"] {
        let reset = settings.resolve(panel, &DEFS);
        assert_eq!(reset["units"], "codes");
        assert_eq!(reset["brightness"], Amount::store(100.0));
        assert_eq!(settings.state_of(panel, &["zoom"])["zoom"], "2");
        assert!(menu_for(&settings, panel).panel_at_default);
    }
    assert_eq!(settings.resolve("Vectorscope", &DEFS)["units"], "codes");
    assert!(scopecreep::zoom::link_remembered(&settings), "Link stays");
    // Nothing to reset: no change is reported.
    let version = settings.version();
    settings.reset_all_settings();
    assert_eq!(settings.version(), version);
}

// ---- The popover reads like the demo (#237) ----

/// The widest amount row the popovers have: a long label and "1023 codes".
const HIGH_REFERENCE: Amount = Amount {
    min: 0.0,
    max: 1023.0,
    step: 1.0,
    default: 940.0,
    unit: " codes",
    decimals: 0,
    legacy: &[],
};

fn wide_menu() -> Menu {
    let defs = [
        SettingDef {
            key: "units",
            label: "Scale units",
            amount: None,
            choices: &[Choice {
                value: "codes",
                text: "10-bit codes",
            }],
        },
        SettingDef {
            key: "high",
            label: "High reference level",
            amount: Some(HIGH_REFERENCE),
            choices: &[],
        },
    ];
    let mut current = PanelSettings::default().resolve("P", &defs);
    current.insert("high".to_string(), Amount::store(1023.0));
    Menu::for_panel(0, &defs, &current)
}

fn rect_prims(prims: &[Prim]) -> Vec<[f32; 4]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

#[test]
fn the_longest_value_and_label_never_touch_the_arrows_or_the_edges() {
    for scale in SCALES {
        let menu = wide_menu();
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let row = row_of(&menu, "high");
        let l = menu.slider_layout(rect, row, scale);
        // Ten characters at the widest a digit or letter runs (0.6 of the 13 px type).
        let wanted = 10.0 * 0.6 * style::TYPE_LABEL * scale;
        assert!(
            wanted + 4.0 * scale <= l.number[2],
            "{wanted} in {} at {scale}",
            l.number[2]
        );
        // The number's part, then the two arrows, one after the other inside the box.
        assert!(l.number[0] + l.number[2] <= l.down[0] + 0.01);
        assert!(l.down[0] + l.down[2] <= l.up[0] + 0.01);
        assert!(l.up[0] + l.up[2] <= l.value_box[0] + l.value_box[2] + 0.01);
    }
}

#[test]
fn the_arrows_and_the_reset_are_icons_that_light_under_the_pointer() {
    for scale in SCALES {
        let menu = wide_menu();
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let row = row_of(&menu, "high");
        let l = menu.slider_layout(rect, row, scale);
        let away = [rect[0] + 1.0, rect[1] + 1.0];
        let rest = popover(&menu, rect, away, scale);
        // The icon drawn in `button`: its glyph, face and colour.
        let icon_in = |prims: &[Prim], button: [f32; 4]| -> (String, Face, [u8; 3]) {
            prims
                .iter()
                .find_map(|p| match p {
                    Prim::Text {
                        text,
                        rect: r,
                        face: Face::Icon,
                        colour,
                        ..
                    } if *r == button => Some((text.clone(), Face::Icon, *colour)),
                    _ => None,
                })
                .expect("an icon is drawn in the button")
        };
        let (up, down, reset) = (
            (l.up, Icon::ChevronUp),
            (l.down, Icon::ChevronDown),
            (l.reset, Icon::Reset),
        );
        for (button, icon) in [up, down, reset] {
            // The icon (16 logical px) sits inside its button.
            assert!(
                style::TYPE_ICON * scale <= button[2] + 0.01
                    && style::TYPE_ICON * scale <= button[3]
            );
            let (glyph, _, ink) = icon_in(&rest, button);
            assert_eq!(glyph, icon.glyph().to_string());
            // Lit under the pointer: white text.
            let lit = popover(&menu, rect, middle(button), scale);
            let (_, _, lit_ink) = icon_in(&lit, button);
            assert_ne!(ink, lit_ink);
            assert_eq!(lit_ink, style::TEXT_HIGH);
        }
    }
}

#[test]
fn groups_are_apart_with_a_quiet_line_before_the_amounts_and_before_the_panel_section() {
    for scale in SCALES {
        let menu = wide_menu();
        let rect = menu.rect([50.0, 50.0], [1600.0, 1200.0], scale);
        let (high, section) = (row_of(&menu, "high"), menu.rows.len() - 5);
        let choice = menu.row_rect(rect, high - 1, scale);
        let amount = menu.own_rect(rect, high, scale);
        let gap = |below: [f32; 4], above: [f32; 4]| below[1] - (above[1] + above[3]);
        assert!(gap(amount, choice) >= style::SPACE_POPOVER_GROUP_GAP * scale - 0.01);
        let heading = menu.own_rect(rect, section, scale);
        assert!(gap(heading, amount) >= style::SPACE_POPOVER_GROUP_GAP * scale - 0.01);
        // The amount row keeps its 34 px.
        assert!((amount[3] - style::SPACE_POPOVER_ROW * scale).abs() < 0.01);
        // A thin line, most of the popover wide, sits in each gap.
        let prims = popover(&menu, rect, [0.0, 0.0], scale);
        let lines = rect_prims(&prims)
            .into_iter()
            .filter(|r| r[3] <= scale.max(1.0) + 0.01 && r[2] > rect[2] / 2.0)
            .count();
        assert!(lines >= 2, "{lines} group lines");
    }
}
