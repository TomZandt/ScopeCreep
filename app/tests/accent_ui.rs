//! The brand accent and the Hairline look at the pure UI seam (spec #196, #226, ADR 0044), at 1x and 2x: the accent's
//! derivation and contrast, the accent setting's round trip and bad input (in temp folders, never the user's own
//! files), the Appearance panel, the chip states (accent only as text and a thin line, hover neutral), the title bar's
//! zones (the view block on picture Panels only, the rule only with chips, "+N"), and the Panel's least width.
use scopecreep::accent::{self, Accent};
use scopecreep::appearance_panel::{AppearanceAction, AppearancePanel};
use scopecreep::components::{
    Chip, DropMenu, MenuEntry, Prim, STATES, State, TitleBar, TitleLayout, dock_overlay,
    focus_line, hovered_row,
};
use scopecreep::menu_bar::{self, MenuAction};
use scopecreep::style;
use scopecreep::tab_strip::TabStrip;
use scopecreep::toolbar::{ChipAction, OVERFLOW_WIDEST, Toolbar};

const SCALES: [f32; 2] = [1.0, 2.0];

fn fills(prims: &[Prim]) -> Vec<[f32; 4]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Rect { colour, .. } => Some(*colour),
            _ => None,
        })
        .collect()
}

fn text_colours(prims: &[Prim]) -> Vec<[u8; 3]> {
    prims
        .iter()
        .filter_map(|p| match p {
            Prim::Text { colour, .. } => Some(*colour),
            _ => None,
        })
        .collect()
}

/// A folder of its own under the system's temp folder, so a test never touches the user's real settings.
fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("scopecreep-accent-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- Derivation and contrast ----

#[test]
fn the_default_accent_is_the_lime_and_hex_reads_and_writes_the_same_way() {
    assert_eq!(accent::DEFAULT_HEX, "#d2ff00");
    assert_eq!(Accent::default().hex(), "#d2ff00");
    assert_eq!(Accent::default(), Accent(accent::DEFAULT));
    for text in ["#d2ff00", "d2ff00", "  #D2FF00 ", "#D2ff00"] {
        assert_eq!(
            accent::parse_hex(text),
            Some([0xd2, 0xff, 0x00]),
            "{text:?}"
        );
    }
    for rgb in [[0, 0, 0], [255, 255, 255], [1, 2, 3], [0x12, 0xab, 0xef]] {
        assert_eq!(accent::parse_hex(&Accent(rgb).hex()), Some(rgb));
    }
}

#[test]
fn a_bad_hex_is_refused() {
    for text in [
        "",
        "#",
        "#fff",
        "d2ff0",
        "#d2ff000",
        "#gggggg",
        "#d2 ff00",
        "red",
        "#d2ff0z",
        "0xd2ff00",
        "#d2ff00;",
        "ＦＦ00ff",
        "#-2ff00",
    ] {
        assert_eq!(accent::parse_hex(text), None, "{text:?}");
    }
}

#[test]
fn the_pressed_accent_is_a_darker_derivation_and_there_is_no_hover_accent() {
    for rgb in [accent::DEFAULT, [0xff, 0xb0, 0x00], [0x2f, 0xd8, 0xff]] {
        let a = Accent(rgb);
        let (full, pressed) = (a.rgb(), a.pressed());
        for k in 0..3 {
            assert!(pressed[k] <= full[k]);
        }
        assert!(accent::luminance(pressed) < accent::luminance(full));
    }
    // Text on an accent fill is the more legible of near black and near white.
    assert_eq!(Accent(accent::DEFAULT).on(), style::TEXT_ON_ACCENT);
    assert_eq!(Accent([0x10, 0x20, 0x90]).on(), style::TEXT_HIGH);
}

#[test]
fn contrast_is_the_wcag_ratio_and_the_panel_says_when_it_is_under_4_5() {
    // The extremes of the scale.
    assert!((accent::contrast([0.0; 3], [1.0; 3]) - 21.0).abs() < 0.01);
    assert!((accent::contrast([0.5; 3], [0.5; 3]) - 1.0).abs() < 1e-4);
    // The lime reads well on a chip; a dark blue does not.
    assert!(!Accent::default().low_contrast());
    assert!(Accent::default().contrast() > accent::MIN_CONTRAST);
    let dark = Accent([0x20, 0x30, 0x80]);
    assert!(dark.low_contrast());
    assert!(
        scopecreep::appearance_panel::contrast_notice(dark).contains("under 4.5:1"),
        "{}",
        scopecreep::appearance_panel::contrast_notice(dark)
    );
    assert!(
        !scopecreep::appearance_panel::contrast_notice(Accent::default()).contains("under"),
        "no warning at the default"
    );
    // Every swatch is a colour the Window offers as it is.
    for (name, hex) in accent::SWATCHES {
        assert!(accent::parse_hex(hex).is_some(), "{name}");
    }
    assert_eq!(accent::SWATCHES[0].1, accent::DEFAULT_HEX);
}

#[test]
fn the_chrome_greys_stay_neutral_and_the_muted_text_reads_at_about_5_to_1() {
    for c in [
        style::SURFACE_PANEL,
        style::SURFACE_CONTROL,
        style::SURFACE_CONTROL_ON,
        style::SURFACE_CONTROL_PRESSED,
        style::STATE_HOVER,
        style::STATE_ON_HOVER,
        style::LINE_RULE,
        style::SURFACE_DOCK_WASH,
    ] {
        assert!(c[0] == c[1] && c[1] == c[2], "{c:?} tints the eye");
    }
    // The rule is #5c5c5c.
    assert_eq!(style::LINE_RULE[0], 0.36);
    let muted = style::rgb(style::TEXT_MUTED);
    assert!(accent::contrast(muted, style::SURFACE_PANEL) >= accent::MIN_CONTRAST);
    let against_window = accent::contrast(muted, style::SURFACE_WINDOW);
    assert!(
        (4.8..=6.5).contains(&against_window),
        "about 5:1: {against_window}"
    );
}

// ---- The setting file ----

#[test]
fn the_accent_is_remembered_in_its_own_file_and_comes_back() {
    let dir = temp_dir("roundtrip");
    let path = dir.join("scopecreep").join("accent");
    assert_eq!(accent::load(&path), Accent::default(), "no file yet");
    for rgb in [[0xff, 0x7a, 0x1a], [0, 0, 0], accent::DEFAULT] {
        accent::save(&path, Accent(rgb)).unwrap();
        assert_eq!(accent::load(&path), Accent(rgb));
    }
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("scopecreep-accent 1\n")
    );
    // Written whole: no temporary file is left beside it.
    let left: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(left, [std::ffi::OsString::from("accent")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bad_accent_file_is_the_default() {
    let dir = temp_dir("bad");
    let path = dir.join("accent");
    let cases: [(&str, String); 9] = [
        ("empty", String::new()),
        ("no header", "accent #ff0000\n".to_string()),
        (
            "other version",
            "scopecreep-accent 2\naccent #ff0000\n".to_string(),
        ),
        ("bad hex", "scopecreep-accent 1\naccent #ff00\n".to_string()),
        ("not a hex", "scopecreep-accent 1\naccent red\n".to_string()),
        (
            "extra words",
            "scopecreep-accent 1\naccent #ff0000 more\n".to_string(),
        ),
        (
            "wrong key",
            "scopecreep-accent 1\ncolour #ff0000\n".to_string(),
        ),
        ("binary", "\u{0}\u{1}\u{2}".to_string()),
        (
            "far too long",
            format!(
                "scopecreep-accent 1\naccent #ff0000\n{}",
                "x".repeat(10_000)
            ),
        ),
    ];
    for (what, text) in cases {
        std::fs::write(&path, &text).unwrap();
        assert_eq!(accent::load(&path), Accent::default(), "{what}");
        if what != "far too long" {
            assert_eq!(accent::from_text(&text), Accent::default(), "{what}");
        }
    }
    // Not valid text at all, and a folder where the file should be.
    std::fs::write(&path, [0xff, 0xfe, 0x00, 0xd8]).unwrap();
    assert_eq!(accent::load(&path), Accent::default());
    assert_eq!(accent::load(&dir), Accent::default());
    // A good value after a bad line is still found; the first good one wins.
    assert_eq!(
        accent::from_text("scopecreep-accent 1\nnoise\naccent #00ff00\naccent #ff0000\n"),
        Accent([0, 255, 0])
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_current_accent_is_what_the_chrome_draws_with_and_one_test_cannot_disturb_another() {
    assert_eq!(accent::current(), Accent::default());
    accent::set_current(Accent([0xff, 0x7a, 0x1a]));
    assert_eq!(style::accent(), style::rgb([0xff, 0x7a, 0x1a]));
    assert_eq!(style::accent_text(), [0xff, 0x7a, 0x1a]);
    let on = Chip::new("Y").draw([0.0; 2], true, State::Normal, 1.0);
    assert!(fills(&on).contains(&style::rgba(style::rgb([0xff, 0x7a, 0x1a]))));
    // Another thread has its own, the default.
    assert_eq!(
        std::thread::spawn(accent::current).join().unwrap(),
        Accent::default()
    );
    accent::set_current(Accent::default());
}

// ---- The Appearance panel ----

fn rows(menu: &menu_bar::Menu) -> Vec<String> {
    menu.drop
        .entries
        .iter()
        .map(|e| match e {
            MenuEntry::Item(i) => format!("{}|{}", i.label, i.hotkey),
            MenuEntry::Heading(h) => format!("# {h}"),
            MenuEntry::Separator => "-".to_string(),
            MenuEntry::Swatches(s) => format!("swatches {}", s.len()),
        })
        .collect()
}

#[test]
fn settings_opens_appearance_and_the_panel_lists_swatches_hex_notice_and_reset() {
    let settings = menu_bar::settings_menu();
    assert_eq!(settings.action(0), Some(MenuAction::OpenAppearance));
    assert_eq!(settings.action(1), Some(MenuAction::OpenHotkeys));
    let menu = AppearancePanel::default().menu(Accent::default());
    let rows = rows(&menu);
    assert_eq!(rows[0], "# App accent colour");
    assert_eq!(rows[1], "swatches 6");
    assert_eq!(rows[2], "Hex|#d2ff00");
    assert!(rows[3].starts_with("# Contrast "), "{rows:?}");
    assert!(rows[5].starts_with("Reset to default"), "{rows:?}");
    // Reset has nothing to do at the default.
    let MenuEntry::Item(reset) = &menu.drop.entries[5] else {
        panic!("a reset item");
    };
    assert!(!reset.enabled);
    // Off the default it can be used, and the chosen swatch is marked.
    let other = AppearancePanel::default().menu(Accent([0xff, 0xb0, 0x00]));
    let MenuEntry::Item(reset) = &other.drop.entries[5] else {
        panic!("a reset item");
    };
    assert!(reset.enabled);
    assert_eq!(
        other.action(5),
        Some(MenuAction::Appearance(AppearanceAction::Reset))
    );
    let MenuEntry::Swatches(s) = &other.drop.entries[1] else {
        panic!("swatches");
    };
    assert_eq!(s.iter().filter(|(_, chosen)| *chosen).count(), 1);
    assert!(s[1].1, "Amber is the chosen one");
}

#[test]
fn a_swatch_click_takes_that_accent_and_the_swatches_have_hit_areas_at_both_scales() {
    for scale in SCALES {
        let menu = AppearancePanel::default().menu(Accent::default());
        let area = menu.drop.rect(
            [0.0, 0.0, 60.0 * scale, 32.0 * scale],
            [4000.0, 4000.0],
            scale,
        );
        let row = menu.drop.entry_rects(area, scale)[1];
        let rects = scopecreep::components::swatch_rects(row, accent::SWATCHES.len(), scale);
        assert_eq!(rects.len(), 6);
        for (n, r) in rects.iter().enumerate() {
            let centre = [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
            assert_eq!(menu.drop.swatch_at(area, centre, scale), Some((1, n)));
            assert!(r[0] + r[2] <= area[0] + area[2], "inside the menu");
            if n > 0 {
                assert!(r[0] >= rects[n - 1][0] + rects[n - 1][2], "no overlap");
            }
        }
        assert_eq!(
            menu.drop
                .swatch_at(area, [area[0] + 1.0, area[1] + 1.0], scale),
            None
        );
        // A swatch is not an item.
        assert_eq!(
            menu.drop
                .hit(area, [rects[0][0] + 1.0, rects[0][1] + 1.0], scale),
            None
        );
    }
    let mut panel = AppearancePanel::default();
    let mut now = Accent::default();
    assert!(panel.click(AppearanceAction::Swatch(1), &mut now));
    assert_eq!(now.hex(), accent::SWATCHES[1].1);
    assert!(
        !panel.click(AppearanceAction::Swatch(1), &mut now),
        "no change"
    );
    assert!(
        !panel.click(AppearanceAction::Swatch(99), &mut now),
        "no such swatch"
    );
    assert_eq!(now.hex(), accent::SWATCHES[1].1);
}

#[test]
fn the_hex_box_types_like_a_value_box_enter_accepts_escape_cancels_and_a_bad_hex_is_refused() {
    let mut panel = AppearancePanel::default();
    let mut now = Accent::default();
    assert!(!panel.takes_keys());
    // A click starts typing from the current value.
    assert!(!panel.click(AppearanceAction::EditHex, &mut now));
    assert!(panel.takes_keys());
    assert_eq!(panel.editing.as_deref(), Some("#d2ff00"));
    let shown = panel.menu(now);
    assert_eq!(rows(&shown)[2], "Hex|#d2ff00|", "a caret while typing");
    // Typing: Backspace, hex digits only, one leading '#', no more than seven characters.
    for _ in 0..7 {
        panel.backspace();
    }
    panel.type_text("#FF 7A\n1Zg1Aff");
    assert_eq!(panel.editing.as_deref(), Some("#ff7a11"));
    // Enter accepts.
    assert!(panel.accept(&mut now));
    assert_eq!(now.hex(), "#ff7a11");
    assert!(!panel.takes_keys());
    // A bad hex is refused and the accent stays.
    panel.click(AppearanceAction::EditHex, &mut now);
    for _ in 0..7 {
        panel.backspace();
    }
    panel.type_text("#12");
    assert!(!panel.accept(&mut now));
    assert_eq!(now.hex(), "#ff7a11");
    // Escape cancels.
    panel.click(AppearanceAction::EditHex, &mut now);
    panel.type_text("0");
    panel.cancel();
    assert!(!panel.takes_keys());
    assert_eq!(now.hex(), "#ff7a11");
    // A second click on the box while typing accepts what is typed.
    panel.click(AppearanceAction::EditHex, &mut now);
    for _ in 0..7 {
        panel.backspace();
    }
    panel.type_text("00ff00");
    assert!(panel.click(AppearanceAction::EditHex, &mut now));
    assert_eq!(now.hex(), "#00ff00");
    // Reset goes back to the default and stops any typing.
    panel.click(AppearanceAction::EditHex, &mut now);
    assert!(panel.click(AppearanceAction::Reset, &mut now));
    assert_eq!(now, Accent::default());
    assert!(!panel.takes_keys());
    // Typing into a panel that is not editing does nothing.
    panel.type_text("abc");
    panel.backspace();
    assert_eq!(panel.editing, None);
}

#[test]
fn the_appearance_panel_is_part_of_the_windows_least_size() {
    let menus = menu_bar::largest_menus();
    let appearance = AppearancePanel::default().menu(Accent::default()).drop;
    let size = appearance.size(1.0);
    assert!(
        menus.iter().any(|m| m.size(1.0)[1] >= size[1]),
        "a menu at least as tall is counted"
    );
    assert!(
        menus.contains(&DropMenu {
            entries: AppearancePanel {
                editing: Some("#ffffff".to_string()),
            }
            .menu(Accent::default())
            .drop
            .entries,
        })
    );
}

// ---- Chip states ----

#[test]
fn an_on_chip_is_accent_text_and_a_thin_accent_line_on_a_lighter_grey() {
    for scale in SCALES {
        let chip = Chip::new("B/W");
        let prims = chip.draw([10.0, 10.0], true, State::Normal, scale);
        let r = chip.rect([10.0, 10.0], scale);
        assert_eq!(fills(&prims)[0], style::rgba(style::SURFACE_CONTROL_ON));
        assert!(
            prims.iter().any(|p| matches!(p, Prim::Rect { rect, colour }
                if *colour == style::rgba(style::accent())
                    && rect[3] == style::SPACE_ACCENT_LINE * scale
                    && rect[1] + rect[3] == r[1] + r[3] && rect[2] == r[2])),
            "a 2 px line along the foot"
        );
        assert_eq!(text_colours(&prims), [style::accent_text()]);
        // Off: a plain grey chip with grey text and no accent anywhere.
        let off = chip.draw([10.0, 10.0], false, State::Normal, scale);
        assert_eq!(fills(&off), [style::rgba(style::SURFACE_CONTROL)]);
        assert_eq!(text_colours(&off), [style::TEXT_SECONDARY]);
    }
}

#[test]
fn hover_is_a_neutral_lift_never_the_accent_and_pressed_is_darker() {
    let chip = Chip::new("Mono");
    let accent_rgba = style::rgba(style::accent());
    for scale in SCALES {
        let hover = chip.draw([0.0; 2], false, State::Hover, scale);
        assert_eq!(fills(&hover), [style::rgba(style::STATE_HOVER)]);
        assert_eq!(text_colours(&hover), [style::TEXT_HIGH], "white text");
        assert!(
            !fills(&hover).contains(&accent_rgba) && text_colours(&hover) != [style::accent_text()]
        );
        let normal = style::SURFACE_CONTROL[0];
        let pressed = chip.draw([0.0; 2], false, State::Pressed, scale);
        assert_eq!(
            fills(&pressed),
            [style::rgba(style::SURFACE_CONTROL_PRESSED)]
        );
        assert!(style::SURFACE_CONTROL_PRESSED[0] < normal && normal < style::STATE_HOVER[0]);
        // On and hovered: a lighter grey, still no new accent fill.
        let on_hover = chip.draw([0.0; 2], true, State::Hover, scale);
        assert_eq!(fills(&on_hover)[0], style::rgba(style::STATE_ON_HOVER));
        assert!(
            fills(&on_hover)[0][0] > style::SURFACE_CONTROL_ON[0],
            "lighter than at rest"
        );
        let on_pressed = chip.draw([0.0; 2], true, State::Pressed, scale);
        assert_eq!(
            fills(&on_pressed)[0],
            style::rgba(style::SURFACE_CONTROL_PRESSED)
        );
        // Disabled: grey text, no accent.
        let off = chip.draw([0.0; 2], true, State::Disabled, scale);
        assert!(!fills(&off).contains(&accent_rgba));
        assert_eq!(text_colours(&off), [style::TEXT_DISABLED]);
    }
    // Every state of every on or off chip is made of neutral greys except the accent line and text.
    for on in [false, true] {
        for state in STATES {
            for p in chip.draw([0.0; 2], on, state, 1.0) {
                if let Prim::Rect { colour, .. } = p
                    && colour != accent_rgba
                {
                    assert!(colour[0] == colour[1] && colour[1] == colour[2]);
                }
            }
        }
    }
}

#[test]
fn the_channel_chips_keep_their_colours_when_on() {
    for (channel, colour) in [
        (scopecreep::components::Channel::R, style::CHANNEL_R),
        (scopecreep::components::Channel::G, style::CHANNEL_G),
        (scopecreep::components::Channel::B, style::CHANNEL_B),
    ] {
        let prims = Chip::channel("x", channel).draw([0.0; 2], true, State::Normal, 1.0);
        assert_eq!(
            fills(&prims),
            [style::rgba(colour)],
            "no accent line on a channel"
        );
    }
}

// ---- Other accent states ----

#[test]
fn focus_is_a_2px_accent_line_along_the_title_bar_and_nothing_round_the_picture() {
    for scale in SCALES {
        let title = [
            101.0 * scale,
            51.0 * scale,
            398.0 * scale,
            style::SPACE_TITLE_BAR * scale,
        ];
        let Prim::Rect { rect, colour } = focus_line(title, scale) else {
            panic!("a rectangle");
        };
        assert_eq!(colour, style::rgba(style::accent()));
        assert_eq!(rect, [title[0], title[1], title[2], 2.0 * scale]);
        // The title bar draws it only when focused.
        let bar = TitleBar {
            name: "Waveform".into(),
            tags: String::new(),
            chips: vec![],
            groups: vec![],
            settings_button: true,
            view: false,
        };
        let plain = bar.draw_focused(title, &[], State::Normal, false, scale);
        let focused = bar.draw_focused(title, &[], State::Normal, true, scale);
        assert!(!fills(&plain).contains(&style::rgba(style::accent())));
        assert!(fills(&focused).contains(&style::rgba(style::accent())));
        assert_eq!(bar.draw(title, &[], State::Normal, scale), plain);
    }
}

#[test]
fn the_dock_overlay_is_a_dashed_accent_outline_over_a_faint_grey_wash() {
    for scale in SCALES {
        let target = [100.0 * scale, 50.0 * scale, 300.0 * scale, 200.0 * scale];
        let prims = dock_overlay(target, scale);
        let Prim::Rect { rect, colour } = &prims[0] else {
            panic!("the wash first");
        };
        assert_eq!(*rect, target);
        assert!(colour[3] < 0.3, "faint");
        assert!(colour[0] == colour[1] && colour[1] == colour[2]);
        let dashes: Vec<[f32; 4]> = prims[1..]
            .iter()
            .filter_map(|p| match p {
                Prim::Rect { rect, colour } if *colour == style::rgba(style::accent()) => {
                    Some(*rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(dashes.len(), prims.len() - 1, "the rest is accent");
        assert!(dashes.len() > 8, "dashed, not solid");
        for d in &dashes {
            assert!(d[0] >= target[0] - 0.01 && d[1] >= target[1] - 0.01);
            assert!(d[0] + d[2] <= target[0] + target[2] + 0.01);
            assert!(d[1] + d[3] <= target[1] + target[3] + 0.01);
            assert!(d[2].min(d[3]) >= style::SPACE_ACCENT_LINE * scale - 0.01);
        }
        // Not a closed frame: along the top there are gaps.
        let top: Vec<_> = dashes
            .iter()
            .filter(|d| d[1] == target[1] && d[3] <= 3.0 * scale)
            .collect();
        assert!(top.windows(2).all(|w| w[1][0] > w[0][0] + w[0][2]));
    }
}

#[test]
fn a_hovered_menu_row_has_a_neutral_lift_and_a_2px_accent_bar_at_its_left() {
    for scale in SCALES {
        let row = [
            10.0 * scale,
            20.0 * scale,
            200.0 * scale,
            style::SPACE_MENU_ROW * scale,
        ];
        let [lift, bar] = hovered_row(row, style::STATE_HOVER, scale);
        assert_eq!(
            lift,
            Prim::Rect {
                rect: row,
                colour: style::rgba(style::STATE_HOVER)
            }
        );
        assert_eq!(
            bar,
            Prim::Rect {
                rect: [row[0], row[1], 2.0 * scale, row[3]],
                colour: style::rgba(style::accent())
            }
        );
    }
    // A drop menu draws it for the row under the pointer, and the chosen row's mark is the accent.
    let mut on = scopecreep::components::MenuItem::new("A", "");
    on.on = true;
    let menu = DropMenu {
        entries: vec![
            MenuEntry::Item(on),
            MenuEntry::Item(scopecreep::components::MenuItem::new("B", "")),
        ],
    };
    let area = menu.rect([0.0; 4], [1000.0, 1000.0], 1.0);
    let quiet = fills(&menu.draw(area, None, false, 1.0));
    let hovered = fills(&menu.draw(area, Some(1), false, 1.0));
    let accents = |f: &[[f32; 4]]| {
        f.iter()
            .filter(|c| **c == style::rgba(style::accent()))
            .count()
    };
    assert_eq!(accents(&quiet), 1, "the chosen row's small square");
    assert_eq!(accents(&hovered), 2, "and the hovered row's bar");
}

#[test]
fn the_page_tab_and_a_tab_group_tab_are_accent_text_with_an_underline() {
    for scale in SCALES {
        let strip = TabStrip {
            labels: vec!["Waveform".into(), "Vectorscope".into()],
            active: 0,
        };
        let area = [0.0, 0.0, 400.0 * scale, style::SPACE_TITLE_BAR * scale];
        let prims = strip.draw(area, None, scale);
        assert!(text_colours(&prims).contains(&style::accent_text()));
        assert!(fills(&prims).contains(&style::rgba(style::accent())));
        // Hover on the other tab is the neutral lift and white text.
        let hovered = strip.draw(area, Some(scopecreep::tab_strip::TabHit::Tab(1)), scale);
        assert!(text_colours(&hovered).contains(&style::TEXT_HIGH));
    }
    // The page bar's tab is a chip: on is accent.
    let bar = scopecreep::page_bar::PageBar {
        names: vec!["Main".into(), "Two".into()],
        current: 0,
        rename: None,
    };
    let prims = bar.draw(0.0, 1000.0, None, 1.0);
    assert!(text_colours(&prims).contains(&style::accent_text()));
}

#[test]
fn the_slider_is_a_thin_accent_track_with_a_grey_thumb() {
    for scale in SCALES {
        let slider = scopecreep::components::Slider {
            value: 0.5,
            number: "50".into(),
        };
        let l = slider.layout([0.0; 2], scale);
        assert_eq!(l.track[3], 2.0 * scale);
        assert_eq!(l.thumb[2], style::SPACE_SLIDER_THUMB * scale);
        assert_eq!(l.thumb[3], style::SPACE_SLIDER_THUMB_HEIGHT * scale);
        let prims = slider.draw([0.0; 2], State::Normal, scale);
        let f = fills(&prims);
        assert_eq!(f[1], style::rgba(style::accent()), "the filled track");
        assert!(
            f[2][0] == f[2][1] && f[2][1] == f[2][2],
            "a grey thumb: {:?}",
            f[2]
        );
    }
}

// ---- The title bar ----

fn bar(chips: Vec<Chip>, groups: Vec<usize>, view: bool, width: f32, scale: f32) -> TitleLayout {
    TitleBar {
        name: "Vectorscope".into(),
        tags: "100 %".into(),
        chips,
        groups,
        settings_button: true,
        view,
    }
    .layout(
        [0.0, 0.0, width * scale, style::SPACE_TITLE_BAR * scale],
        scale,
    )
}

fn some_chips() -> (Vec<Chip>, Vec<usize>) {
    (
        ["100 %", "200 %", "Both", "Mono"].map(Chip::new).to_vec(),
        vec![0, 0, 0, 1],
    )
}

#[test]
fn the_view_block_is_only_on_a_picture_panel_and_the_settings_button_always() {
    for scale in SCALES {
        let (chips, groups) = some_chips();
        let picture = bar(chips.clone(), groups.clone(), true, 700.0, scale);
        let scope = bar(chips, groups, false, 700.0, scale);
        assert!(picture.view.is_some() && picture.divider.is_some());
        assert!(scope.view.is_none(), "a scope has no view block");
        assert!(scope.divider.is_none(), "and so no rule after its chips");
        assert!(picture.settings.is_some() && scope.settings.is_some());
        // The scope's chips end beside the settings button, a settings gap from it.
        let last = scope.chips.last().unwrap();
        let settings = scope.settings.unwrap();
        assert!(
            (settings[0] - (last[0] + last[2]) - style::SPACE_SETTINGS_GAP * scale).abs() < 0.01
        );
    }
}

#[test]
fn the_rule_is_2_wide_grey_20_of_air_each_side_and_only_where_the_panel_has_chips() {
    for scale in SCALES {
        let (chips, groups) = some_chips();
        let with = bar(chips, groups, true, 700.0, scale);
        let rule = with.divider.expect("chips and a block");
        let view = with.view.unwrap();
        let last = with.chips.last().unwrap();
        assert_eq!(rule[2], 2.0 * scale);
        assert!(
            (rule[0] - (last[0] + last[2]) - 20.0 * scale).abs() < 0.01,
            "20 of air before"
        );
        assert!(
            (view.fit[0] - (rule[0] + rule[2]) - 20.0 * scale).abs() < 0.01,
            "and after"
        );
        assert_eq!(
            rule[3],
            style::SPACE_CHIP_HEIGHT * scale,
            "as tall as a chip"
        );
        let prims = TitleBar {
            name: "V".into(),
            tags: String::new(),
            chips: vec![Chip::new("a")],
            groups: vec![0],
            settings_button: true,
            view: true,
        }
        .draw(
            [0.0, 0.0, 700.0 * scale, style::SPACE_TITLE_BAR * scale],
            &[(false, State::Normal)],
            State::Normal,
            scale,
        );
        assert!(
            fills(&prims).contains(&style::rgba(style::LINE_RULE)),
            "grey #5c5c5c, not the accent"
        );
        // No chips: no rule, and the block follows the name.
        let none = bar(vec![], vec![], true, 700.0, scale);
        assert!(none.divider.is_none());
        assert!(none.view.is_some());
    }
}

#[test]
fn the_title_bar_zones_run_name_tag_chips_rule_view_block_then_the_settings_button() {
    for scale in SCALES {
        let (chips, groups) = some_chips();
        let l = bar(chips, groups, true, 700.0, scale);
        let tags = l.tags.expect("room for the tag");
        let first = l.chips[0];
        let view = l.view.unwrap();
        let settings = l.settings.unwrap();
        assert!(l.name[0] + l.name[2] <= tags[0]);
        assert!(tags[0] + tags[2] <= first[0] - style::SPACE_NAME_GAP * scale + 0.01);
        assert!(view.fit[0] + view.fit[2] <= view.readout[0]);
        assert!(view.readout[0] + view.readout[2] <= view.link[0]);
        // The gaps inside the block (4), inside a chip group (2) and between groups (8).
        assert!((view.readout[0] - (view.fit[0] + view.fit[2]) - 4.0 * scale).abs() < 0.01);
        assert!((l.chips[1][0] - (l.chips[0][0] + l.chips[0][2]) - 2.0 * scale).abs() < 0.01);
        assert!((l.chips[3][0] - (l.chips[2][0] + l.chips[2][2]) - 8.0 * scale).abs() < 0.01);
        // The settings gap (8) and the settings button's size (28 wide, a chip high), then the bar's padding (12).
        assert!((settings[0] - (view.link[0] + view.link[2]) - 8.0 * scale).abs() < 0.01);
        assert_eq!([settings[2], settings[3]], [28.0 * scale, 24.0 * scale]);
        let right = l.bar[0] + l.bar[2];
        assert!((right - (settings[0] + settings[2]) - 12.0 * scale).abs() < 0.01);
        assert_eq!(l.bar[3], 34.0 * scale);
        // Everything is centred in the bar's height.
        for r in l.chips.iter().chain([&view.fit, &view.link, &settings]) {
            assert!((r[1] + r[3] / 2.0 - (l.bar[1] + l.bar[3] / 2.0)).abs() < 0.01);
        }
    }
}

#[test]
fn the_panels_least_width_is_the_title_bars_least_width_rounded_up_and_the_bar_holds_at_it() {
    assert_eq!(
        style::SPACE_MIN_PANEL_WIDTH,
        TitleBar::least_width(1.0).ceil(),
        "the token is what the tokens add up to"
    );
    assert_eq!(style::SPACE_MIN_PANEL_WIDTH, 255.0);
    assert_eq!(style::SPACE_MIN_PANEL_HEIGHT, 117.0);
    for scale in SCALES {
        let width = style::SPACE_MIN_PANEL_WIDTH;
        // A bar the least width wide, with the block and the settings button: nothing leaves it and the name keeps
        // room for a few characters.
        let l = bar(vec![], vec![], true, width, scale);
        let bar_right = l.bar[0] + l.bar[2];
        let settings = l.settings.unwrap();
        let view = l.view.unwrap();
        assert!(settings[0] + settings[2] <= bar_right - 12.0 * scale + 0.01);
        assert!(view.fit[0] >= l.bar[0] + 12.0 * scale);
        assert!(
            l.name[2] >= style::SPACE_NAME_MIN * scale - 1.0,
            "the name keeps its room: {:?}",
            l.name
        );
        assert!(view.fit[0] - (l.name[0] + l.name[2]) >= 0.0);
        // The least width is a minimum of the tokens, not a margin of safety: a hair narrower, the name is under 44.
        let tighter = bar(
            vec![],
            vec![],
            true,
            TitleBar::least_width(1.0) - 4.0,
            scale,
        );
        assert!(tighter.name[2] < style::SPACE_NAME_MIN * scale + 4.0 * scale);
    }
    // The "+N" chip shows where it fits beside the rule, the block and the least name: a little wider.
    assert!(TitleBar::overflow_width(1.0) > TitleBar::least_width(1.0));
    assert!(
        (TitleBar::overflow_width(1.0)
            - TitleBar::least_width(1.0)
            - TitleBar::rule_room(1.0)
            - Chip::new(OVERFLOW_WIDEST).size(1.0)[0]
            - style::SPACE_CHIP_GROUP_GAP)
            .abs()
            < 0.01
    );
}

fn vectorscope_toolbar() -> Toolbar {
    // Three groups, as a scope's chips (plots, Mono, tonal range): 100 % 200 % Both | Mono | All Shadows Mids.
    let texts = ["100 %", "200 %", "Both", "Mono", "All", "Shadows", "Mids"];
    let groups = [0, 0, 0, 1, 2, 2, 2];
    let keys = ["plots", "plots", "plots", "mono", "tones", "tones", "tones"];
    let mut t = Toolbar::default();
    for (i, text) in texts.iter().enumerate() {
        t.chips.push(Chip::new(text));
        t.groups.push(groups[i]);
        t.on.push(false);
        t.tooltips.push(text.to_string());
        t.picks.push(ChipAction::Set(keys[i], text.to_string()));
    }
    t
}

#[test]
fn when_narrow_the_chips_give_way_a_setting_at_a_time_from_the_right_into_a_plus_n_chip() {
    for scale in SCALES {
        let reserve = TitleBar::view_reserve(scale);
        let name = 100.0 * scale;
        let full = {
            let t = vectorscope_toolbar();
            scopecreep::components::chip_row_width_grouped(&t.chips, &t.groups, scale)
        };
        let settings = TitleBar::settings_room(scale);
        let pad = 24.0 * scale;
        let mut seen = Vec::new();
        // Walk the bar from wide to narrow: each step hides whole settings, never the middle of one, and the "+N"
        // chip always counts what is hidden.
        for width in (200..=900).rev().step_by(10) {
            let mut t = vectorscope_toolbar();
            t.fit_to_reserving_floor(
                width as f32 * scale,
                true,
                reserve + name,
                reserve + 56.0 * scale,
                scale,
            );
            let shown: Vec<&str> = t.chips.iter().map(|c| c.text.as_str()).collect();
            if t.hidden == 0 {
                assert_eq!(shown.len(), 7, "{width}");
                assert!(!shown.iter().any(|s| s.starts_with('+')));
            } else if let Some(last) = shown.last().filter(|s| s.starts_with('+')) {
                assert_eq!(*last, format!("+{}", t.hidden));
                let kept = shown.len() - 1;
                assert_eq!(
                    kept + t.hidden,
                    7,
                    "the +N counts every hidden chip at {width}"
                );
                assert!(
                    matches!(kept, 0 | 3 | 4),
                    "whole settings only: {shown:?} at {width}"
                );
                assert_eq!(t.action(kept), Some(&ChipAction::More));
                assert!(!t.collapsed.is_empty());
                // What is left fits the room beside the name, the block and the settings button.
                let used =
                    scopecreep::components::chip_row_width_grouped(&t.chips, &t.groups, scale);
                assert!(
                    used + settings + reserve + name + pad <= width as f32 * scale + 0.01
                        || kept == 0,
                    "{shown:?} fits at {width}"
                );
            } else {
                assert!(
                    shown.is_empty(),
                    "no room even for +N at {width}: {shown:?}"
                );
            }
            seen.push(t.hidden);
        }
        // Wider: everything; narrower: more hidden. The hidden count never shrinks as the bar narrows.
        assert_eq!(seen[0], 0);
        assert!(seen.windows(2).all(|w| w[1] >= w[0]), "{seen:?}");
        assert_eq!(*seen.last().unwrap(), 7);
        assert!(full > 0.0);
    }
}

#[test]
fn the_plus_n_chip_stays_in_the_title_bar_beside_the_rule_and_the_block() {
    for scale in SCALES {
        let mut t = vectorscope_toolbar();
        let width = 340.0 * scale;
        t.fit_to_reserving_floor(
            width,
            true,
            TitleBar::view_reserve(scale) + 100.0 * scale,
            TitleBar::view_reserve(scale) + 56.0 * scale,
            scale,
        );
        assert_eq!(
            t.chips.last().map(|c| c.text.clone()),
            Some(format!("+{}", t.hidden))
        );
        let l = bar(t.chips.clone(), t.groups.clone(), true, 340.0, scale);
        let plus = l.chips.last().unwrap();
        let rule = l.divider.unwrap();
        assert!(plus[0] + plus[2] + 20.0 * scale <= rule[0] + 0.01);
        assert!(plus[0] >= 12.0 * scale);
        assert!(!l.name_cut || l.name[2] > 0.0);
    }
}

#[test]
fn a_toolbar_with_room_has_no_plus_n_and_a_title_with_no_chips_has_none() {
    for scale in SCALES {
        let mut t = vectorscope_toolbar();
        t.fit_to_reserving_floor(2000.0 * scale, true, 300.0 * scale, 150.0 * scale, scale);
        assert_eq!((t.hidden, t.chips.len()), (0, 7));
        let mut none = Toolbar::default();
        none.fit_to_reserving_floor(10.0 * scale, true, 300.0 * scale, 150.0 * scale, scale);
        assert_eq!((none.hidden, none.chips.len()), (0, 0));
    }
}

#[test]
fn the_stylesheet_shows_the_accent_states() {
    use scopecreep::showcase::showcase;
    for scale in SCALES {
        let sheet = showcase(scale);
        let words: Vec<String> = sheet
            .prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        for want in ["App accent colour", "Split right", "+5", "Hex"] {
            assert!(words.iter().any(|w| w == want), "{want} on the sheet");
        }
        // The accent as a line (focus, underline, dashes, bar) and as text.
        let lines = fills(&sheet.prims)
            .iter()
            .filter(|c| **c == style::rgba(style::accent()))
            .count();
        assert!(lines > 20, "{lines}");
        assert!(text_colours(&sheet.prims).contains(&style::accent_text()));
    }
}

#[test]
fn the_window_default_is_at_least_its_least_size_and_every_size_goes_through_tokens() {
    let tree = scopecreep::layout_tree::LayoutTree::default_for(&[
        "Channel Viewer",
        "Waveform",
        "False Colour - Arri",
        "Vectorscope",
        "Saturation",
        "Skin Tone",
        "False Colour - Stops",
    ]);
    let [w, h] = scopecreep::layout::min_window(&tree, &menu_bar::largest_menus());
    assert_eq!([w, h], [1420.0, 833.0]);
    // The Window opens at 1440 x 840, over it.
    assert!(w <= 1440.0 && h <= 840.0);
    // The spacing tokens of the demo.
    for (got, want) in [
        (style::SPACE_MENU_BAR, 32.0),
        (style::SPACE_PAGE_BAR, 36.0),
        (style::SPACE_STATUS_ROW, 26.0),
        (style::SPACE_TITLE_BAR, 34.0),
        (style::SPACE_CHIP_HEIGHT, 24.0),
        (style::SPACE_CHIP_PAD, 9.0),
        (style::SPACE_CHIP_GAP, 2.0),
        (style::SPACE_CHIP_GROUP_GAP, 8.0),
        (style::SPACE_NAME_GAP, 12.0),
        (style::SPACE_DIVIDER_GAP, 20.0),
        (style::SPACE_RULE_WIDTH, 2.0),
        (style::SPACE_VIEW_GAP, 4.0),
        (style::SPACE_SETTINGS_GAP, 8.0),
        (style::SPACE_SETTINGS_BUTTON, 28.0),
        (style::SPACE_BAR_PAD, 12.0),
        (style::SPACE_MARGIN, 8.0),
        (style::SPACE_MENU_ROW, 30.0),
        (style::SPACE_POPOVER_ROW, 34.0),
        (style::SPACE_POPOVER_WIDTH, 400.0),
        (style::SPACE_POPOVER_PAD, 8.0),
        (style::SPACE_VALUE_BOX, 84.0),
        (style::SPACE_RESET_ICON, 24.0),
        (style::TYPE_LABEL, 13.0),
        (style::TYPE_SCALE, 12.0),
    ] {
        assert_eq!(got, want);
    }
}
