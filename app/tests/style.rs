//! The style tokens (spec #113): neutral by role, and the type scale a short ordered list.
use scopecreep::style::*;

#[test]
fn every_chrome_colour_is_a_neutral_grey() {
    for c in [
        SURFACE_WINDOW,
        SURFACE_MENU,
        LINE_BORDER,
        STATE_HOVER,
        STATE_ON,
    ] {
        assert!(c[0] == c[1] && c[1] == c[2], "{c:?} tints the eye");
    }
    for c in [TEXT_PRIMARY, TEXT_SECONDARY, TEXT_MUTED] {
        assert!(c[0] == c[1] && c[1] == c[2], "{c:?} tints the eye");
    }
}

#[test]
fn text_steps_down_and_the_picture_stays_the_brightest_thing() {
    assert!(TEXT_PRIMARY[0] > TEXT_SECONDARY[0] && TEXT_SECONDARY[0] > TEXT_MUTED[0]);
    assert!(
        SURFACE_WINDOW[0] < SURFACE_MENU[0]
            && SURFACE_MENU[0] < STATE_HOVER[0]
            && STATE_HOVER[0] < LINE_BORDER[0]
    );
    // Chrome never reaches white.
    assert!(STATE_ON[0] < 0.9 && TEXT_HIGH[0] < 240 && TEXT_PRIMARY[0] <= 200);
}

#[test]
fn the_type_scale_is_three_sizes_largest_first() {
    assert!(TYPE_NOTICE > TYPE_LABEL && TYPE_LABEL > TYPE_SCALE);
}

#[test]
fn the_tokens_keep_the_values_the_window_already_used() {
    // The menu frame, fill and the window's grey are as ever; the sizes are the Hairline look's (#226, ADR 0044).
    assert_eq!(rgba(LINE_BORDER), [0.30, 0.30, 0.30, 1.0]);
    assert_eq!(rgba(SURFACE_MENU), [0.11, 0.11, 0.11, 1.0]);
    assert_eq!(clear(SURFACE_WINDOW).r, 0.071f32 as f64);
    assert_eq!((TYPE_LABEL, TYPE_SCALE, TYPE_NOTICE), (13.0, 12.0, 15.0));
    assert_eq!(
        (SPACE_MARGIN, SPACE_MENU_ROW, SPACE_STATUS_ROW),
        (8.0, 30.0, 26.0)
    );
}
