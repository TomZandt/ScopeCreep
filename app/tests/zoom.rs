//! The zoom model at its pure seam (spec #163, ticket #164): where a picture goes in its Panel. Fit is the only
//! zoom so far, and it must place exactly as the fit-only placement always did. No GPU.
use scopecreep::picture_stage::place;
use scopecreep::zoom::{Geometry, ZoomModel};

const SHAPES: [[u32; 2]; 5] = [[1920, 1080], [1080, 1920], [3840, 2160], [64, 36], [1, 1]];
const AREAS: [[f32; 4]; 5] = [
    [0.0, 0.0, 192.0, 200.0],
    [0.0, 0.0, 300.0, 100.0],
    [10.5, 20.25, 333.0, 187.0],
    [8.0, 8.0, 1424.0, 738.0],
    [0.0, 0.0, 0.5, 90.0],
];

#[test]
fn at_fit_every_frame_and_panel_places_as_before() {
    let model = ZoomModel::default();
    for picture in SHAPES {
        for area in AREAS {
            for strip in [0.0, 40.0, 95.0] {
                assert_eq!(
                    model.placement(Geometry::new(area, picture, strip)),
                    place(area, picture, strip),
                    "{picture:?} in {area:?} strip {strip}"
                );
            }
        }
    }
}

#[test]
fn a_frame_over_100_percent_at_fit_stays_at_fit_and_says_so() {
    let model = ZoomModel::default();
    let p = model
        .placement(Geometry::new([0.0, 0.0, 1000.0, 1000.0], [100, 50], 0.0))
        .unwrap();
    assert_eq!((p.w, p.h), (1000.0, 500.0));
    assert_eq!(
        model.fit_percent(Geometry::new([0.0, 0.0, 1000.0, 1000.0], [100, 50], 0.0)),
        Some(1000)
    );
}

#[test]
fn fit_percent_is_how_much_the_frame_is_scaled() {
    let model = ZoomModel::default();
    assert_eq!(
        model.fit_percent(Geometry::new([0.0, 0.0, 442.0, 400.0], [1920, 1080], 0.0)),
        Some(23)
    );
    // The strip kept for a legend takes room from the fit.
    assert_eq!(
        model.fit_percent(Geometry::new(
            [0.0, 0.0, 1920.0, 1180.0],
            [1920, 1080],
            100.0
        )),
        Some(100)
    );
    assert_eq!(
        model.fit_percent(Geometry::new([0.0, 0.0, 0.5, 90.0], [1920, 1080], 0.0)),
        None
    );
    assert_eq!(
        model.fit_percent(Geometry::new([0.0, 0.0, 100.0, 100.0], [0, 0], 0.0)),
        None
    );
}

// ---- zoom (#165)

use scopecreep::zoom::{Zoom, ZoomModel as Model};

const PANEL: [f32; 4] = [10.0, 20.0, 400.0, 300.0];
const FRAME: [u32; 2] = [1920, 1080];

fn percent(m: &Model) -> f32 {
    m.percent(Geometry::new(PANEL, FRAME, 0.0)).unwrap()
}

fn wheel_in(m: &mut Model, at: [f32; 2], notches: u32) {
    for _ in 0..notches {
        m.wheel(Geometry::new(PANEL, FRAME, 0.0), at, 1.0);
    }
}

#[test]
fn wheel_in_from_fit_is_free_below_100_lands_on_100_then_whole_steps_and_stops_at_800() {
    let mut m = Model::default();
    let fit = percent(&m);
    assert!((fit - 400.0 / 1920.0 * 100.0).abs() < 0.01);
    let mut seen = vec![fit];
    for _ in 0..30 {
        m.wheel(Geometry::new(PANEL, FRAME, 0.0), [200.0, 150.0], 1.0);
        let p = percent(&m);
        if seen.last() != Some(&p) {
            seen.push(p);
        }
    }
    assert!(
        seen.windows(2).all(|w| w[1] > w[0]),
        "each notch zooms in: {seen:?}"
    );
    let tail: Vec<f32> = seen.iter().copied().filter(|p| *p >= 100.0).collect();
    assert_eq!(
        tail,
        [100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0],
        "lands on 100 then whole steps"
    );
    assert!(
        seen.iter().filter(|p| **p < 100.0).count() > 3,
        "free steps below 100: {seen:?}"
    );
    assert_eq!(m.zoom, Zoom::Percent(800.0));
}

#[test]
fn wheel_out_retraces_the_steps_and_stops_at_fit() {
    let mut m = Model::default();
    wheel_in(&mut m, [200.0, 150.0], 40);
    assert_eq!(percent(&m), 800.0);
    let mut seen = vec![800.0];
    for _ in 0..40 {
        m.wheel(Geometry::new(PANEL, FRAME, 0.0), [200.0, 150.0], -1.0);
        let p = percent(&m);
        if seen.last() != Some(&p) {
            seen.push(p);
        }
    }
    assert_eq!(
        &seen[..8],
        [800.0, 700.0, 600.0, 500.0, 400.0, 300.0, 200.0, 100.0]
    );
    assert!(seen[8] < 100.0);
    assert_eq!(m.zoom, Zoom::Fit, "stops at Fit");
    let fit = percent(&m);
    m.wheel(Geometry::new(PANEL, FRAME, 0.0), [200.0, 150.0], -1.0);
    assert_eq!(percent(&m), fit);
}

#[test]
fn a_part_of_a_notch_waits_for_the_rest() {
    let mut m = Model::default();
    m.wheel(Geometry::new(PANEL, FRAME, 0.0), [200.0, 150.0], 0.4);
    assert_eq!(m.zoom, Zoom::Fit);
    m.wheel(Geometry::new(PANEL, FRAME, 0.0), [200.0, 150.0], 0.7);
    assert_ne!(m.zoom, Zoom::Fit);
}

/// The picture pixel under `at`, from where the model places the picture.
fn pixel_under(m: &Model, at: [f32; 2]) -> [f32; 2] {
    let v = m.view(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    let k = v.placement.w / FRAME[0] as f32;
    [(at[0] - v.placement.x) / k, (at[1] - v.placement.y) / k]
}

#[test]
fn the_picture_pixel_under_the_pointer_stays_under_it() {
    for at in [[210.0, 170.0], [60.0, 60.0], [380.0, 280.0], [120.0, 240.0]] {
        let mut m = Model::default();
        for notch in 0..30 {
            let before = pixel_under(&m, at);
            m.wheel(Geometry::new(PANEL, FRAME, 0.0), at, 1.0);
            let after = pixel_under(&m, at);
            let v = m.view(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
            let k = v.placement.w / FRAME[0] as f32;
            // Rounding the placement to whole device pixels moves it by at most a device pixel or two. An axis on
            // which the picture is smaller than the Panel stays centred, and a point off the picture has no pixel
            // under it; neither is held.
            let held = |axis: usize, size: u32| {
                let on_picture_before = before[axis] >= 0.0 && before[axis] <= size as f32;
                // The picture's edge stopping at the Panel's edge also holds the spot where it is.
                let pinned = v.window[axis] == 0.0 || v.window[2 + axis] == 1.0;
                on_picture_before
                    && !pinned
                    && [v.placement.w, v.placement.h][axis] > PANEL[2 + axis]
            };
            for axis in 0..2 {
                if held(axis, FRAME[axis]) {
                    let off = (after[axis] - before[axis]).abs() * k;
                    assert!(
                        off <= 1.5,
                        "notch {notch} at {at:?} axis {axis}: {before:?} -> {after:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn zooming_toward_a_corner_never_leaves_the_panel_empty() {
    let mut m = Model::default();
    wheel_in(&mut m, [PANEL[0], PANEL[1]], 40);
    let v = m.view(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!(v.visible, PANEL, "the zoomed picture fills the Panel");
    assert!(v.placement.x <= PANEL[0] && v.placement.y <= PANEL[1]);
    assert!(v.placement.x + v.placement.w >= PANEL[0] + PANEL[2]);
    assert!(v.window[0] >= 0.0 && v.window[1] >= 0.0 && v.window[2] <= 1.0 && v.window[3] <= 1.0);
    assert!(v.nearest, "above 100 % the texel is exact");
}

#[test]
fn a_zoomed_view_shows_the_window_of_the_picture_it_covers_and_an_axis_smaller_than_the_panel_stays_centred()
 {
    let mut m = Model::default();
    let fit = m.view(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!(fit.window, [0.0, 0.0, 1.0, 1.0]);
    assert!(!fit.nearest);
    // 300 x 100 in 400 x 300 is Fit 133 %; at 200 % it is 600 x 200: wider than the Panel, centred in its height.
    m.set_percent(200.0, Geometry::new(PANEL, [300, 100], 0.0));
    let v = m.view(Geometry::new(PANEL, [300, 100], 0.0)).unwrap();
    assert_eq!((v.placement.w, v.placement.h), (600.0, 200.0));
    assert_eq!((v.visible[1], v.visible[3]), (v.placement.y, 200.0));
    assert_eq!(v.placement.y, 20.0 + 50.0, "centred in the Panel's height");
    assert_eq!((v.visible[0], v.visible[2]), (10.0, 400.0));
    assert_eq!((v.window[1], v.window[3]), (0.0, 1.0));
    assert!(
        (v.window[2] - v.window[0] - 400.0 / 600.0).abs() < 1e-4,
        "two thirds of the width is drawn"
    );
}

#[test]
fn the_slider_snaps_to_whole_steps_above_100_and_is_free_below() {
    let mut m = Model::default();
    let set = |m: &mut Model, p: f32| {
        m.set_percent(p, Geometry::new(PANEL, FRAME, 0.0));
        m.percent(Geometry::new(PANEL, FRAME, 0.0)).unwrap()
    };
    assert_eq!(set(&mut m, 60.0), 60.0);
    assert_eq!(set(&mut m, 100.0), 100.0);
    assert_eq!(set(&mut m, 140.0), 100.0);
    assert_eq!(set(&mut m, 150.0), 200.0);
    assert_eq!(set(&mut m, 260.0), 300.0);
    assert_eq!(set(&mut m, 399.0), 400.0);
    assert_eq!(set(&mut m, 760.0), 800.0);
    assert_eq!(set(&mut m, 1200.0), 800.0, "never beyond 800");
    let fit = {
        m.set_percent(0.0, Geometry::new(PANEL, FRAME, 0.0));
        assert_eq!(m.zoom, Zoom::Fit, "the bottom of the slider is Fit");
        percent(&m)
    };
    assert!(fit < 25.0);
}

#[test]
fn the_readout_says_fit_with_its_percent_or_the_whole_zoom() {
    let mut m = Model::default();
    let r = m.readout(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!(r.text, "Fit 21 %");
    assert_eq!((r.fit, r.max), (r.percent, 800.0));
    m.set_percent(200.0, Geometry::new(PANEL, FRAME, 0.0));
    let r = m.readout(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!((r.text.as_str(), r.percent), ("200 %", 200.0));
    assert!(r.fit < 25.0);
    assert!(
        Model::default()
            .readout(Geometry::new(PANEL, [0, 0], 0.0))
            .is_none()
    );
}

#[test]
fn a_frame_already_over_100_percent_at_fit_zooms_on_from_fit_in_whole_steps() {
    // 50 x 25 in 400 x 300: Fit is 800 %, the top of the range; a 160 x 90 frame is Fit 250 %, then 300 ... 800.
    let mut m = Model::default();
    let mut seen = Vec::new();
    for _ in 0..7 {
        m.wheel(Geometry::new(PANEL, [160, 90], 0.0), [200.0, 150.0], 1.0);
        seen.push(m.percent(Geometry::new(PANEL, [160, 90], 0.0)).unwrap());
    }
    assert_eq!(seen, [300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 800.0]);
    let mut big = Model::default();
    big.wheel(Geometry::new(PANEL, [50, 25], 0.0), [200.0, 150.0], 1.0);
    assert_eq!(
        big.zoom,
        Zoom::Fit,
        "nothing beyond Fit when Fit is already 800 % or more"
    );
}

// ---- pan (#166)

fn zoomed(percent: f32) -> Model {
    let mut m = Model::default();
    m.set_percent(percent, Geometry::new(PANEL, FRAME, 0.0));
    m
}

#[test]
fn pan_moves_the_picture_exactly_as_far_as_the_pointer() {
    let mut m = zoomed(200.0);
    let before = m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert!(m.pan(Geometry::new(PANEL, FRAME, 0.0), [-30.0, 12.0]));
    let after = m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!((after.x - before.x, after.y - before.y), (-30.0, 12.0));
    assert_eq!((after.w, after.h), (before.w, before.h));
}

#[test]
fn pan_clamps_so_the_picture_edge_stops_at_the_panel_edge() {
    let mut m = zoomed(200.0);
    m.pan(Geometry::new(PANEL, FRAME, 0.0), [1.0e6, 1.0e6]);
    let p = m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!(
        (p.x, p.y),
        (PANEL[0], PANEL[1]),
        "left and top edges at the Panel's"
    );
    m.pan(Geometry::new(PANEL, FRAME, 0.0), [-1.0e6, -1.0e6]);
    let p = m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap();
    assert_eq!(
        (p.x + p.w, p.y + p.h),
        (PANEL[0] + PANEL[2], PANEL[1] + PANEL[3]),
        "right and bottom edges at the Panel's"
    );
    assert!(
        !m.pan(Geometry::new(PANEL, FRAME, 0.0), [-5.0, -5.0]),
        "already at the edge: nothing moves"
    );
}

#[test]
fn a_picture_smaller_than_the_panel_stays_centred_and_fit_does_not_pan() {
    let mut m = Model::default();
    assert!(
        !m.pan(Geometry::new(PANEL, FRAME, 0.0), [40.0, 40.0]),
        "nothing to pan at Fit"
    );
    // 300 x 100 at 200 % is 600 x 200: wider than the Panel, centred in its height.
    let mut m = Model::default();
    m.set_percent(200.0, Geometry::new(PANEL, [300, 100], 0.0));
    m.pan(Geometry::new(PANEL, [300, 100], 0.0), [25.0, 60.0]);
    let p = m.placement(Geometry::new(PANEL, [300, 100], 0.0)).unwrap();
    assert_eq!(p.y, PANEL[1] + (PANEL[3] - p.h) / 2.0);
    assert!(p.x <= PANEL[0] && p.x + p.w >= PANEL[0] + PANEL[2]);
}

#[test]
fn zooming_after_a_pan_still_zooms_toward_the_pointer() {
    let mut m = zoomed(200.0);
    m.pan(Geometry::new(PANEL, FRAME, 0.0), [-80.0, -40.0]);
    let at = [150.0, 120.0];
    let (p, k) = (m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap(), 2.0);
    let under = [(at[0] - p.x) / k, (at[1] - p.y) / k];
    m.wheel(Geometry::new(PANEL, FRAME, 0.0), at, 1.0);
    let (q, k) = (m.placement(Geometry::new(PANEL, FRAME, 0.0)).unwrap(), 3.0);
    let now = [(at[0] - q.x) / k, (at[1] - q.y) / k];
    assert!(
        (now[0] - under[0]).abs() < 1.0 && (now[1] - under[1]).abs() < 1.0,
        "{under:?} vs {now:?}"
    );
}
