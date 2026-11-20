//! The Picture stage (#110, spec #106): what every Picture View gets from it, with a plain test rule so nothing
//! here depends on a View's colour. Frames go in through the synthetic Feed writer (bottom row first, as the Sender
//! writes them) or are made by hand (top row first), the stage draws a Panel and the pixels are read back.
//! Placement (fit, centre, pixel rounding, scale) is also tested as the plain function every View shares. Needs a GPU.
mod common;
use common::{Rig, TARGET};
use scopecreep::gpu_frame::{GpuFrame, new_texture_id};
use scopecreep::picture_stage::{Legend, PictureStage, Placement, Rule, place};

const W: u32 = 120;
const H: u32 = 100;
const BLACK: [u8; 3] = [0, 0, 0];
const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const GREEN: [u8; 3] = [0, 255, 0];
const WHITE: [u8; 3] = [255, 255, 255];

/// The test rule: the exact texel times a tint, and a green legend.
const RULE: &str = r#"
struct Values { tint: vec4<f32> };
fn colour(uv: vec2<f32>) -> vec3<f32> { return load_picture(uv) * values.tint.xyz; }
fn legend(uv: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0, 1.0, 0.0); }
"#;

fn stage(rig: &Rig) -> PictureStage {
    PictureStage::new::<[f32; 4]>(
        &rig.device,
        &rig.queue,
        TARGET,
        false,
        Rule {
            label: "test",
            wgsl: RULE.into(),
            has_legend: true,
        },
    )
}

/// What the Feed monitor hands the stage when Resolve plays `pixels` (bottom row first).
fn play(rig: &mut Rig, stage: &PictureStage, width: u32, height: u32, pixels: &[f32]) {
    rig.write(width, height, pixels);
    for _ in 0..50 {
        rig.now += common::MS;
        rig.poll(|encoder, frame| stage.show(encoder, frame));
    }
}

/// A frame made by hand, the first row the top of the picture.
fn top_down(rig: &Rig, width: u32, height: u32, pixels: &[f32]) -> GpuFrame {
    let texture = rig.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    rig.queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(pixels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 16),
            rows_per_image: None,
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    GpuFrame {
        texture,
        width,
        height,
        bottom_up: false,
        texture_id: new_texture_id(),
    }
}

fn px(image: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * W as usize + x as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// Every pixel of the rectangle (left, top, width, height) is `colour`.
fn filled(image: &[u8], rect: [u32; 4], colour: [u8; 3]) -> bool {
    (rect[1]..rect[1] + rect[3])
        .all(|y| (rect[0]..rect[0] + rect[2]).all(|x| px(image, x, y) == colour))
}

fn solid(width: u32, height: u32, rgb: [f32; 3]) -> Vec<f32> {
    (0..width * height)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 1.0])
        .collect()
}

const ID: [f32; 4] = [1.0; 4];

// ---- placement: fit, centre, rounding, scale

#[test]
fn a_wide_frame_fits_the_width_and_is_centred_vertically() {
    assert_eq!(
        place([0.0, 0.0, 192.0, 200.0], [1920, 1080], 0.0),
        Some(Placement {
            x: 0.0,
            y: 46.0,
            w: 192.0,
            h: 108.0
        })
    );
}

#[test]
fn a_tall_frame_fits_the_height_and_is_centred_horizontally() {
    assert_eq!(
        place([0.0, 0.0, 300.0, 100.0], [1080, 1920], 0.0),
        Some(Placement {
            x: 122.0,
            y: 0.0,
            w: 56.0,
            h: 100.0
        })
    );
}

#[test]
fn the_panel_offset_is_kept_and_the_edges_land_on_whole_device_pixels() {
    let p = place([10.5, 20.25, 333.0, 187.0], [1920, 1080], 0.0).unwrap();
    for v in [p.x, p.y, p.w, p.h] {
        assert_eq!(v, v.round());
    }
    assert!(p.x >= 10.0 && p.x + p.w <= 343.5 + 0.5 && p.y >= 20.0 && p.y + p.h <= 207.25 + 0.5);
}

#[test]
fn the_same_shape_at_twice_the_scale_is_twice_the_size() {
    let one = place([0.0, 0.0, 300.0, 200.0], [1920, 1080], 0.0).unwrap();
    let two = place([0.0, 0.0, 600.0, 400.0], [1920, 1080], 0.0).unwrap();
    assert_eq!((two.w, two.h), (one.w * 2.0, one.h * 2.0));
}

#[test]
fn the_legend_strip_is_kept_free_under_the_picture() {
    let p = place([0.0, 0.0, 200.0, 100.0], [100, 100], 26.0).unwrap();
    assert_eq!(
        p,
        Placement {
            x: 63.0,
            y: 0.0,
            w: 74.0,
            h: 74.0
        }
    );
}

#[test]
fn nothing_is_placed_in_a_panel_with_no_room() {
    assert_eq!(place([0.0, 0.0, 0.5, 100.0], [100, 100], 0.0), None);
    assert_eq!(place([0.0, 0.0, 100.0, 20.0], [100, 100], 20.0), None);
}

// ---- drawing

#[test]
fn a_frame_is_drawn_fitted_and_centred_with_black_around_it() {
    let mut rig = Rig::new("stage-fit");
    let stage = stage(&rig);
    play(&mut rig, &stage, 4, 2, &solid(4, 2, [1.0, 1.0, 1.0]));
    // The Panel is the whole target: a 4 x 2 frame fits the width, 120 x 60, centred.
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, W as f32, H as f32],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&image, [0, 20, 120, 60], WHITE));
    assert!(filled(&image, [0, 0, 120, 20], BLACK));
    assert!(filled(&image, [0, 80, 120, 20], BLACK));
}

#[test]
fn a_panel_away_from_the_corner_is_placed_inside_its_own_area() {
    let mut rig = Rig::new("stage-offset");
    let stage = stage(&rig);
    play(&mut rig, &stage, 2, 2, &solid(2, 2, [1.0, 1.0, 1.0]));
    // A 100 x 60 Panel at (10, 30): a square frame fits the height, 60 x 60, centred at x 30..90.
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [10.0, 30.0, 100.0, 60.0],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&image, [30, 30, 60, 60], WHITE));
    assert!(filled(&image, [10, 30, 20, 60], BLACK));
    assert!(filled(&image, [90, 30, 20, 60], BLACK));
    assert!(filled(&image, [0, 0, 120, 30], BLACK));
    // A Panel as tall as the target: the square picture is 100 x 100, centred.
    let big = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 120.0, 100.0],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&big, [10, 0, 100, 100], WHITE));
}

#[test]
fn a_bottom_up_feed_frame_is_drawn_the_right_way_up() {
    let mut rig = Rig::new("stage-bottom-up");
    let stage = stage(&rig);
    // The Feed carries the bottom row first: red is the bottom row, blue the top.
    let mut pixels = solid(2, 1, [1.0, 0.0, 0.0]);
    pixels.extend(solid(2, 1, [0.0, 0.0, 1.0]));
    play(&mut rig, &stage, 2, 2, &pixels);
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 100.0, 100.0],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&image, [0, 0, 100, 50], BLUE));
    assert!(filled(&image, [0, 50, 100, 50], RED));
}

#[test]
fn a_top_down_frame_is_drawn_as_it_is() {
    let rig = Rig::new("stage-top-down");
    let stage = stage(&rig);
    // The first row is the top: red on top, blue under it.
    let mut pixels = solid(2, 1, [1.0, 0.0, 0.0]);
    pixels.extend(solid(2, 1, [0.0, 0.0, 1.0]));
    stage.show_now(&top_down(&rig, 2, 2, &pixels));
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 100.0, 100.0],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&image, [0, 0, 100, 50], RED));
    assert!(filled(&image, [0, 50, 100, 50], BLUE));
}

#[test]
fn the_rules_values_reach_the_shader() {
    let mut rig = Rig::new("stage-values");
    let stage = stage(&rig);
    play(&mut rig, &stage, 2, 2, &solid(2, 2, [1.0, 1.0, 1.0]));
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 100.0, 100.0],
            [W as f32, H as f32],
            &[0.0f32, 0.0, 1.0, 1.0],
            None,
        )
    });
    assert!(filled(&image, [0, 0, 100, 100], BLUE));
}

#[test]
fn the_legend_bar_is_drawn_under_a_picture_that_leaves_room_for_it() {
    let mut rig = Rig::new("stage-legend");
    let stage = stage(&rig);
    play(&mut rig, &stage, 2, 2, &solid(2, 2, [1.0, 1.0, 1.0]));
    // A 100 x 100 Panel with a 30 pixel strip whose 10 pixel bar starts 5 pixels down: the square picture is 70 x 70
    // centred above, the bar spans the Panel's width at y 75..85.
    let legend = Legend {
        strip: 30.0,
        bar_offset: 5.0,
        bar: 10.0,
    };
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 100.0, 100.0],
            [W as f32, H as f32],
            &ID,
            Some(legend),
        )
    });
    assert!(filled(&image, [15, 0, 70, 70], WHITE));
    assert!(filled(&image, [0, 75, 100, 10], GREEN));
    assert!(
        filled(&image, [0, 70, 100, 5], BLACK),
        "between the picture and the bar"
    );
    assert!(filled(&image, [0, 85, 100, 15], BLACK), "under the bar");
}

#[test]
fn fill_covers_the_whole_panel_whatever_the_frames_shape() {
    let mut rig = Rig::new("stage-fill");
    let stage = stage(&rig);
    play(&mut rig, &stage, 4, 2, &solid(4, 2, [1.0, 1.0, 1.0]));
    let image = rig.render(W, H, |pass| {
        stage.fill(pass, [10.0, 10.0, 80.0, 80.0], [W as f32, H as f32], &ID)
    });
    assert!(filled(&image, [10, 10, 80, 80], WHITE));
    assert!(filled(&image, [0, 0, 120, 10], BLACK));
}

#[test]
fn nothing_is_drawn_before_a_frame_arrives_and_the_size_follows_the_frame() {
    let mut rig = Rig::new("stage-empty");
    let stage = stage(&rig);
    assert_eq!(stage.size(), None);
    let image = rig.render(W, H, |pass| {
        stage.draw(
            pass,
            [0.0, 0.0, 100.0, 100.0],
            [W as f32, H as f32],
            &ID,
            None,
        )
    });
    assert!(filled(&image, [0, 0, 120, 100], BLACK));
    play(&mut rig, &stage, 4, 2, &solid(4, 2, [1.0, 1.0, 1.0]));
    assert_eq!(stage.size(), Some([4, 2]));
    play(&mut rig, &stage, 6, 3, &solid(6, 3, [1.0, 1.0, 1.0]));
    assert_eq!(stage.size(), Some([6, 3]));
}

// ---- zoom (#165, spec #163)

use scopecreep::zoom::{Geometry, Zoom, ZoomModel};

/// The rule of a Picture View that reads the frame the smoothed way, so zoom must make it exact.
const SMOOTH_RULE: &str = r#"
struct Values { tint: vec4<f32> };
fn colour(uv: vec2<f32>) -> vec3<f32> { return sample_picture(uv); }
fn legend(uv: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0, 1.0, 0.0); }
"#;

/// A rule that paints red wherever the stage lets it, to see the mattes and the strip.
const RED_RULE: &str = r#"
struct Values { tint: vec4<f32> };
fn colour(uv: vec2<f32>) -> vec3<f32> { return vec3<f32>(1.0, 0.0, 0.0); }
fn legend(uv: vec2<f32>) -> vec3<f32> { return vec3<f32>(0.0, 1.0, 0.0); }
"#;

fn stage_with(rig: &Rig, wgsl: &str) -> PictureStage {
    PictureStage::new::<[f32; 4]>(
        &rig.device,
        &rig.queue,
        TARGET,
        false,
        Rule {
            label: "test",
            wgsl: wgsl.into(),
            has_legend: true,
        },
    )
}

/// Lengths of the runs of equal values, less the first and last (which the Panel's edge cuts short).
fn runs(values: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut n = 1;
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            n += 1;
        } else {
            out.push(n);
            n = 1;
        }
    }
    out.push(n);
    out[1..out.len() - 1].to_vec()
}

fn zoomed(percent: f32) -> ZoomModel {
    let mut m = ZoomModel::default();
    m.set_percent(
        percent,
        Geometry::new([0.0, 0.0, W as f32, H as f32], [160, 100], 0.0),
    );
    assert_eq!(m.zoom, Zoom::Percent(percent));
    m
}

#[test]
fn zoomed_in_every_picture_pixel_is_an_exact_square_of_2_3_and_4_device_pixels() {
    let mut rig = Rig::new("stage-zoom-squares");
    let stage = stage_with(&rig, SMOOTH_RULE);
    // Neighbouring pixels all differ: the red code moves by 3 along a row and 5 down a column.
    let pixels: Vec<f32> = (0..100)
        .flat_map(|y| {
            (0..160)
                .flat_map(move |x| [(((3 * x + 5 * y) % 200 + 20) as f32) / 255.0, 0.0, 0.0, 1.0])
        })
        .collect();
    play(&mut rig, &stage, 160, 100, &pixels);
    for k in [2u32, 3, 4] {
        let model = zoomed(k as f32 * 100.0);
        let image = rig.render(W, H, |pass| {
            stage.draw_zoomed(
                pass,
                [0.0, 0.0, W as f32, H as f32],
                [W as f32, H as f32],
                &ID,
                None,
                &model,
            )
        });
        let row: Vec<u8> = (0..W).map(|x| px(&image, x, H / 2)[0]).collect();
        let column: Vec<u8> = (0..H).map(|y| px(&image, W / 2, y)[0]).collect();
        for (name, values) in [("row", row), ("column", column)] {
            let r = runs(&values);
            assert!(
                r.len() > 5 && r.iter().all(|n| *n == k as usize),
                "{k}00 % {name}: runs {r:?}"
            );
        }
    }
}

#[test]
fn at_fit_the_frame_is_drawn_exactly_as_it_always_was() {
    let mut rig = Rig::new("stage-zoom-fit");
    let stage = stage_with(&rig, SMOOTH_RULE);
    play(
        &mut rig,
        &stage,
        160,
        100,
        &solid(160, 100, [0.0, 0.5, 1.0]),
    );
    let area = [0.0, 0.0, W as f32, H as f32];
    let plain = rig.render(W, H, |pass| {
        stage.draw(pass, area, [W as f32, H as f32], &ID, None)
    });
    let fit = rig.render(W, H, |pass| {
        stage.draw_zoomed(
            pass,
            area,
            [W as f32, H as f32],
            &ID,
            None,
            &ZoomModel::default(),
        )
    });
    assert_eq!(plain, fit);
    // 120 x 75 centred in 120 x 100: black above and below.
    assert!(filled(&plain, [0, 0, 120, 12], BLACK) && px(&plain, 60, 50) != BLACK);
}

#[test]
fn zoomed_in_the_mattes_stay_black_and_the_legend_strip_is_untouched() {
    let mut rig = Rig::new("stage-zoom-mattes");
    let stage = stage_with(&rig, RED_RULE);
    // 160 x 100 with the left 20 columns black bars.
    let pixels: Vec<f32> = (0..100)
        .flat_map(|_| {
            (0..160).flat_map(|x| {
                if x < 20 {
                    [0.0, 0.0, 0.0, 1.0]
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                }
            })
        })
        .collect();
    play(&mut rig, &stage, 160, 100, &pixels);
    let legend = Legend {
        strip: 20.0,
        bar_offset: 5.0,
        bar: 10.0,
    };
    let area = [0.0, 0.0, 120.0, 100.0];
    let mut model = ZoomModel::default();
    model.set_percent(200.0, Geometry::new(area, [160, 100], 20.0));
    model.centre = Some([30.0, 50.0]);
    let image = rig.render(W, H, |pass| {
        stage.draw_zoomed(pass, area, [W as f32, H as f32], &ID, Some(legend), &model)
    });
    // The picture shows its first 60 columns: 20 black picture columns are 40 black device columns.
    assert!(
        filled(&image, [0, 0, 40, 80], BLACK),
        "the matte stays black"
    );
    assert!(
        filled(&image, [40, 0, 80, 80], RED),
        "the rest is the rule's colour, up to the strip"
    );
    assert!(
        filled(&image, [0, 80, 120, 5], BLACK) && filled(&image, [0, 95, 120, 5], BLACK),
        "nothing is drawn in the strip but the bar"
    );
    assert!(
        filled(&image, [0, 85, 120, 10], GREEN),
        "the legend bar is where it always is"
    );
}
