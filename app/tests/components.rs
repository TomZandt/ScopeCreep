//! The style components on the pure seam (#115): size, hit area, states and drawing, at 1x and 2x. No GPU.
use scopecreep::build_info::Build;
use scopecreep::commands::Command;
use scopecreep::components::*;
use scopecreep::settings::{Choice, Menu, PanelSettings, SettingDef};
use scopecreep::style;

const SCALES: [f32; 2] = [1.0, 2.0];

fn fills(prims: &[Prim]) -> Vec<[f32; 4]> {
    prims
        .iter()
        .filter_map(|p| {
            if let Prim::Rect { colour, .. } = p {
                Some(*colour)
            } else {
                None
            }
        })
        .collect()
}

fn texts(prims: &[Prim]) -> Vec<String> {
    prims
        .iter()
        .filter_map(|p| {
            if let Prim::Text { text, .. } = p {
                Some(text.clone())
            } else {
                None
            }
        })
        .collect()
}

fn neutral(c: [f32; 4]) -> bool {
    c[0] == c[1] && c[1] == c[2]
}

#[test]
fn the_new_chrome_tokens_are_neutral_except_the_channel_colours() {
    for c in [
        style::SURFACE_PANEL,
        style::SURFACE_CONTROL,
        style::STATE_PRESSED,
        style::SURFACE_CONTROL_ON,
        style::SURFACE_CONTROL_PRESSED,
        style::STATE_ON_HOVER,
        style::LINE_RULE,
        style::SURFACE_DOCK_WASH,
        style::STATE_FOCUS,
        style::CHANNEL_Y,
    ] {
        assert!(c[0] == c[1] && c[1] == c[2], "{c:?} tints the eye");
    }
    for c in [
        style::TEXT_ON_ACCENT,
        style::TEXT_DISABLED,
        style::TEXT_HIGH,
        style::TEXT_PRIMARY,
        style::TEXT_SECONDARY,
        style::TEXT_MUTED,
    ] {
        assert!(c[0] == c[1] && c[1] == c[2]);
    }
    // The picture stays the brightest thing: no chrome fill reaches white.
    assert!(style::CHANNEL_R.iter().all(|v| *v < 0.9));
}

#[test]
fn a_chip_has_one_size_description_that_scales_with_the_display() {
    let chip = Chip::new("R");
    let (one, two) = (chip.size(1.0), chip.size(2.0));
    let wide = 2.0 * style::SPACE_CHIP_PAD + style::SPACE_CHAR_WIDTH;
    assert_eq!(
        one,
        [
            style::SPACE_CHIP_MIN_WIDTH.max(wide),
            style::SPACE_CHIP_HEIGHT
        ]
    );
    assert_eq!(two, [one[0] * 2.0, one[1] * 2.0]);
    let long = Chip::new("Heatmap");
    assert!(
        long.size(1.0)[0] > one[0],
        "a longer word makes a wider chip"
    );
}

#[test]
fn a_chip_hit_area_is_its_rectangle_at_every_scale() {
    for scale in SCALES {
        let chip = Chip::new("G");
        let r = chip.rect([100.0, 50.0], scale);
        assert!(contains(r, [r[0] + 1.0, r[1] + 1.0]));
        assert!(contains(r, [r[0] + r[2] - 1.0, r[1] + r[3] - 1.0]));
        assert!(!contains(r, [r[0] + r[2] + 1.0, r[1]]));
        assert!(!contains(r, [r[0] - 1.0, r[1]]));
    }
}

#[test]
fn every_state_of_a_chip_looks_different_and_on_is_not_off() {
    for scale in SCALES {
        let chip = Chip::new("Y");
        let mut seen = Vec::new();
        for on in [false, true] {
            for state in STATES {
                let prims = chip.draw([0.0, 0.0], on, state, scale);
                seen.push((
                    fills(&prims)[0],
                    prims.iter().find_map(|p| match p {
                        Prim::Text { colour, .. } => Some(*colour),
                        _ => None,
                    }),
                ));
            }
        }
        for (i, a) in seen.iter().enumerate() {
            for b in &seen[i + 1..] {
                assert_ne!(a, b, "two states of a chip look the same");
            }
        }
    }
}

#[test]
fn a_channel_chip_is_in_its_colour_when_on_and_grey_when_off() {
    for (channel, colour) in [
        (Channel::R, style::CHANNEL_R),
        (Channel::G, style::CHANNEL_G),
        (Channel::B, style::CHANNEL_B),
    ] {
        let chip = Chip::channel("x", channel);
        assert_eq!(
            fills(&chip.draw([0.0; 2], true, State::Normal, 1.0))[0],
            style::rgba(colour)
        );
        assert!(
            neutral(fills(&chip.draw([0.0; 2], false, State::Normal, 1.0))[0]),
            "off is grey"
        );
        assert!(neutral(
            fills(&chip.draw([0.0; 2], false, State::Hover, 1.0))[0]
        ));
    }
    assert!(
        neutral(fills(&Chip::channel("Y", Channel::Y).draw([0.0; 2], true, State::Normal, 1.0))[0]),
        "luma is grey"
    );
}

#[test]
fn a_disabled_chip_does_not_light_under_the_pointer() {
    let chip = Chip::new("A");
    assert_eq!(
        chip.draw([0.0; 2], false, State::Disabled, 1.0),
        chip.draw([0.0; 2], false, State::Disabled, 1.0)
    );
    let disabled = fills(&chip.draw([0.0; 2], false, State::Disabled, 1.0))[0];
    assert_ne!(
        disabled,
        fills(&chip.draw([0.0; 2], false, State::Hover, 1.0))[0]
    );
    assert_eq!(disabled, style::rgba(style::SURFACE_CONTROL));
}

#[test]
fn single_choice_chips_turn_the_clicked_one_on_and_the_rest_off() {
    assert_eq!(
        clicked(ChipMode::Single, &[true, false, false, false], 2),
        [false, false, true, false]
    );
    assert_eq!(
        clicked(ChipMode::Single, &[false, false, true, false], 2),
        [false, false, true, false],
        "clicking the one that is on keeps it on"
    );
}

#[test]
fn multiple_choice_chips_combine_but_never_all_go_off() {
    assert_eq!(
        clicked(ChipMode::Multiple, &[true, false, false, false], 1),
        [true, true, false, false]
    );
    assert_eq!(
        clicked(ChipMode::Multiple, &[true, true, false, false], 0),
        [false, true, false, false]
    );
    assert_eq!(
        clicked(ChipMode::Multiple, &[false, true, false, false], 1),
        [false, true, false, false],
        "the last one stays on"
    );
}

#[test]
fn one_or_none_chips_switch_turn_off_on_a_second_click_and_start_with_none() {
    assert_eq!(
        clicked(ChipMode::OneOrNone, &[false, false, false, false], 2),
        [false, false, true, false],
        "none to one"
    );
    assert_eq!(
        clicked(ChipMode::OneOrNone, &[false, false, true, false], 3),
        [false, false, false, true],
        "one to another"
    );
    assert_eq!(
        clicked(ChipMode::OneOrNone, &[false, false, true, false], 2),
        [false, false, false, false],
        "the lit one turns off"
    );
}

#[test]
fn a_toggle_flips_and_may_be_off() {
    assert_eq!(clicked(ChipMode::Toggle, &[false], 0), [true]);
    assert_eq!(clicked(ChipMode::Toggle, &[true], 0), [false]);
}

#[test]
fn chips_in_a_row_sit_left_to_right_a_gap_apart() {
    for scale in SCALES {
        let chips = [
            Chip::new("Y"),
            Chip::new("R"),
            Chip::new("G"),
            Chip::new("B"),
        ];
        let rects = chip_row(&chips, [10.0, 5.0], scale);
        for pair in rects.windows(2) {
            assert_eq!(
                pair[1][0],
                pair[0][0] + pair[0][2] + style::SPACE_CHIP_GAP * scale
            );
            assert_eq!(pair[0][1], pair[1][1]);
        }
        let last = rects.last().unwrap();
        assert!((last[0] + last[2] - 10.0 - chip_row_width(&chips, scale)).abs() < 1e-3);
    }
}

#[test]
fn a_segmented_control_finds_the_segment_under_the_pointer() {
    for scale in SCALES {
        let control = Segmented::new(&["Luma", "Chroma", "Both"]);
        let rects = control.rects([20.0, 20.0], scale);
        assert_eq!(rects.len(), 3);
        for (i, r) in rects.iter().enumerate() {
            assert_eq!(
                control.hit([20.0, 20.0], [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0], scale),
                Some(i)
            );
            assert_eq!(r[3], style::SPACE_SEGMENT_HEIGHT * scale);
        }
        assert_eq!(control.hit([20.0, 20.0], [0.0, 0.0], scale), None);
        let whole = control.size(scale);
        assert!((rects[2][0] + rects[2][2] - 20.0 - whole[0]).abs() < 1e-3);
    }
}

#[test]
fn a_segmented_control_marks_the_selected_segment_and_the_one_under_the_pointer() {
    let control = Segmented::new(&["A", "B"]);
    let normal = fills(&control.draw([0.0; 2], 0, 1, State::Normal, 1.0));
    let hover = fills(&control.draw([0.0; 2], 0, 1, State::Hover, 1.0));
    assert_ne!(normal, hover);
    assert_eq!(
        normal[1],
        style::rgba(style::SURFACE_CONTROL_ON),
        "the selected segment is on: a lighter grey with an accent line"
    );
    assert!(
        normal.contains(&style::rgba(style::accent())),
        "the selected segment has the accent line"
    );
    assert!(
        fills(&control.draw([0.0; 2], 0, 1, State::Disabled, 1.0))
            .iter()
            .all(|c| *c != style::rgba(style::accent())),
        "disabled: nothing is lit"
    );
}

#[test]
fn a_slider_puts_its_thumb_along_the_track_by_value_and_its_number_beside_it() {
    for scale in SCALES {
        for value in [0.0f32, 0.5, 1.0] {
            let slider = Slider {
                value,
                number: "75 %".into(),
            };
            let l = slider.layout([10.0, 10.0], scale);
            let centre = l.thumb[0] + l.thumb[2] / 2.0;
            assert!((centre - (l.track[0] + value * l.track[2])).abs() < 1e-3);
            assert!((l.filled[2] - value * l.track[2]).abs() < 1e-3);
            assert!(
                l.number[0] >= l.thumb[0] + l.thumb[2],
                "the number sits beside the track"
            );
            assert_eq!(l.row[3], style::SPACE_SLIDER_HEIGHT * scale);
            assert!(l.row[0] + l.row[2] >= l.number[0] + l.number[2] - 1e-3);
        }
    }
}

#[test]
fn a_slider_turns_a_pointer_position_into_a_value_and_clamps_it() {
    for scale in SCALES {
        let slider = Slider {
            value: 0.0,
            number: String::new(),
        };
        let l = slider.layout([0.0, 0.0], scale);
        assert!(
            (slider.value_at([0.0, 0.0], l.track[0] + l.track[2] / 4.0, scale) - 0.25).abs() < 1e-4
        );
        assert_eq!(slider.value_at([0.0, 0.0], -50.0, scale), 0.0);
        assert_eq!(slider.value_at([0.0, 0.0], 5000.0, scale), 1.0);
        let grab = slider.grab([0.0, 0.0], scale);
        assert!(
            contains(grab, [l.thumb[0] + 1.0, l.thumb[1] + 1.0]),
            "the thumb is inside the hit area"
        );
        assert!(
            grab[3] >= l.thumb[3],
            "the hit area is at least as tall as the thumb"
        );
    }
}

#[test]
fn a_slider_states_differ_and_a_disabled_one_is_dimmed() {
    let slider = Slider {
        value: 0.5,
        number: "50".into(),
    };
    let thumbs: Vec<_> = STATES
        .iter()
        .map(|s| fills(&slider.draw([0.0; 2], *s, 1.0))[2])
        .collect();
    for (i, a) in thumbs.iter().enumerate() {
        for b in &thumbs[i + 1..] {
            assert_ne!(a, b);
        }
    }
    // The number, then the two arrows and the reset icon, which are Lucide icons (#238).
    assert_eq!(
        texts(&slider.draw([0.0; 2], State::Normal, 1.0)),
        ["50", "\u{E06D}", "\u{E070}", "\u{E148}"]
    );
}

const DEFS: [SettingDef; 2] = [
    SettingDef {
        key: "weights",
        label: "Weights",
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
        key: "spread",
        label: "Spread",
        amount: None,
        choices: &[
            Choice {
                value: "on",
                text: "On",
            },
            Choice {
                value: "off",
                text: "Off",
            },
        ],
    },
];

#[test]
fn the_popover_draws_the_menu_with_its_frame_its_ticks_and_the_row_under_the_pointer() {
    for scale in SCALES {
        let menu = Menu::new(0, &DEFS, &PanelSettings::default().resolve("P", &DEFS)).unwrap();
        let area = menu.rect([10.0, 10.0], [800.0 * scale, 600.0 * scale], scale);
        let away = popover(&menu, area, [-1.0, -1.0], scale);
        assert_eq!(
            texts(&away),
            ["Weights", "Alpha", "Beta", "Spread", "On", "Off"],
            "every row, in order"
        );
        // Border, fill, one tick per setting and the line above the second group.
        assert_eq!(fills(&away).len(), 5);
        assert_eq!(fills(&away)[1], style::rgba(style::SURFACE_MENU));
        // The pointer over "Beta" (row 2) lights that row.
        let over = [
            area[0] + 5.0 * scale,
            menu.row_top(area, 2, scale) + 2.0 * scale,
        ];
        let lit = popover(&menu, area, over, scale);
        // The row's lift and the accent bar at its left.
        assert_eq!(fills(&lit).len(), 7);
        assert!(fills(&lit).contains(&style::rgba(style::accent())));
        assert!(fills(&lit).contains(&style::rgba(style::STATE_HOVER)));
        // A heading never lights.
        let heading = [
            area[0] + 5.0 * scale,
            menu.row_top(area, 0, scale) + 2.0 * scale,
        ];
        assert_eq!(fills(&popover(&menu, area, heading, scale)).len(), 5);
    }
}

#[test]
fn a_tooltip_sits_under_what_it_names_and_always_inside_the_window() {
    for scale in SCALES {
        let tip = Tooltip::new("Red channel");
        let window = [400.0 * scale, 300.0 * scale];
        let anchor = [100.0 * scale, 20.0 * scale, 26.0 * scale, 20.0 * scale];
        let r = tip.rect(anchor, window, scale);
        assert!(r[1] >= anchor[1] + anchor[3], "below the anchor");
        assert_eq!([r[2], r[3]], tip.size(scale));
        // No room below: it goes above.
        let low = [
            100.0 * scale,
            window[1] - 22.0 * scale,
            26.0 * scale,
            20.0 * scale,
        ];
        assert!(tip.rect(low, window, scale)[1] + r[3] <= low[1]);
        // Near the right edge: pulled back inside.
        let edge = [
            window[0] - 10.0 * scale,
            20.0 * scale,
            10.0 * scale,
            20.0 * scale,
        ];
        let e = tip.rect(edge, window, scale);
        assert!(e[0] + e[2] <= window[0] + 1e-3 && e[0] >= 0.0);
        assert_eq!(texts(&tip.draw(r, scale)), ["Red channel"]);
    }
}

fn title(chips: usize) -> TitleBar {
    let all = [
        Chip::channel("Y", Channel::Y),
        Chip::channel("R", Channel::R),
        Chip::channel("G", Channel::G),
        Chip::channel("B", Channel::B),
    ];
    TitleBar {
        name: "Waveform".into(),
        tags: "Rec.709 weights".into(),
        chips: all[..chips].to_vec(),
        groups: vec![],
        settings_button: true,
        view: false,
    }
}

#[test]
fn a_wide_title_bar_shows_name_tags_chips_and_the_settings_button_in_order() {
    for scale in SCALES {
        let area = [0.0, 0.0, 500.0 * scale, 200.0 * scale];
        let l = title(4).layout(area, scale);
        assert_eq!(l.bar[3], style::SPACE_TITLE_BAR * scale);
        let tags = l.tags.expect("room for the tags");
        assert!(!l.name_cut && !l.tags_in_tooltip);
        assert!(l.name[0] + l.name[2] <= tags[0], "name, then tags");
        assert!(tags[0] + tags[2] <= l.chips[0][0], "then the chips");
        assert!(
            l.chips[3][0] + l.chips[3][2] <= l.settings.unwrap()[0],
            "then the settings button"
        );
        assert!(
            l.settings.unwrap()[0] + l.settings.unwrap()[2] <= area[2],
            "inside the bar"
        );
    }
}

#[test]
fn a_narrow_title_bar_keeps_the_chips_cuts_the_name_and_moves_the_tags_to_the_hover() {
    for scale in SCALES {
        let narrow = title(4).layout([0.0, 0.0, 190.0 * scale, 100.0 * scale], scale);
        assert_eq!(narrow.chips.len(), 4, "the chips stay");
        assert!(
            narrow.tags.is_none() && narrow.tags_in_tooltip,
            "the tags move to the hover"
        );
        // A bit wider: the whole name fits but the tags do not.
        let mid = title(4).layout([0.0, 0.0, 270.0 * scale, 100.0 * scale], scale);
        assert!(!mid.name_cut && mid.tags.is_none() && mid.tags_in_tooltip);
        // Narrower still: the name truncates before anything else moves.
        let tiny = title(4).layout([0.0, 0.0, 150.0 * scale, 100.0 * scale], scale);
        assert!(tiny.name_cut);
        assert!(
            tiny.name[0] + tiny.name[2] <= tiny.chips[0][0],
            "the name never runs under the chips"
        );
        assert!(tiny.chips.iter().all(|c| c[0] >= 0.0));
    }
}

#[test]
fn a_title_bar_without_tags_or_chips_has_nothing_to_move() {
    let bare = TitleBar {
        name: "Skin Tone".into(),
        tags: String::new(),
        chips: vec![],
        groups: vec![],
        settings_button: false,
        view: false,
    };
    let l = bare.layout([0.0, 0.0, 300.0, 100.0], 1.0);
    assert!(l.tags.is_none() && !l.tags_in_tooltip && l.settings.is_none() && l.chips.is_empty());
}

#[test]
fn the_title_bar_draws_its_name_tags_and_one_chip_each_in_its_state() {
    let bar = title(2);
    let prims = bar.draw(
        [0.0, 0.0, 500.0, 100.0],
        &[(true, State::Normal), (false, State::Hover)],
        State::Normal,
        1.0,
    );
    assert_eq!(
        texts(&prims),
        ["Waveform", "Rec.709 weights", "Y", "R", "\u{E0B6}"]
    );
    let lit = fills(&prims);
    assert!(
        lit.contains(&style::rgba(style::CHANNEL_Y)),
        "the on chip is in its channel's colour"
    );
    assert!(
        !lit.contains(&style::rgba(style::CHANNEL_R)),
        "the off chip is grey"
    );
}

#[test]
fn a_panel_gives_its_display_the_room_under_the_title_bar() {
    for scale in SCALES {
        let area = [10.0, 10.0, 300.0 * scale, 200.0 * scale];
        let content = panel_content(area, scale);
        assert_eq!(content[1], area[1] + style::SPACE_TITLE_BAR * scale);
        assert_eq!(content[1] + content[3], area[1] + area[3]);
        assert_eq!(panel_frame(area, scale).len(), 2, "border and fill");
    }
}

#[test]
fn the_top_bar_and_the_status_bar_hold_text_at_both_ends() {
    for scale in SCALES {
        assert_eq!(BarKind::Top.height(scale), style::SPACE_TOP_BAR * scale);
        assert_eq!(
            BarKind::Status.height(scale),
            style::SPACE_STATUS_ROW * scale
        );
        let prims = bar(
            BarKind::Status,
            0.0,
            0.0,
            600.0 * scale,
            "Playing",
            "Sender A",
            scale,
        );
        assert_eq!(texts(&prims), ["Playing", "Sender A"]);
    }
}

#[test]
fn a_legend_strip_is_horizontal_full_width_and_read_left_to_right() {
    for scale in SCALES {
        let strip = LegendStrip {
            items: (0..5)
                .map(|i| LegendItem {
                    colour: [i as f32 / 4.0; 3],
                    label: format!("{i}"),
                })
                .collect(),
        };
        let width = 500.0 * scale;
        let l = strip.layout(20.0, 300.0, width, scale);
        assert_eq!(l.bars.len(), 5);
        assert!((l.bars[0][0] - 20.0).abs() < 1e-3);
        assert!(
            (l.bars[4][0] + l.bars[4][2] - (20.0 + width)).abs() < 1e-2,
            "the bars fill the full width"
        );
        for pair in l.bars.windows(2) {
            assert!(pair[1][0] > pair[0][0], "left to right");
            assert_eq!(pair[0][1], pair[1][1], "one row: never a vertical column");
            assert_eq!(pair[0][3], pair[1][3]);
        }
        for (bar, label) in l.bars.iter().zip(&l.labels) {
            let label = label.expect("room for a one-digit label");
            assert!(label[1] >= bar[1] + bar[3], "labels sit under their bars");
        }
        assert_eq!(l.strip[3], style::SPACE_LEGEND_HEIGHT * scale);
    }
}

#[test]
fn a_legend_label_that_does_not_fit_its_bar_is_left_out() {
    let strip = LegendStrip {
        items: (0..15)
            .map(|_| LegendItem {
                colour: [0.5; 3],
                label: "+0.5".into(),
            })
            .collect(),
    };
    assert!(
        strip
            .layout(0.0, 0.0, 150.0, 1.0)
            .labels
            .iter()
            .all(Option::is_none)
    );
    assert!(
        strip
            .layout(0.0, 0.0, 1500.0, 1.0)
            .labels
            .iter()
            .all(Option::is_some)
    );
}

#[test]
fn a_legend_keeps_the_colours_it_is_given() {
    let data = [0.2, 0.4, 0.9];
    let strip = LegendStrip {
        items: vec![LegendItem {
            colour: data,
            label: "x".into(),
        }],
    };
    assert!(
        fills(&strip.draw(0.0, 0.0, 100.0, 1.0)).contains(&[0.2, 0.4, 0.9, 1.0]),
        "a band is data and is never themed"
    );
}

#[test]
fn the_showcase_draws_every_component_in_every_state_at_both_scales() {
    let (one, two) = (
        scopecreep::showcase::showcase(1.0),
        scopecreep::showcase::showcase(2.0),
    );
    assert!(
        (two.size[0] - 2.0 * one.size[0]).abs() < 1e-2
            && (two.size[1] - 2.0 * one.size[1]).abs() < 1e-2,
        "the 2x sheet is the 1x sheet doubled"
    );
    for sheet in [&one, &two] {
        let said = texts(&sheet.prims);
        for state in STATES {
            assert!(
                said.contains(&format!("{state:?}")),
                "a column for {state:?}"
            );
        }
        for part in [
            "Chip off",
            "Chip on",
            "R on",
            "G on",
            "B on",
            "Y on",
            "Segmented",
            "Slider with number",
            "Settings button",
            "Popover, Tooltip",
            "About popover",
            "Update dialog",
            "Title bar: focused, +N, Scope",
            "Tab strip, edge +",
            "Appearance: accent",
            "Top bar",
            "Status bar",
            "Legend strip",
        ] {
            assert!(said.contains(&part.to_string()), "a row for {part}");
        }
        for c in [style::CHANNEL_R, style::CHANNEL_G, style::CHANNEL_B] {
            assert!(fills(&sheet.prims).contains(&style::rgba(c)));
        }
    }
    assert_eq!(
        texts(&one.prims),
        texts(&two.prims),
        "the same sheet at either scale"
    );
}

// ---- About popover (#232, spec #230) ----

fn about() -> AboutPopover {
    AboutPopover::new(Build::new("1.2.3", "1.2"))
}

const WINDOW: [f32; 2] = [1420.0, 764.0];

fn inside(outer: [f32; 4], inner: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.01
        && inner[1] >= outer[1] - 0.01
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.01
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.01
}

fn centre_of(r: [f32; 4]) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

#[test]
fn the_about_popover_is_centred_in_the_window_and_scales_with_the_display() {
    let (one, two) = (
        about().layout(WINDOW, 1.0),
        about().layout([2.0 * WINDOW[0], 2.0 * WINDOW[1]], 2.0),
    );
    for (l, window, scale) in [
        (&one, WINDOW, 1.0),
        (&two, [2.0 * WINDOW[0], 2.0 * WINDOW[1]], 2.0),
    ] {
        let a = l.area;
        assert!((a[0] + a[2] / 2.0 - window[0] / 2.0).abs() < 0.5, "across");
        assert!((a[1] + a[3] / 2.0 - window[1] / 2.0).abs() < 0.5, "down");
        assert!((a[2] - style::SPACE_ABOUT_WIDTH * scale).abs() < 0.01);
    }
    assert!((two.area[2] - 2.0 * one.area[2]).abs() < 0.01);
    assert!((two.area[3] - 2.0 * one.area[3]).abs() < 0.01);
}

#[test]
fn the_about_popover_stays_inside_a_small_window() {
    for scale in [1.0, 2.0] {
        let window = [1015.0 * scale, 645.0 * scale];
        let a = about().layout(window, scale).area;
        assert!(inside([0.0, 0.0, window[0], window[1]], a));
    }
}

#[test]
fn the_about_popover_says_the_name_version_credit_and_licence_at_both_scales() {
    for scale in [1.0, 2.0] {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = about().layout(window, scale);
        let said = texts(&about().draw(window, [0.0, 0.0], scale));
        assert!(said.contains(&"ScopeCreep".to_string()), "the wordmark");
        assert!(
            said.contains(&"1.2.3 (Feed protocol 1.2)".to_string()),
            "the version and protocol --version prints: {said:?}"
        );
        assert_eq!(
            said.iter().filter(|s| s.contains("ScopeCreep")).count(),
            1,
            "the name shows once: {said:?}"
        );
        assert!(said.contains(&"by Tom Zandt-Valentine".to_string()));
        assert!(
            said.join(" ").contains(
                "\u{a9} 2026 Tom Zandt-Valentine \u{b7} free to use, including paid work; selling or bundling needs a commercial licence"
            ),
            "the licence line, wrapped inside the popover: {said:?}"
        );
        for p in about().draw(window, [0.0, 0.0], scale) {
            if let Prim::Text { rect, .. } = p {
                assert!(inside(l.area, rect), "text {rect:?} outside {:?}", l.area);
            }
        }
    }
}

#[test]
fn the_about_popover_links_are_website_licence_and_buy_me_a_coffee_with_real_hit_areas() {
    for scale in [1.0, 2.0] {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = about().layout(window, scale);
        let got: Vec<Command> = l.links.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            got,
            [Command::Website, Command::Licence, Command::BuyMeACoffee]
        );
        for (command, rect) in &l.links {
            assert!(inside(l.area, *rect));
            assert!(
                rect[3] >= style::SPACE_CHIP_HEIGHT * scale - 0.01,
                "tall enough to hit"
            );
            assert_eq!(
                about().hit(window, centre_of(*rect), scale),
                AboutHit::Link(*command)
            );
        }
        for pair in l.links.windows(2) {
            assert!(pair[0].1[0] + pair[0].1[2] <= pair[1].1[0], "no overlap");
        }
    }
}

#[test]
fn the_about_popover_close_control_and_outside_click_close_it() {
    for scale in [1.0, 2.0] {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = about().layout(window, scale);
        assert!(inside(l.area, l.close));
        assert_eq!(
            about().hit(window, centre_of(l.close), scale),
            AboutHit::Close
        );
        assert_eq!(about().hit(window, [1.0, 1.0], scale), AboutHit::Outside);
        let corner = [l.area[0] + 1.0, l.area[1] + l.area[3] - 1.0];
        assert_eq!(about().hit(window, corner, scale), AboutHit::Inside);
    }
}

#[test]
fn the_about_popover_lights_the_link_under_the_pointer() {
    let l = about().layout(WINDOW, 1.0);
    let rest = about().draw(WINDOW, [0.0, 0.0], 1.0);
    let over = about().draw(WINDOW, centre_of(l.links[0].1), 1.0);
    assert_ne!(rest, over);
}

#[test]
fn the_about_version_text_is_what_version_prints_after_the_name() {
    let build = Build::new("1.2.3", "1.2");
    assert_eq!(build.line(), "ScopeCreep 1.2.3 (Feed protocol 1.2)");
    assert_eq!(
        build.line(),
        format!(
            "ScopeCreep {}",
            AboutPopover::new(build.clone()).version_line()
        )
    );
}

#[test]
fn the_current_build_reads_the_package_version_and_feed_protocol() {
    let build = Build::current();
    assert_eq!(build.version, env!("CARGO_PKG_VERSION"));
    assert!(build.protocol.split('.').count() == 2 && build.line().starts_with("ScopeCreep "));
}

#[test]
fn the_about_popover_takes_every_key_and_only_escape_closes_it() {
    assert!(about().key(true));
    assert!(
        !about().key(false),
        "a hotkey must not reach the app behind it"
    );
}

// ---- Update dialog (#246, spec #234) ----

fn dialog() -> UpdateDialog {
    UpdateDialog::new("0.5.0", "0.4.0")
}

#[test]
fn the_update_dialog_says_what_is_available_and_what_you_have_at_both_scales() {
    for scale in SCALES {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = dialog().layout(window, scale);
        let said = texts(&dialog().draw(window, [0.0, 0.0], scale));
        assert!(
            said.join(" ")
                .contains("ScopeCreep 0.5.0 is available (you have 0.4.0)"),
            "{said:?}"
        );
        for part in ["What's new", "Update", "Skip this version", "Later"] {
            assert!(said.contains(&part.to_string()), "{part}: {said:?}");
        }
        for p in dialog().draw(window, [0.0, 0.0], scale) {
            if let Prim::Text { rect, .. } = p {
                assert!(inside(l.area, rect), "text {rect:?} outside {:?}", l.area);
            }
        }
    }
}

#[test]
fn the_update_dialog_is_centred_scales_and_fits_the_least_window() {
    let (one, two) = (
        dialog().layout(WINDOW, 1.0),
        dialog().layout([2.0 * WINDOW[0], 2.0 * WINDOW[1]], 2.0),
    );
    for (l, window) in [(&one, WINDOW), (&two, [2.0 * WINDOW[0], 2.0 * WINDOW[1]])] {
        let a = l.area;
        assert!((a[0] + a[2] / 2.0 - window[0] / 2.0).abs() < 0.5, "across");
        assert!((a[1] + a[3] / 2.0 - window[1] / 2.0).abs() < 0.5, "down");
        assert!(inside([0.0, 0.0, window[0], window[1]], a));
    }
    assert!((two.area[2] - 2.0 * one.area[2]).abs() < 0.01);
    assert!((two.area[3] - 2.0 * one.area[3]).abs() < 0.01);
    // Long version numbers still fit.
    let long = UpdateDialog::new("10.20.30", "9.99.99").layout(WINDOW, 1.0);
    assert!(inside([0.0, 0.0, WINDOW[0], WINDOW[1]], long.area));
    for r in [long.whats_new, long.update, long.skip, long.later] {
        assert!(inside(long.area, r));
    }
}

#[test]
fn the_update_dialog_buttons_and_link_have_real_hit_areas() {
    for scale in SCALES {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = dialog().layout(window, scale);
        for (rect, want) in [
            (l.whats_new, UpdateHit::WhatsNew),
            (l.update, UpdateHit::Update),
            (l.skip, UpdateHit::Skip),
            (l.later, UpdateHit::Later),
        ] {
            assert!(inside(l.area, rect));
            assert!(
                rect[3] >= style::SPACE_CHIP_HEIGHT * scale - 0.01,
                "tall enough to hit"
            );
            assert_eq!(dialog().hit(window, centre_of(rect), scale), want);
        }
        for pair in [(l.update, l.skip), (l.skip, l.later)] {
            assert!(pair.0[0] + pair.0[2] <= pair.1[0], "no overlap");
        }
    }
}

#[test]
fn a_click_outside_the_update_dialog_is_later_and_one_on_it_stays() {
    for scale in SCALES {
        let window = [WINDOW[0] * scale, WINDOW[1] * scale];
        let l = dialog().layout(window, scale);
        assert_eq!(dialog().hit(window, [1.0, 1.0], scale), UpdateHit::Later);
        let corner = [l.area[0] + 1.0, l.area[1] + l.area[3] - 1.0];
        assert_eq!(dialog().hit(window, corner, scale), UpdateHit::Inside);
    }
}

#[test]
fn the_update_dialog_takes_every_key_and_only_escape_means_later() {
    assert!(dialog().key(true));
    assert!(!dialog().key(false));
}

#[test]
fn the_update_dialog_lights_the_button_under_the_pointer() {
    let l = dialog().layout(WINDOW, 1.0);
    let rest = dialog().draw(WINDOW, [0.0, 0.0], 1.0);
    for r in [l.whats_new, l.update, l.skip, l.later] {
        assert_ne!(rest, dialog().draw(WINDOW, centre_of(r), 1.0));
    }
}
