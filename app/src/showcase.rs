//! The showcase (#115): every component in every state on one sheet, for a development example to draw to an
//! image and a test to check. Layout numbers here are the sheet's own, not chrome, so this file is not in the
//! style check's list; every colour and size of a component still comes from its own module.
use crate::components::*;
use crate::layout_tree::Edge;
use crate::panel_grid::PanelLayout;
use crate::settings::Menu;
use crate::style;
use crate::tab_strip::{TabHit, TabStrip};
use crate::view_block::ViewBlock;

/// Every component in every state, laid out on one sheet.
pub struct Showcase {
    /// The sheet's width and height in device pixels.
    pub size: [f32; 2],
    pub prims: Vec<Prim>,
}

/// The sheet a development example draws to an image: one row per component, one column per state, plus the
/// parts that are not states (popover, tooltip, title bars, bars, legend).
pub fn showcase(scale: f32) -> Showcase {
    let margin = style::SPACE_MARGIN * scale;
    let label_w = style::SPACE_SHOWCASE_LABEL * scale;
    let column = style::SPACE_SHOWCASE_COLUMN * scale;
    let row = style::SPACE_SHOWCASE_ROW * scale;
    let width = margin * 2.0 + label_w + column * STATES.len() as f32;
    let mut prims = vec![];
    let caption = |t: &str, x: f32, y: f32, w: f32| {
        text(
            t,
            [x, y, w, row],
            Face::Scale,
            style::TEXT_MUTED,
            Align::Left,
        )
    };
    let mut y = margin;
    prims.push(rect(
        [0.0, 0.0, width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale],
        style::SURFACE_WINDOW,
    ));
    for (i, state) in STATES.iter().enumerate() {
        prims.push(caption(
            &format!("{state:?}"),
            margin + label_w + column * i as f32,
            y,
            column,
        ));
    }
    y += row;
    let at = |i: usize, y: f32| {
        [
            margin + label_w + column * i as f32,
            y + (row - style::SPACE_CHIP_HEIGHT * scale) / 2.0,
        ]
    };
    let sample = [
        ("Chip off", false, None, "Y"),
        ("Chip on", true, None, "Y"),
        ("R on", true, Some(Channel::R), "R"),
        ("G on", true, Some(Channel::G), "G"),
        ("B on", true, Some(Channel::B), "B"),
        ("Y on", true, Some(Channel::Y), "Y"),
    ];
    for (name, on, channel, letter) in sample {
        prims.push(caption(name, margin, y, label_w));
        for (i, state) in STATES.iter().enumerate() {
            let chip = Chip {
                text: letter.to_string(),
                channel,
            };
            prims.extend(chip.draw(at(i, y), on, *state, scale));
        }
        y += row;
    }
    prims.push(caption("Segmented", margin, y, label_w));
    let segmented = Segmented::new(&["Luma", "Chroma", "Both"]);
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(segmented.draw(
            [
                at(i, y)[0],
                y + (row - style::SPACE_SEGMENT_HEIGHT * scale) / 2.0,
            ],
            0,
            1,
            *state,
            scale,
        ));
    }
    y += row;
    prims.push(caption("Slider with number", margin, y, label_w));
    let slider = Slider {
        value: 0.6,
        number: "75 %".to_string(),
    };
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(slider.draw(
            [
                at(i, y)[0],
                y + (row - style::SPACE_SLIDER_HEIGHT * scale) / 2.0,
            ],
            *state,
            scale,
        ));
    }
    y += row;
    prims.push(caption("Settings button", margin, y, label_w));
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(settings_button(
            [
                at(i, y)[0],
                at(i, y)[1],
                style::SPACE_SETTINGS_BUTTON * scale,
                style::SPACE_CHIP_HEIGHT * scale,
            ],
            *state,
            scale,
        ));
    }
    y += row * 1.5;

    // Popover and tooltip side by side.
    prims.push(caption("Popover, Tooltip", margin, y, label_w));
    let defs = [
        crate::settings::SettingDef {
            key: "weights",
            label: "Luma weights",
            amount: None,
            choices: &[
                crate::settings::Choice {
                    value: "a",
                    text: "Measured Space",
                },
                crate::settings::Choice {
                    value: "b",
                    text: "Match Resolve",
                },
            ],
        },
        crate::settings::SettingDef {
            key: "spread",
            label: "Soft spread",
            amount: None,
            choices: &[
                crate::settings::Choice {
                    value: "on",
                    text: "On",
                },
                crate::settings::Choice {
                    value: "off",
                    text: "Off",
                },
            ],
        },
    ];
    let current = crate::settings::PanelSettings::default().resolve("sample", &defs);
    let menu = Menu::new(0, &defs, &current).expect("the sample has settings");
    let area = [margin + label_w, y, 0.0, 0.0];
    let popover_rect = menu.rect(
        [area[0], area[1]],
        [width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale],
        scale,
    );
    // The second choice of the first setting is under the pointer.
    let hover = [
        popover_rect[0] + popover_rect[2] / 2.0,
        menu.row_top(popover_rect, 2, scale) + row / 4.0,
    ];
    prims.extend(popover(&menu, popover_rect, hover, scale));
    let tip = Tooltip::new("Red channel");
    let anchor = Chip::channel("R", Channel::R).rect(
        [
            margin + label_w + column + style::SPACE_SHOWCASE_ANCHOR_OFFSET * scale,
            y,
        ],
        scale,
    );
    prims.extend(Chip::channel("R", Channel::R).draw(
        [anchor[0], anchor[1]],
        true,
        State::Hover,
        scale,
    ));
    prims.extend(tip.draw(
        tip.rect(
            anchor,
            [width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale],
            scale,
        ),
        scale,
    ));
    y += popover_rect[3] + row * 0.5;

    // Panel frames: a focused picture Panel (accent line along its title bar, chips, the rule, the view block), the same
    // Panel narrow with its chips given way to "+N", and a Scope, which has no view block.
    prims.push(caption("Title bar: focused, +N, Scope", margin, y, label_w));
    let bar_chips = vec![
        Chip::channel("Y", Channel::Y),
        Chip::channel("R", Channel::R),
        Chip::channel("G", Channel::G),
        Chip::channel("B", Channel::B),
        Chip::new("B/W"),
    ];
    let picture = TitleBar {
        name: "Channel Viewer".to_string(),
        tags: "Rec.709".to_string(),
        chips: bar_chips.clone(),
        groups: vec![0, 0, 0, 0, 1],
        settings_button: true,
        view: true,
    };
    let overflow = TitleBar {
        name: "Channel Viewer".to_string(),
        tags: String::new(),
        chips: vec![Chip::new("+5")],
        groups: vec![0],
        settings_button: true,
        view: true,
    };
    let scope = TitleBar {
        name: "Waveform".to_string(),
        tags: "Rec.709 weights, 100 %".to_string(),
        chips: bar_chips[..4].to_vec(),
        groups: Vec::new(),
        settings_button: true,
        view: false,
    };
    let chip_states = [
        (false, State::Normal),
        (false, State::Normal),
        (false, State::Hover),
        (true, State::Normal),
        (true, State::Normal),
    ];
    let frames = [
        (
            &picture,
            style::SPACE_SHOWCASE_PANEL_WIDE,
            true,
            0usize,
            0.0f32,
        ),
        (&overflow, style::SPACE_SHOWCASE_PANEL_NARROW, false, 1, 0.0),
        (&scope, style::SPACE_SHOWCASE_PANEL_WIDE, false, 0, 1.0),
    ];
    let mut x_next = margin + label_w;
    let mut y_row = y;
    for (title, panel_width, focused, _, new_row) in frames {
        if new_row > 0.0 {
            y_row += style::SPACE_SHOWCASE_PANEL_STEP * scale;
            x_next = margin + label_w;
        }
        let frame = [
            x_next,
            y_row + 2.0 * scale,
            panel_width * scale,
            style::SPACE_SHOWCASE_PANEL_HEIGHT * scale,
        ];
        x_next += panel_width * scale + margin;
        prims.extend(panel_frame(frame, scale));
        let bar_rect = PanelLayout::new(frame, scale).title;
        prims.extend(title.draw_focused(
            bar_rect,
            &chip_states[..title.chips.len()],
            State::Normal,
            focused,
            scale,
        ));
        if let Some(view) = title.layout(bar_rect, scale).view {
            let block = ViewBlock {
                enabled: true,
                readout: "62 %".to_string(),
                link: true,
            };
            prims.extend(block.draw(&view, None, scale));
        }
    }
    y = y_row + style::SPACE_SHOWCASE_PANEL_STEP * scale;

    // The accent's other states: a tab group's tabs, the dock overlay with its label, the edge +, and the Appearance
    // panel with its swatches and the contrast notice.
    prims.push(caption("Tab strip, edge +", margin, y, label_w));
    let strip = TabStrip {
        labels: vec!["Waveform".into(), "Vectorscope".into(), "Saturation".into()],
        active: 1,
    };
    let tabs_frame = [
        margin + label_w,
        y + 2.0 * scale,
        style::SPACE_SHOWCASE_PANEL_WIDE * scale,
        style::SPACE_SHOWCASE_PANEL_HEIGHT * scale,
    ];
    prims.extend(panel_frame(tabs_frame, scale));
    let tabs_bar = PanelLayout::new(tabs_frame, scale).title;
    prims.push(rect(tabs_bar, style::SURFACE_PANEL));
    prims.extend(strip.draw(
        [
            tabs_bar[0] + style::SPACE_BAR_PAD * scale,
            tabs_bar[1],
            tabs_bar[2] - 2.0 * style::SPACE_BAR_PAD * scale,
            tabs_bar[3],
        ],
        Some(TabHit::Tab(0)),
        scale,
    ));
    let edge_lit = edge_plus_draw(tabs_frame, Some(Edge::Right), scale);
    prims.extend(edge_lit);
    let dock_target = [
        tabs_frame[0] + tabs_frame[2] + margin,
        tabs_frame[1],
        style::SPACE_SHOWCASE_PANEL_NARROW * scale / 2.0,
        tabs_frame[3],
    ];
    prims.extend(dock_overlay(dock_target, scale));
    prims.push(text(
        "Split right",
        dock_target,
        Face::Label,
        style::TEXT_PRIMARY,
        Align::Centre,
    ));
    y += style::SPACE_SHOWCASE_PANEL_STEP * scale;

    prims.push(caption("Appearance: accent", margin, y, label_w));
    let appearance = crate::appearance_panel::AppearancePanel::default()
        .menu(crate::accent::current())
        .drop;
    let appearance_area =
        appearance.rect_near([margin + label_w, y, 0.0, 0.0], [width, f32::MAX], scale);
    prims.extend(appearance.draw(appearance_area, Some(2), false, scale));
    y = appearance_area[1] + appearance_area[3] + row * 0.5;

    prims.push(caption("Menu bar", margin, y, label_w));
    let menu_bar = crate::menu_bar::bar();
    prims.extend(menu_bar.draw(margin + label_w, y, column * 4.0, Some(0), None, scale));
    let title = menu_bar.title_rects(margin + label_w, y, scale)[0];
    let mut on = MenuItem::new("1  Grade", "Ctrl+1");
    on.on = true;
    let mut off = MenuItem::new("Rename layout…", "");
    off.enabled = false;
    let drop = DropMenu {
        entries: vec![
            MenuEntry::Item(MenuItem::new("Default layout", "")),
            MenuEntry::Separator,
            MenuEntry::Item(on),
            MenuEntry::Item(MenuItem::new("2  empty", "Ctrl+Shift+2")),
            MenuEntry::Separator,
            MenuEntry::Item(off),
        ],
    };
    let area = drop.rect(title, [width, f32::MAX], scale);
    prims.extend(drop.draw(area, Some(3), false, scale));
    y = area[1] + area[3] + row * 0.5;

    prims.push(caption("Top bar", margin, y, label_w));
    prims.extend(bar(
        BarKind::Top,
        margin + label_w,
        y,
        column * 4.0,
        "ScopeCreep",
        "Colour Setup: Rec.709 / Gamma 2.4",
        scale,
    ));
    y += BarKind::Top.height(scale) + row * 0.5;
    prims.push(caption("Status bar", margin, y, label_w));
    prims.extend(bar(
        BarKind::Status,
        margin + label_w,
        y,
        column * 4.0,
        "Resolve playing at 50 fps, 0 dropped",
        "Sender A",
        scale,
    ));
    y += BarKind::Status.height(scale) + row * 0.5;

    prims.push(caption("Legend strip", margin, y, label_w));
    let legend = LegendStrip {
        items: (0..6)
            .map(|i| {
                let v = i as f32 / 5.0;
                LegendItem {
                    colour: [v, v * 0.5, 1.0 - v],
                    label: format!("{}", i * 20),
                }
            })
            .collect(),
    };
    prims.extend(legend.draw(margin + label_w, y + 4.0 * scale, column * 4.0, scale));
    y += style::SPACE_LEGEND_HEIGHT * scale + margin * 2.0;
    prims.push(caption(
        "The legend's colours are a display's data; the sample ramp is not part of the style.",
        margin,
        y,
        width - 2.0 * margin,
    ));
    y += row;

    // Trim the sheet's background to what is drawn.
    if let Some(Prim::Rect { rect, .. }) = prims.first_mut() {
        rect[3] = y;
    }
    Showcase {
        size: [width, y],
        prims,
    }
}
