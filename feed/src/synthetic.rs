//! Synthetic pictures for the test harness: RGBA 32-bit float, tightly packed, bottom row first (as the Sender
//! writes them).

/// Values a careless copy would damage: negative, over-range, denormal, tiny, huge, and every pixel different.
pub fn awkward_values(width: u32, height: u32) -> Vec<f32> {
    let specials = [
        -0.5f32,
        7.25,
        1e-40,
        1.0,
        -0.0,
        65504.0,
        f32::MIN_POSITIVE,
        0.18,
    ];
    (0..width as usize * height as usize * 4)
        .map(|i| {
            if i < specials.len() {
                specials[i]
            } else {
                (i as f32 * 0.618_034).fract() * 2.0 - 0.5
            }
        })
        .collect()
}

/// One colour everywhere.
pub fn solid(width: u32, height: u32, rgba: [f32; 4]) -> Vec<f32> {
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for _ in 0..width as usize * height as usize {
        out.extend_from_slice(&rgba);
    }
    out
}

/// A test card that is easy to recognise in a screenshot: eight colour bars over a black-to-white ramp, with a
/// white marker that moves one step to the right every frame and a brighter top-left corner block (so a flipped
/// or mirrored picture is obvious).
pub fn test_card(width: u32, height: u32, frame: u64) -> Vec<f32> {
    let mut out = vec![0.0f32; width as usize * height as usize * 4];
    draw_test_card_rows(&mut out, width, height, frame, 0..height as usize);
    out
}

/// A [`test_card`] with values out of range along the bottom: its last twelfth (at least one row) is a grey ramp
/// from −0.25 at the left to 1.25 at the right, so the left sixth of the columns is below 0 and the right sixth
/// above 1 (what `feed-synth --out-of-range` plays). The marker never moves through it.
pub fn test_card_out_of_range(width: u32, height: u32, frame: u64) -> Vec<f32> {
    let mut out = test_card(width, height, frame);
    let w = width as usize;
    let strip = (height as usize / 12).max(1).min(height as usize);
    for row in 0..strip {
        // Rows are stored bottom first: the strip is the first rows in memory.
        for x in 0..w {
            let v = -0.25 + 1.5 * x as f32 / (w - 1).max(1) as f32;
            let i = (row * w + x) * 4;
            out[i..i + 3].copy_from_slice(&[v, v, v]);
        }
    }
    out
}

/// Move the marker of a [`test_card`] already in `pixels` to where it is on `frame`, redrawing only the band the
/// marker runs along (a twelfth of the picture): cheap enough to play a UHD test card at 60 fps.
pub fn move_test_card_marker(pixels: &mut [f32], width: u32, height: u32, frame: u64) {
    let h = height as usize;
    let band_from_top = h * 2 / 3 - h / 12..h * 2 / 3;
    draw_test_card_rows(
        pixels,
        width,
        height,
        frame,
        h - band_from_top.end..h - band_from_top.start,
    );
}

fn draw_test_card_rows(
    out: &mut [f32],
    width: u32,
    height: u32,
    frame: u64,
    rows: std::ops::Range<usize>,
) {
    const BARS: [[f32; 3]; 8] = [
        [0.75, 0.75, 0.75],
        [0.75, 0.75, 0.0],
        [0.0, 0.75, 0.75],
        [0.0, 0.75, 0.0],
        [0.75, 0.0, 0.75],
        [0.75, 0.0, 0.0],
        [0.0, 0.0, 0.75],
        [0.05, 0.05, 0.05],
    ];
    let (w, h) = (width as usize, height as usize);
    let marker_w = (w / 60).max(1);
    let marker_x = (frame as usize * marker_w) % w;
    for row in rows {
        let from_top = h - 1 - row; // rows are stored bottom first
        for x in 0..w {
            let mut rgb = if from_top < h * 2 / 3 {
                BARS[x * 8 / w]
            } else {
                let v = x as f32 / (w - 1).max(1) as f32;
                [v, v, v]
            };
            if from_top < h / 12 && x < w / 12 {
                rgb = [1.0, 1.0, 1.0];
            }
            let in_marker = x >= marker_x && x < marker_x + marker_w;
            if in_marker && from_top >= h * 2 / 3 - h / 12 && from_top < h * 2 / 3 {
                rgb = [1.0, 1.0, 1.0];
            }
            let i = (row * w + x) * 4;
            out[i..i + 3].copy_from_slice(&rgb);
            out[i + 3] = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn moving_the_marker_gives_the_same_card_as_drawing_it_afresh() {
        let mut card = super::test_card(120, 72, 0);
        for frame in [1, 7, 59, 60, 61] {
            super::move_test_card_marker(&mut card, 120, 72, frame);
            assert!(card == super::test_card(120, 72, frame), "frame {frame}");
        }
    }

    /// The lowest and highest red, green or blue in `pixels` (alpha left out).
    fn extremes(pixels: &[f32]) -> (f32, f32) {
        pixels
            .chunks(4)
            .flat_map(|p| &p[..3])
            .fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)))
    }

    #[test]
    fn the_plain_test_card_stays_in_range() {
        assert_eq!(extremes(&super::test_card(120, 72, 3)), (0.0, 1.0));
    }

    #[test]
    fn every_frame_feed_synth_writes_with_out_of_range_holds_values_below_0_and_above_1() {
        // As feed-synth plays it: the card drawn once, then only the marker moved each frame.
        let (w, h) = (1920, 1080);
        let mut card = super::test_card_out_of_range(w, h, 0);
        for frame in [0, 1, 59, 60, 61, 500] {
            super::move_test_card_marker(&mut card, w, h, frame);
            let (lo, hi) = extremes(&card);
            assert_eq!((lo, hi), (-0.25, 1.25), "frame {frame}");
            // A sixth of the columns on each side, in every row of the bottom twelfth.
            let row = |r: usize| &card[r * w as usize * 4..(r + 1) * w as usize * 4];
            for r in [0, h as usize / 12 - 1] {
                let below = row(r).chunks(4).filter(|p| p[1] < 0.0).count();
                let above = row(r).chunks(4).filter(|p| p[1] > 1.0).count();
                assert!(
                    (319..=321).contains(&below) && (319..=321).contains(&above),
                    "frame {frame}, row {r}: {below} below 0, {above} above 1"
                );
            }
            // Above the strip the card is the plain one.
            let strip = (h as usize / 12) * w as usize * 4;
            assert!(
                card[strip..] == super::test_card(w, h, frame)[strip..],
                "frame {frame}"
            );
        }
    }

    #[test]
    fn a_tiny_card_still_gets_its_out_of_range_row() {
        assert_eq!(
            extremes(&super::test_card_out_of_range(4, 3, 0)),
            (-0.25, 1.25)
        );
    }
}
