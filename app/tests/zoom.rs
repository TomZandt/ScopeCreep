//! The zoom model at its pure seam (spec #163, ticket #164): where a picture goes in its Panel. Fit is the only
//! zoom so far, and it must place exactly as the fit-only placement always did. No GPU.
use scopecreep::picture_stage::place;
use scopecreep::zoom::ZoomModel;

const SHAPES: [[u32; 2]; 5] = [[1920, 1080], [1080, 1920], [3840, 2160], [64, 36], [1, 1]];
const AREAS: [[f32; 4]; 5] = [[0.0, 0.0, 192.0, 200.0], [0.0, 0.0, 300.0, 100.0], [10.5, 20.25, 333.0, 187.0], [12.0, 12.0, 1256.0, 716.0], [0.0, 0.0, 0.5, 90.0]];

#[test]
fn at_fit_every_frame_and_panel_places_as_before() {
    let model = ZoomModel::default();
    for picture in SHAPES {
        for area in AREAS {
            for strip in [0.0, 40.0, 95.0] {
                assert_eq!(model.placement(area, picture, strip), place(area, picture, strip), "{picture:?} in {area:?} strip {strip}");
            }
        }
    }
}

#[test]
fn a_frame_over_100_percent_at_fit_stays_at_fit_and_says_so() {
    let model = ZoomModel::default();
    let p = model.placement([0.0, 0.0, 1000.0, 1000.0], [100, 50], 0.0).unwrap();
    assert_eq!((p.w, p.h), (1000.0, 500.0));
    assert_eq!(model.fit_percent([0.0, 0.0, 1000.0, 1000.0], [100, 50], 0.0), Some(1000));
}

#[test]
fn fit_percent_is_how_much_the_frame_is_scaled() {
    let model = ZoomModel::default();
    assert_eq!(model.fit_percent([0.0, 0.0, 442.0, 400.0], [1920, 1080], 0.0), Some(23));
    // The strip kept for a legend takes room from the fit.
    assert_eq!(model.fit_percent([0.0, 0.0, 1920.0, 1180.0], [1920, 1080], 100.0), Some(100));
    assert_eq!(model.fit_percent([0.0, 0.0, 0.5, 90.0], [1920, 1080], 0.0), None);
    assert_eq!(model.fit_percent([0.0, 0.0, 100.0, 100.0], [0, 0], 0.0), None);
}
