//! Synthetic pictures for the test harness: RGBA 32-bit float, tightly packed, bottom row first (as the Sender
//! writes them).

/// Values a careless copy would damage: negative, over-range, denormal, tiny, huge, and every pixel different.
pub fn awkward_values(width: u32, height: u32) -> Vec<f32> {
    let specials = [-0.5f32, 7.25, 1e-40, 1.0, -0.0, 65504.0, f32::MIN_POSITIVE, 0.18];
    (0..width as usize * height as usize * 4)
        .map(|i| if i < specials.len() { specials[i] } else { (i as f32 * 0.618_034).fract() * 2.0 - 0.5 })
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
    let mut out = vec![0.0f32; w * h * 4];
    for row in 0..h {
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
    out
}
