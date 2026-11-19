//! The scale bar (#122), the legend strip ARRI False Colour and the saturation heatmap share: pure layout, no GPU.
use scopecreep::arri_false_colour::BAR;
use scopecreep::display::ScaleText;
use scopecreep::scale_bar::ScaleBar;

fn texts(bar: &ScaleBar, width: f32, scale: f32) -> Vec<ScaleText> {
    let layout = bar.layout(true, [0.0, 0.0, width, 400.0 * scale], scale, true).unwrap();
    layout.ticks.into_iter().chain(layout.marks).collect()
}

#[test]
fn the_bar_is_only_there_when_on_and_in_a_panel_tall_enough_and_follows_the_scale() {
    let area = |h: f32| [0.0, 0.0, 200.0, h];
    assert_eq!(BAR.layout(true, area(109.0), 1.0, true), None);
    assert_eq!(BAR.layout(false, area(300.0), 1.0, true), None);
    let one = BAR.layout(true, area(110.0), 1.0, true).unwrap().legend;
    let two = BAR.layout(true, area(220.0), 2.0, true).unwrap().legend;
    assert_eq!((two.strip, two.bar_offset, two.bar), (one.strip * 2.0, one.bar_offset * 2.0, one.bar * 2.0));
    assert_eq!(one.bar, 8.0);
    assert!(BAR.layout(true, area(219.0), 2.0, true).is_none());
    // No picture yet: the strip is kept and has no text.
    let bare = BAR.layout(true, area(300.0), 1.0, false).unwrap();
    assert!(bare.ticks.is_empty() && bare.marks.is_empty());
}

#[test]
fn ticks_run_from_the_start_to_the_end_with_the_unit_on_the_end() {
    let said: Vec<String> = BAR.layout(true, [0.0, 0.0, 600.0, 400.0], 1.0, true).unwrap().ticks.into_iter().map(|t| t.text).collect();
    assert_eq!(said.first().map(String::as_str), Some("0"));
    assert_eq!(said.last().map(String::as_str), Some("100 %"));
    assert_eq!(said.len(), 20, "every 5 % where they fit, 95 giving way to the wider end");
    let medium: Vec<String> = BAR.layout(true, [0.0, 0.0, 300.0, 400.0], 1.0, true).unwrap().ticks.into_iter().map(|t| t.text).collect();
    assert!(medium.len() < said.len() && medium.contains(&"50".to_string()), "{medium:?}");
}

#[test]
fn a_scale_that_is_not_a_multiple_of_the_step_ends_on_itself() {
    let bar = ScaleBar { from: 0.0, to: 75.0, unit: " %", steps: &[10.0], marks: &[] };
    let said: Vec<String> = bar.layout(true, [0.0, 0.0, 300.0, 400.0], 1.0, true).unwrap().ticks.into_iter().map(|t| t.text).collect();
    assert_eq!(said, ["0", "10", "20", "30", "40", "50", "60", "75 %"], "70 gives way to the end");
}

#[test]
fn no_text_overlaps_or_leaves_the_bar_at_any_width() {
    for scale in [1.0f32, 2.0] {
        for width in (40..1400).step_by(13) {
            let width = width as f32 * scale;
            for bar in [&BAR, &ScaleBar { from: 0.0, to: 200.0, unit: " %", steps: &[10.0, 25.0, 50.0, 100.0], marks: &[] }] {
                let all = texts(bar, width, scale);
                for (i, a) in all.iter().enumerate() {
                    assert!(a.left >= -0.01 && a.left + a.width <= width + 0.01, "{} leaves the bar at {width}", a.text);
                    for c in &all[i + 1..] {
                        let same_line = (a.top - c.top).abs() < 1.0;
                        assert!(!(same_line && a.left < c.left + c.width && c.left < a.left + a.width), "{} touches {} at {width}", a.text, c.text);
                    }
                }
            }
        }
    }
}

#[test]
fn marks_sit_under_the_bar_in_order_of_importance_and_drop_out_when_they_do_not_fit() {
    let marks = |width: f32| -> Vec<String> { BAR.layout(true, [0.0, 0.0, width, 400.0], 1.0, true).unwrap().marks.into_iter().map(|t| t.text).collect() };
    assert_eq!(marks(900.0), ["near black clip", "18 % grey", "near white clip"]);
    assert_eq!(marks(400.0), ["near black clip", "18 % grey", "near white clip"]);
    assert_eq!(marks(200.0), ["near black clip", "white clip"], "what collides with a more important mark is dropped, the clip ends come last");
    assert!(marks(40.0).is_empty());
    // The marks line is under the bar, the ticks line over it.
    let l = BAR.layout(true, [0.0, 0.0, 900.0, 400.0], 1.0, true).unwrap();
    let bar_top = 400.0 - l.legend.strip + l.legend.bar_offset;
    assert!(l.ticks.iter().all(|t| t.top < bar_top) && l.marks.iter().all(|t| t.top > bar_top + l.legend.bar));
}
