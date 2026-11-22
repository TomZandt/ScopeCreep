//! Link at the zoom model seam (spec #163, ticket #168): picture Panels of different sizes share one zoom and one
//! centre point in picture coordinates. No GPU.
use scopecreep::display::Pointer;
use scopecreep::zoom::{Geometry, Link, PanelZoom};

const BIG: [f32; 4] = [0.0, 0.0, 800.0, 450.0];
const SMALL: [f32; 4] = [0.0, 0.0, 320.0, 180.0];
const FRAME: [u32; 2] = [1600, 900];

/// Two Panels on a Link that is switched off (a new Link opens on, ADR 0035).
fn pair() -> (PanelZoom, PanelZoom, Link) {
    let (a, b, link) = (PanelZoom::default(), PanelZoom::default(), Link::default());
    link.set(false, None);
    a.link_to(&link);
    b.link_to(&link);
    (a, b, link)
}

fn wheel(z: &PanelZoom, area: [f32; 4], at: [f32; 2], lines: f32) {
    z.pointer(
        Some(Geometry::new(area, FRAME, 0.0)),
        Pointer::Wheel { at, lines },
    );
}

fn drag(z: &PanelZoom, area: [f32; 4], from: [f32; 2], to: [f32; 2]) {
    z.pointer(
        Some(Geometry::new(area, FRAME, 0.0)),
        Pointer::MiddleDown(from),
    );
    z.pointer(Some(Geometry::new(area, FRAME, 0.0)), Pointer::Move(to));
    z.pointer(Some(Geometry::new(area, FRAME, 0.0)), Pointer::MiddleUp);
}

fn text(z: &PanelZoom, area: [f32; 4]) -> String {
    z.readout(Some(Geometry::new(area, FRAME, 0.0)))
        .unwrap()
        .text
}

/// The picture point at the middle of the Panel.
fn middle(z: &PanelZoom, area: [f32; 4]) -> [f32; 2] {
    let p = z
        .model()
        .placement(Geometry::new(area, FRAME, 0.0))
        .unwrap();
    let k = p.w / FRAME[0] as f32;
    [
        (area[0] + area[2] / 2.0 - p.x) / k,
        (area[1] + area[3] / 2.0 - p.y) / k,
    ]
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1.0 && (a[1] - b[1]).abs() < 1.0
}

#[test]
fn a_new_link_opens_on_and_with_it_off_panels_zoom_alone() {
    assert!(Link::default().on(), "Link opens on");
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
    assert!(
        near(middle(&b, SMALL), want),
        "{:?} looks at {want:?}",
        middle(&b, SMALL)
    );
}

#[test]
fn scroll_drag_and_the_slider_on_either_panel_move_both() {
    let (a, b, link) = pair();
    link.set(true, Some(a.model()));
    wheel(&b, SMALL, [160.0, 90.0], 9.0);
    assert_eq!(text(&a, BIG), text(&b, SMALL));
    assert_ne!(text(&a, BIG), "Fit 50 %");
    drag(&b, SMALL, [100.0, 100.0], [60.0, 90.0]);
    assert!(
        near(middle(&a, BIG), middle(&b, SMALL)),
        "same spot: {:?} {:?}",
        middle(&a, BIG),
        middle(&b, SMALL)
    );
    a.set_percent(300.0, Some(Geometry::new(BIG, FRAME, 0.0)));
    assert_eq!(text(&b, SMALL), "300 %");
}

#[test]
fn at_fit_each_panel_fits_its_own_size() {
    let (a, b, link) = pair();
    link.set(true, Some(a.model()));
    assert_eq!(
        (text(&a, BIG).as_str(), text(&b, SMALL).as_str()),
        ("Fit 50 %", "Fit 20 %")
    );
    wheel(&a, BIG, [400.0, 225.0], 4.0);
    wheel(&a, BIG, [400.0, 225.0], -10.0);
    assert_eq!(
        (text(&a, BIG).as_str(), text(&b, SMALL).as_str()),
        ("Fit 50 %", "Fit 20 %")
    );
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
    assert!(
        middle(&b, SMALL)[0] < before[0] - 1.0,
        "the drag on the big Panel pans the shared view"
    );
    // A notch out steps from the shared 31.25 %, not from the big Panel's Fit.
    wheel(&a, BIG, [400.0, 225.0], -1.0);
    assert_eq!(text(&b, SMALL), "25 %");
}

// ---- The Fit toggle (Z, #169)

fn g(area: [f32; 4]) -> Option<Geometry> {
    Some(Geometry::new(area, FRAME, 0.0))
}

#[test]
fn z_goes_to_fit_and_back_to_the_last_zoom_and_spot() {
    let z = PanelZoom::default();
    wheel(&z, BIG, [600.0, 100.0], 3.0);
    drag(&z, BIG, [400.0, 200.0], [350.0, 230.0]);
    let (zoomed, spot) = (text(&z, BIG), middle(&z, BIG));
    assert!(!zoomed.starts_with("Fit"));
    assert!(z.toggle_fit(g(BIG)));
    assert!(text(&z, BIG).starts_with("Fit"));
    assert!(z.toggle_fit(g(BIG)));
    assert_eq!(text(&z, BIG), zoomed);
    assert_eq!(middle(&z, BIG), spot, "the same spot, not the middle");
}

#[test]
fn z_at_fit_with_no_last_zoom_does_nothing() {
    let z = PanelZoom::default();
    assert!(!z.toggle_fit(g(BIG)));
    assert!(text(&z, BIG).starts_with("Fit"));
    // Z to Fit and back uses the last zoom up; at Fit by the chip there is then nothing to go back to.
    wheel(&z, BIG, [400.0, 225.0], 2.0);
    z.toggle_fit(g(BIG));
    z.toggle_fit(g(BIG));
    z.set_percent(0.0, g(BIG));
    assert!(text(&z, BIG).starts_with("Fit"));
    assert!(!z.toggle_fit(g(BIG)), "nothing left to go back to");
}

#[test]
fn z_before_a_frame_has_arrived_does_nothing() {
    assert!(!PanelZoom::default().toggle_fit(None));
}

#[test]
fn with_link_on_z_on_one_panel_moves_every_panel_and_back() {
    let (a, b, link) = pair();
    link.set(true, None);
    wheel(&a, BIG, [400.0, 225.0], 3.0);
    let (before, spot) = (text(&b, SMALL), middle(&b, SMALL));
    assert!(!before.starts_with("Fit"));
    assert!(a.toggle_fit(g(BIG)));
    assert!(text(&a, BIG).starts_with("Fit") && text(&b, SMALL).starts_with("Fit"));
    assert!(
        b.toggle_fit(g(SMALL)),
        "Z on the other Panel goes back for both"
    );
    assert_eq!(text(&b, SMALL), before);
    assert_eq!(middle(&b, SMALL), spot);
}

#[test]
fn with_link_off_z_moves_only_its_own_panel() {
    let (a, b, link) = pair();
    link.set(false, None);
    wheel(&a, BIG, [400.0, 225.0], 3.0);
    wheel(&b, SMALL, [160.0, 90.0], 3.0);
    let other = text(&b, SMALL);
    a.toggle_fit(g(BIG));
    assert!(text(&a, BIG).starts_with("Fit"));
    assert_eq!(text(&b, SMALL), other);
}
