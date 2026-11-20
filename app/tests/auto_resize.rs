//! Auto resize at the pure layout seam (#213, spec #196, ADR 0041): ratios only, top down; a picture cell gets the
//! footage's aspect with room left for the title bar, the legend strip and the waveform's gutter; the clamp holds;
//! the Vectorscope is not picture-shaped; undo is one step; nothing runs it by itself.
use scopecreep::auto_resize::{self, DEFAULT_ASPECT, Shape, legend_allowance};
use scopecreep::layout_tree::LayoutTree;
use scopecreep::pages::Pages;
use scopecreep::panel_grid::PanelLayout;

const NAMES: [&str; 7] = [
    "Channel Viewer",
    "Waveform",
    "False Colour - Arri",
    "Vectorscope",
    "Saturation",
    "Skin Tone",
    "False Colour - Stops",
];
const GAP: f32 = 12.0;

fn shapes() -> Vec<Shape> {
    NAMES.iter().map(|n| auto_resize::shape_of(n)).collect()
}

fn tree(text: &str) -> LayoutTree {
    LayoutTree::parse_text(&NAMES, text).expect("a layout")
}

fn index(name: &str) -> usize {
    NAMES.iter().position(|n| *n == name).unwrap()
}

/// The saved text with every ratio left out: the places and the nesting.
fn skeleton(tree: &LayoutTree) -> String {
    tree.to_text(&NAMES)
        .lines()
        .map(|l| {
            if l.starts_with("split") {
                l.rsplit_once(' ').unwrap().0.to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn two(dir: &str, first: &str, second: &str) -> LayoutTree {
    tree(&format!(
        "layout 2\nsplit {dir} 0.5\nleaf 0 {first}\nleaf 0 {second}\n"
    ))
}

/// The aspect of the picture a Panel's cell leaves room for: the content area less the legend strip, and for the
/// waveform less its gutter.
fn picture_aspect(cell: [f32; 4], shape: Shape, scale: f32) -> f32 {
    let content = PanelLayout::new(cell, scale).content;
    let (gutter, legend) = legend_allowance(shape, scale);
    (content[2] - gutter) / (content[3] - legend)
}

fn assert_aspect(t: &LayoutTree, area: [f32; 4], panel: &str, aspect: f32, scale: f32) {
    let cell = t.cells(area, GAP * scale)[index(panel)];
    let got = picture_aspect(cell, shapes()[index(panel)], scale);
    assert!(
        (got - aspect).abs() < 0.01 * aspect,
        "{panel}: wanted {aspect}, got {got}"
    );
}

#[test]
fn a_picture_cell_gets_the_footage_aspect_leaving_room_for_the_title_bar_legend_and_gutter() {
    let wide = [0.0, 0.0, 1000.0, 1200.0];
    let tall = [0.0, 0.0, 1600.0, 600.0];
    let cases = [
        ("row", tall, 16.0 / 9.0),
        ("row", tall, 4.0 / 3.0),
        ("row", tall, 1.0),
        ("row", tall, 9.0 / 16.0),
        ("col", wide, 16.0 / 9.0),
        ("col", wide, 2.39),
    ];
    for scale in [1.0, 2.0] {
        for (dir, area, aspect) in cases {
            let area = area.map(|v| v * scale);
            for panel in ["Channel Viewer", "Waveform", "Skin Tone"] {
                for picture_first in [true, false] {
                    let mut t = if picture_first {
                        two(dir, panel, "Vectorscope")
                    } else {
                        two(dir, "Vectorscope", panel)
                    };
                    t.auto_resize(&shapes(), aspect, area, GAP * scale, scale);
                    assert_aspect(&t, area, panel, aspect, scale);
                }
            }
        }
    }
}

#[test]
fn only_ratios_change_places_and_nesting_stay() {
    let area = [0.0, 0.0, 1256.0, 716.0];
    let mut t = LayoutTree::default_for(&NAMES);
    let before = skeleton(&t);
    let cells_before = t.cells(area, GAP);
    t.auto_resize(&shapes(), 2.39, area, GAP, 1.0);
    assert_eq!(skeleton(&t), before);
    assert_ne!(t.cells(area, GAP), cells_before, "the cells moved");
}

#[test]
fn a_split_with_two_picture_children_or_none_is_left_alone() {
    for text in [
        "layout 2\nsplit row 0.3\nleaf 0 Clean picture\nleaf 0 Skin tone\n",
        "layout 2\nsplit col 0.7\nleaf 0 Vectorscope\nleaf 0 Colour waveform\n",
    ] {
        let mut t = tree(text);
        let before = t.to_text(&NAMES);
        t.auto_resize(&shapes(), 1.0, [0.0, 0.0, 1256.0, 716.0], GAP, 1.0);
        assert_eq!(t.to_text(&NAMES), before);
    }
}

#[test]
fn the_vectorscope_is_not_picture_shaped_and_the_luma_waveform_is() {
    assert_eq!(auto_resize::shape_of("Vectorscope"), Shape::Other);
    assert_eq!(auto_resize::shape_of("Colour waveform"), Shape::Other);
    assert_eq!(auto_resize::shape_of("Waveform"), Shape::Waveform);
    assert_eq!(auto_resize::shape_of("Channel Viewer"), Shape::Picture);
    assert_eq!(
        auto_resize::shape_of("False Colour - Stops"),
        Shape::Picture
    );
}

#[test]
fn the_ratio_is_clamped_to_15_to_85_percent() {
    let area = [0.0, 0.0, 1256.0, 716.0];
    let mut t = two("row", "Channel Viewer", "Vectorscope");
    t.auto_resize(&shapes(), 2.39, area, GAP, 1.0);
    let c = t.cells(area, GAP);
    assert!((c[0][2] / (area[2] - GAP) - 0.85).abs() < 1e-4, "too wide");
    let thin = [0.0, 0.0, 3000.0, 300.0];
    let mut t = two("row", "Channel Viewer", "Vectorscope");
    t.auto_resize(&shapes(), 9.0 / 16.0, thin, GAP, 1.0);
    let c = t.cells(thin, GAP);
    assert!((c[0][2] / (thin[2] - GAP) - 0.15).abs() < 1e-4, "too thin");
    let mut t = two("row", "Vectorscope", "Channel Viewer");
    t.auto_resize(&shapes(), 2.39, area, GAP, 1.0);
    let c = t.cells(area, GAP);
    assert!(
        (c[0][2] / (area[2] - GAP) - 0.85).abs() < 1e-4,
        "picture second"
    );
}

#[test]
fn it_goes_top_down_so_a_nested_split_sees_its_parents_new_size() {
    let area = [0.0, 0.0, 2000.0, 900.0];
    let mut t = tree(
        "layout 2\nsplit row 0.5\nleaf 0 Clean picture\nsplit col 0.5\nsplit row 0.5\nleaf 0 Skin tone\nleaf 0 Vectorscope\nleaf 0 Saturation heatmap\n",
    );
    t.auto_resize(&shapes(), 16.0 / 9.0, area, GAP, 1.0);
    assert_aspect(&t, area, "Channel Viewer", 16.0 / 9.0, 1.0);
    // The split below the first has two shapes of its own: the Skin tone is fitted in the column that is left.
    let cells = t.cells(area, GAP);
    let skin = cells[index("Skin Tone")];
    let scope = cells[index("Vectorscope")];
    let content = PanelLayout::new(skin, 1.0).content;
    let (_, legend) = legend_allowance(Shape::Picture, 1.0);
    let want = (content[3] - legend) * 16.0 / 9.0;
    assert!((content[2] - want).abs() < 0.02 * want || skin[2] / (skin[2] + scope[2]) <= 0.851);
}

#[test]
fn no_feed_means_16_to_9() {
    assert!((DEFAULT_ASPECT - 16.0 / 9.0).abs() < 1e-6);
    assert!((auto_resize::aspect_of(None) - 16.0 / 9.0).abs() < 1e-6);
    assert!((auto_resize::aspect_of(Some([0, 0])) - 16.0 / 9.0).abs() < 1e-6);
    assert!((auto_resize::aspect_of(Some([2048, 858])) - 2048.0 / 858.0).abs() < 1e-4);
}

#[test]
fn it_is_one_undo_step_and_a_press_that_changes_nothing_is_none() {
    let area = [0.0, 0.0, 1256.0, 716.0];
    let mut pages = Pages::new(two("row", "Channel Viewer", "Vectorscope"));
    let before = pages.tree().to_text(&NAMES);
    pages.change(|t| t.auto_resize(&shapes(), 2.39, area, GAP, 1.0));
    assert_ne!(pages.tree().to_text(&NAMES), before);
    assert!(pages.undo());
    assert_eq!(pages.tree().to_text(&NAMES), before);
    assert!(!pages.can_undo(), "one step");
    pages.change(|t| t.auto_resize(&shapes(), 2.39, area, GAP, 1.0));
    pages.change(|t| t.auto_resize(&shapes(), 2.39, area, GAP, 1.0));
    assert!(pages.undo());
    assert!(
        !pages.can_undo(),
        "a press that changes nothing adds no step"
    );
}

#[test]
fn it_never_leaves_a_panel_under_its_least_size_near_the_windows_minimum() {
    use scopecreep::panel_grid::min_panel_size;
    for scale in [1.0, 2.0] {
        let min = min_panel_size(scale);
        // Just wide enough for two Panels at their least size, so the footage's share cannot be had.
        let area = [0.0, 0.0, 2.0 * min[0] + GAP * scale + 30.0, 716.0 * scale];
        for (first, second) in [
            ("Channel Viewer", "Vectorscope"),
            ("Vectorscope", "Waveform"),
        ] {
            let mut t = two("row", first, second);
            t.auto_resize(&shapes(), 2.39, area, GAP * scale, scale);
            let cells = t.cells(area, GAP * scale);
            for cell in t.panels().into_iter().map(|p| cells[p]) {
                assert!(cell[2] >= min[0] - 0.01, "{first}|{second}: {cell:?}");
                assert!(cell[3] >= min[1] - 0.01, "{cell:?} under {min:?}");
            }
        }
    }
}
