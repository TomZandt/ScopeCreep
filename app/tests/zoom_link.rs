//! Link at the zoom model seam (spec #163, ticket #168): picture Panels of different sizes share one zoom and one
//! centre point in picture coordinates. No GPU.
use scopecreep::display::Pointer;
use scopecreep::zoom::{Link, PanelZoom};

const BIG: [f32; 4] = [0.0, 0.0, 800.0, 450.0];
const SMALL: [f32; 4] = [0.0, 0.0, 320.0, 180.0];
const FRAME: [u32; 2] = [1600, 900];

fn pair() -> (PanelZoom, PanelZoom, Link) {
    let (a, b, link) = (PanelZoom::default(), PanelZoom::default(), Link::default());
    a.link_to(&link);
    b.link_to(&link);
    (a, b, link)
}

fn wheel(z: &PanelZoom, area: [f32; 4], at: [f32; 2], lines: f32) {
    z.pointer(area, Some(FRAME), 0.0, Pointer::Wheel { at, lines });
}

fn drag(z: &PanelZoom, area: [f32; 4], from: [f32; 2], to: [f32; 2]) {
    z.pointer(area, Some(FRAME), 0.0, Pointer::MiddleDown(from));
    z.pointer(area, Some(FRAME), 0.0, Pointer::Move(to));
    z.pointer(area, Some(FRAME), 0.0, Pointer::MiddleUp);
}

fn text(z: &PanelZoom, area: [f32; 4]) -> String {
    z.readout(area, Some(FRAME), 0.0).unwrap().text
}

/// The picture point at the middle of the Panel.
fn middle(z: &PanelZoom, area: [f32; 4]) -> [f32; 2] {
    let p = z.model().placement(area, FRAME, 0.0).unwrap();
    let k = p.w / FRAME[0] as f32;
    [(area[0] + area[2] / 2.0 - p.x) / k, (area[1] + area[3] / 2.0 - p.y) / k]
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1.0 && (a[1] - b[1]).abs() < 1.0
}

#[test]
fn link_is_off_until_turned_on_and_panels_zoom_alone() {
    let (a, b, link) = pair();
    assert!(!link.on());
    wheel(&a, BIG, [400.0, 225.0], 2.0);
    assert_eq!(text(&b, SMALL), "Fit 20 %");
    assert_ne!(text(&a, BIG), "Fit 50 %");
}

#[test]
fn turning_link_on_takes_the_clicked_panels_zoom_and_spot() {
    let (a, b, link) = pair();
    wheel(&a, BIG, [100.0, 60.0], 4.0);
    drag(&a, BIG, [400.0, 225.0], [250.0, 285.0]);
    let want = middle(&a, BIG);
    link.set(true, Some(a.model()));
    assert!(link.on());
    assert_eq!(text(&b, SMALL), text(&a, BIG));
    assert!(near(middle(&b, SMALL), want), "{:?} looks at {want:?}", middle(&b, SMALL));
}

#[test]
fn scroll_drag_and_the_slider_on_either_panel_move_both() {
    let (a, b, link) = pair();
    link.set(true, Some(a.model()));
    wheel(&b, SMALL, [160.0, 90.0], 9.0);
    assert_eq!(text(&a, BIG), text(&b, SMALL));
    assert_ne!(text(&a, BIG), "Fit 50 %");
    drag(&b, SMALL, [100.0, 100.0], [60.0, 90.0]);
    assert!(near(middle(&a, BIG), middle(&b, SMALL)), "same spot: {:?} {:?}", middle(&a, BIG), middle(&b, SMALL));
    a.set_percent(300.0, BIG, Some(FRAME), 0.0);
    assert_eq!(text(&b, SMALL), "300 %");
}

#[test]
fn at_fit_each_panel_fits_its_own_size() {
    let (a, b, link) = pair();
    link.set(true, Some(a.model()));
    assert_eq!((text(&a, BIG).as_str(), text(&b, SMALL).as_str()), ("Fit 50 %", "Fit 20 %"));
    wheel(&a, BIG, [400.0, 225.0], 4.0);
    wheel(&a, BIG, [400.0, 225.0], -10.0);
    assert_eq!((text(&a, BIG).as_str(), text(&b, SMALL).as_str()), ("Fit 50 %", "Fit 20 %"));
}

#[test]
fn turning_link_off_keeps_each_view_and_then_they_move_apart() {
    let (a, b, link) = pair();
    link.set(true, Some(a.model()));
    wheel(&a, BIG, [400.0, 225.0], 5.0);
    let (ta, tb) = (text(&a, BIG), text(&b, SMALL));
    let (ma, mb) = (middle(&a, BIG), middle(&b, SMALL));
    link.set(false, None);
    assert!(!link.on());
    assert_eq!((text(&a, BIG), text(&b, SMALL)), (ta, tb));
    assert_eq!((middle(&a, BIG), middle(&b, SMALL)), (ma, mb));
    wheel(&a, BIG, [400.0, 225.0], -100.0);
    assert_eq!(text(&a, BIG), "Fit 50 %");
    assert_ne!(text(&b, SMALL), "Fit 20 %");
}

#[test]
fn a_panel_whose_fit_is_above_the_shared_zoom_still_pans_and_steps_the_shared_view() {
    let (a, b, link) = pair();
    // The small Panel zooms to 31.25 %, below the big Panel's Fit (50 %).
    wheel(&b, SMALL, [160.0, 90.0], 2.0);
    link.set(true, Some(b.model()));
    assert_eq!(text(&a, BIG), "Fit 50 %");
    assert_eq!(text(&b, SMALL), "31 %");
    let before = middle(&b, SMALL);
    drag(&a, BIG, [400.0, 225.0], [420.0, 225.0]);
    assert!(middle(&b, SMALL)[0] < before[0] - 1.0, "the drag on the big Panel pans the shared view");
    // A notch out steps from the shared 31.25 %, not from the big Panel's Fit.
    wheel(&a, BIG, [400.0, 225.0], -1.0);
    assert_eq!(text(&b, SMALL), "25 %");
}
