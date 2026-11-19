//! The showcase (#115): every component in every state on one sheet, for a development example to draw to an
//! image and a test to check. Layout numbers here are the sheet's own, not chrome, so this file is not in the
//! style check's list; every colour and size of a component still comes from its own module.
use crate::components::*;
use crate::settings::Menu;
use crate::style;

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
    let caption = |t: &str, x: f32, y: f32, w: f32| text(t, [x, y, w, row], Face::Scale, style::TEXT_MUTED, Align::Left);
    let mut y = margin;
    prims.push(rect([0.0, 0.0, width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale], style::SURFACE_WINDOW));
    for (i, state) in STATES.iter().enumerate() {
        prims.push(caption(&format!("{state:?}"), margin + label_w + column * i as f32, y, column));
    }
    y += row;
    let at = |i: usize, y: f32| [margin + label_w + column * i as f32, y + (row - style::SPACE_CHIP_HEIGHT * scale) / 2.0];
    let sample = [("Chip off", false, None, "Y"), ("Chip on", true, None, "Y"), ("R on", true, Some(Channel::R), "R"), ("G on", true, Some(Channel::G), "G"), ("B on", true, Some(Channel::B), "B"), ("Y on", true, Some(Channel::Y), "Y")];
    for (name, on, channel, letter) in sample {
        prims.push(caption(name, margin, y, label_w));
        for (i, state) in STATES.iter().enumerate() {
            let chip = Chip { text: letter.to_string(), channel };
            prims.extend(chip.draw(at(i, y), on, *state, scale));
        }
        y += row;
    }
    prims.push(caption("Segmented", margin, y, label_w));
    let segmented = Segmented::new(&["Luma", "Chroma", "Both"]);
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(segmented.draw([at(i, y)[0], y + (row - style::SPACE_SEGMENT_HEIGHT * scale) / 2.0], 0, 1, *state, scale));
    }
    y += row;
    prims.push(caption("Slider with number", margin, y, label_w));
    let slider = Slider { value: 0.6, number: "75 %".to_string() };
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(slider.draw([at(i, y)[0], y + (row - style::SPACE_SLIDER_HEIGHT * scale) / 2.0], *state, scale));
    }
    y += row;
    prims.push(caption("Settings button", margin, y, label_w));
    for (i, state) in STATES.iter().enumerate() {
        prims.extend(settings_button([at(i, y)[0], at(i, y)[1], style::SPACE_SETTINGS_BUTTON * scale, style::SPACE_SETTINGS_BUTTON * scale], *state, scale));
    }
    y += row * 1.5;

    // Popover and tooltip side by side.
    prims.push(caption("Popover, Tooltip", margin, y, label_w));
    let defs = [
        crate::settings::SettingDef { key: "weights", label: "Luma weights", amount: None, choices: &[crate::settings::Choice { value: "a", text: "Measured Space" }, crate::settings::Choice { value: "b", text: "Match Resolve" }] },
        crate::settings::SettingDef { key: "spread", label: "Soft spread", amount: None, choices: &[crate::settings::Choice { value: "on", text: "On" }, crate::settings::Choice { value: "off", text: "Off" }] },
    ];
    let current = crate::settings::PanelSettings::default().resolve("sample", &defs);
    let menu = Menu::new(0, &defs, &current).expect("the sample has settings");
    let area = [margin + label_w, y, 0.0, 0.0];
    let popover_rect = menu.rect([area[0], area[1]], [width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale], scale);
    // The second choice of the first setting is under the pointer.
    let hover = [popover_rect[0] + popover_rect[2] / 2.0, Menu::row_top(popover_rect, 2, scale) + row / 4.0];
    prims.extend(popover(&menu, popover_rect, hover, scale));
    let tip = Tooltip::new("Red channel");
    let anchor = Chip::channel("R", Channel::R).rect([margin + label_w + column + style::SPACE_SHOWCASE_ANCHOR_OFFSET * scale, y], scale);
    prims.extend(Chip::channel("R", Channel::R).draw([anchor[0], anchor[1]], true, State::Hover, scale));
    prims.extend(tip.draw(tip.rect(anchor, [width, style::SPACE_SHOWCASE_SHEET_HEIGHT * scale], scale), scale));
    y += popover_rect[3] + row * 0.5;

    // Panel frame with a wide and a narrow title bar.
    prims.push(caption("Panel frame, title bar", margin, y, label_w));
    let bar_chips = vec![Chip::channel("Y", Channel::Y), Chip::channel("R", Channel::R), Chip::channel("G", Channel::G), Chip::channel("B", Channel::B)];
    let title = TitleBar { name: "Luma waveform".to_string(), tags: "Rec.709 weights, 100 %".to_string(), chips: bar_chips, settings_button: true };
    let states = [(true, State::Normal), (true, State::Normal), (false, State::Hover), (false, State::Normal)];
    for (i, panel_width) in [style::SPACE_SHOWCASE_PANEL_WIDE, style::SPACE_SHOWCASE_PANEL_NARROW].into_iter().enumerate() {
        let frame = [margin + label_w + (style::SPACE_SHOWCASE_PANEL_WIDE * scale + margin) * i as f32, y + 2.0 * scale, panel_width * scale, style::SPACE_SHOWCASE_PANEL_HEIGHT * scale];
        prims.extend(panel_frame(frame, scale));
        prims.extend(title.draw(frame, &states, State::Normal, scale));
    }
    y += style::SPACE_SHOWCASE_PANEL_STEP * scale;

    prims.push(caption("Top bar", margin, y, label_w));
    prims.extend(bar(BarKind::Top, margin + label_w, y, column * 4.0, "scopecreep", "Colour Setup: Rec.709 / Gamma 2.4", scale));
    y += BarKind::Top.height(scale) + row * 0.5;
    prims.push(caption("Status bar", margin, y, label_w));
    prims.extend(bar(BarKind::Status, margin + label_w, y, column * 4.0, "Resolve playing at 50 fps, 0 dropped", "Sender A", scale));
    y += BarKind::Status.height(scale) + row * 0.5;

    prims.push(caption("Legend strip", margin, y, label_w));
    let legend = LegendStrip {
        items: (0..6)
            .map(|i| {
                let v = i as f32 / 5.0;
                LegendItem { colour: [v, v * 0.5, 1.0 - v], label: format!("{}", i * 20) }
            })
            .collect(),
    };
    prims.extend(legend.draw(margin + label_w, y + 4.0 * scale, column * 4.0, scale));
    y += style::SPACE_LEGEND_HEIGHT * scale + margin * 2.0;
    prims.push(caption("The legend's colours are a display's data; the sample ramp is not part of the style.", margin, y, width - 2.0 * margin));
    y += row;

    // Trim the sheet's background to what is drawn.
    if let Some(Prim::Rect { rect, .. }) = prims.first_mut() {
        rect[3] = y;
    }
    Showcase { size: [width, y], prims }
}
