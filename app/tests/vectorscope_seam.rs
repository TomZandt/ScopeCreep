//! The vectorscope Panels (#61, spec #49 stories 26 to 32), driven at the Feed seam: synthetic frames go in through
//! the synthetic writer, the app's Feed monitor hands them to the GPU exactly as the app does, and the tests look
//! at what each vectorscope measured, what it drew and its label. Expected positions come from the standard
//! BT.709 Cb/Cr constants (1.8556, 1.5748) and the research table's hue angles, never from the shader. Needs a GPU.
mod common;
use common::{Rig, TARGET, layouts, solid};
use scopecreep::colour_setup::{ColourSetup, KnownColourSetup, Levels, Provenance, Science};
use scopecreep::display::{DisplayContext, DisplaySet, ScopeHandles};
use scopecreep::panel_grid::PanelLayout;
use scopecreep::settings::PanelSettings;
use scopecreep::skin_tone::SKIN_TONE_LINE_DEGREES;
use scopecreep::vectorscope::{HIGHLIGHTS_FROM, PlotSquare, Plots, SHADOWS_BELOW, Tonal, VectorscopeCounts, VectorscopePlots, Zoom, halves};
use scopecreep_accuracy::colorchecker::{ColorChecker, TimelineSpace};
use scopecreep_accuracy::transfer::bt709_oetf;

const V: &str = "Vectorscope";

struct Bench {
    rig: Rig,
    set: DisplaySet,
    scopes: ScopeHandles,
    settings: PanelSettings,
    context: DisplayContext,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let (set, scopes) = DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
        let b = Bench {
            set,
            scopes,
            settings: PanelSettings::default(),
            context: DisplayContext::default(),
            rig,
        };
        b.reconfigure();
        b
    }

    fn reconfigure(&self) {
        self.set.configure(&self.context, &self.settings);
    }

    fn index(&self, name: &str) -> usize {
        self.set.names().iter().position(|n| *n == name).unwrap_or_else(|| panic!("no display called {name}"))
    }

    fn choose(&mut self, panel: &str, key: &str, value: &str) {
        self.settings.set(panel, key, value);
        self.reconfigure();
    }

    fn label(&self, name: &str) -> String {
        self.set.labels()[self.index(name)].clone()
    }

    fn plots(&self) -> VectorscopePlots {
        self.scopes.vectorscope.plots()
    }

    fn x1(&self) -> VectorscopeCounts {
        self.plots().x1
    }

    fn x2(&self) -> VectorscopeCounts {
        self.plots().x2
    }

    /// The square of the 100 % plot (`which` 0) or the 200 % plot (1) in a rendered Panel.
    fn square(&self, panels: &[PanelLayout], which: usize) -> PlotSquare {
        PlotSquare::new(halves(panels[self.index(V)].content)[which], 1.0)
    }

    fn play(&mut self, width: u32, height: u32, pixels: &[f32]) {
        let set = &self.set;
        self.rig.play(width, height, pixels, |rig| rig.poll(|encoder, frame| set.on_frame(encoder, frame)));
    }

    /// Draw every display into its Panel of a `width` × `height` target, as the window does, and read it back
    /// (RGBA8, rows top first).
    fn render(&self, width: u32, height: u32, scale: f32) -> (Vec<u8>, Vec<PanelLayout>) {
        let panels = layouts(self.set.len(), width, height, scale);
        let image = self.rig.render(width, height, |pass| {
            for (i, panel) in panels.iter().enumerate() {
                self.set.draw(pass, i, panel.content, [width as f32, height as f32], scale);
            }
        });
        (image, panels)
    }
}

/// BT.709 Cb and Cr from the standard's own constants.
fn cb_cr(rgb: [f64; 3]) -> [f64; 2] {
    let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
    [(rgb[2] - y) / 1.8556, (rgb[0] - y) / 1.5748]
}

/// Colour bars: red, yellow, green, cyan, magenta, blue at `level`, as ten-pixel stripes.
fn bars(level: f32) -> (u32, u32, Vec<f32>, [[f64; 3]; 6]) {
    let colours = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]].map(|c: [f64; 3]| c.map(|v| v * level as f64));
    let (w, h) = (60u32, 36u32);
    let pixels = (0..w * h).flat_map(|i| colours[(i % w / 10) as usize].map(|v| v as f32).into_iter().chain([1.0])).collect();
    (w, h, pixels, colours)
}

/// Where `render` put the pixel under `at`, as three bytes.
fn pixel(image: &[u8], width: u32, at: [f32; 2]) -> [u8; 3] {
    let i = (at[1] as usize * width as usize + at[0] as usize) * 4;
    [image[i], image[i + 1], image[i + 2]]
}

/// The brightest red value in the 3 x 3 pixels around `at`.
fn brightest_near(image: &[u8], width: u32, at: [f32; 2]) -> u8 {
    let mut best = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            best = best.max(pixel(image, width, [at[0] + dx as f32, at[1] + dy as f32])[0]);
        }
    }
    best
}

fn angle_gap(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn there_is_one_vectorscope_panel() {
    let b = Bench::new("order");
    assert_eq!(b.set.names().iter().filter(|n| n.starts_with("Vectorscope")).count(), 1);
    assert_eq!(b.set.names()[b.index(V)], V);
}

#[test]
fn rec709_bars_at_75_and_100_percent_land_on_their_targets() {
    let mut b = Bench::new("bars");
    for level in [0.75f32, 1.0] {
        let (w, h, pixels, colours) = bars(level);
        b.play(w, h, &pixels);
        let counts = b.x1();
        for colour in &colours {
            let [cb, cr] = cb_cr(*colour);
            assert_eq!(counts.near(cb, cr), 360, "{}% bars: the stripe {colour:?} lands at Cb {cb:.4}, Cr {cr:.4}", level * 100.0);
        }
        assert_eq!(counts.total(), 6 * 360, "every pixel is counted once");
    }
}

#[test]
fn the_100_percent_vectorscope_shows_the_whole_picture_and_the_200_percent_only_what_is_within_its_radius() {
    let mut b = Bench::new("range");
    let (w, h, pixels, _) = bars(1.0);
    b.play(w, h, &pixels);
    assert_eq!(b.x1().total(), 6 * 360, "at 100 % every full-strength bar is on the plot");
    assert_eq!(b.x1().clipped_total(), 0, "full-strength bars are not clipping");
    // At 200 % a full-strength bar is further out than the plot reaches: those pixels are off the plot, not wrapped,
    // and not clipping either: clipping does not depend on the zoom.
    assert!(b.x2().total() < 6 * 360);
    assert_eq!(b.x2().clipped_total(), 0, "and the 200 % Panel does not call them clipping");
}

#[test]
fn each_colorchecker_patch_lands_at_the_research_tables_hue_angle_within_one_degree() {
    let mut b = Bench::new("hue");
    // Rec.709 BT.709-encoded patches, as the research table's hue column defines them, shown as sent.
    let chart = ColorChecker::new(TimelineSpace::Rec709Gamma24);
    for patch in &chart.patches()[..18] {
        let rgb = patch.rec709_linear.map(|l| (l.signum() * bt709_oetf(l.abs())) as f32);
        b.play(64, 36, &solid(64, 36, rgb));
        let counts = b.x1();
        assert_eq!(counts.total(), 64 * 36, "{} is on the plot", patch.name);
        let (x, y) = counts.peak_bin();
        assert_eq!(counts.at(x, y), 64 * 36, "{} is one colour", patch.name);
        let [cb, cr] = counts.centre_of(x, y);
        let hue = cr.atan2(cb).to_degrees().rem_euclid(360.0);
        assert!(angle_gap(hue, patch.hue_degrees) <= 1.0, "{} lands at {hue:.2} degrees, the table says {:.2}", patch.name, patch.hue_degrees);
    }
}

#[test]
fn the_200_percent_panel_shows_the_same_data_at_twice_the_radius() {
    let mut b = Bench::new("twice");
    b.play(64, 36, &solid(64, 36, [0.5, 0.25, 0.375]));
    let from_middle = |counts: &VectorscopeCounts| {
        let (x, y) = counts.peak_bin();
        assert_eq!(counts.at(x, y), 64 * 36);
        [x as f64 + 0.5 - 512.0, y as f64 + 0.5 - 512.0]
    };
    let (one, two) = (from_middle(&b.x1()), from_middle(&b.x2()));
    for axis in 0..2 {
        assert!((two[axis] - 2.0 * one[axis]).abs() <= 1.0, "axis {axis}: {} bins from the middle at 100 %, {} at 200 %", one[axis], two[axis]);
    }
    assert!(one[0].abs() > 10.0 && one[1].abs() > 10.0, "a colour well off the middle: {one:?}");
}

fn black_frame_and_render(b: &mut Bench) -> (Vec<u8>, PlotSquare, PanelLayout) {
    b.play(64, 36, &solid(64, 36, [0.0; 3]));
    let (image, panels) = b.render(2000, 900, 1.0);
    let panel = panels[b.index(V)];
    (image, b.square(&panels, 0), panel)
}

#[test]
fn the_skin_tone_line_is_drawn_at_the_recorded_angle_and_can_be_turned_off() {
    assert_eq!(SKIN_TONE_LINE_DEGREES, 123.0, "the angle measured in Resolve 21.2 (#60), held in one shared constant");
    let mut b = Bench::new("skin");
    let (on, square, _) = black_frame_and_render(&mut b);
    let width = 2000;
    let [mx, my] = square.centre();
    let along = |degrees: f64, fraction: f32| {
        let r = square.ring_radius() * fraction;
        [mx + r * degrees.to_radians().cos() as f32, my - r * degrees.to_radians().sin() as f32]
    };
    let lit = |image: &[u8], degrees: f64| brightest_near(image, width, along(degrees, 0.7));
    let background = 9; // 0.035 of 255
    assert!(lit(&on, SKIN_TONE_LINE_DEGREES) > 60, "the skin-tone line is drawn: {}", lit(&on, SKIN_TONE_LINE_DEGREES));
    assert!(lit(&on, SKIN_TONE_LINE_DEGREES + 25.0) <= 40, "and only at its angle: {}", lit(&on, SKIN_TONE_LINE_DEGREES + 25.0));

    let panel = V;
    b.choose(panel, "skin-line", "off");
    let (off, _, _) = black_frame_and_render(&mut b);
    assert!(lit(&off, SKIN_TONE_LINE_DEGREES) <= 40, "turned off, nothing is drawn there: {}", lit(&off, SKIN_TONE_LINE_DEGREES));
    assert!(lit(&off, SKIN_TONE_LINE_DEGREES) >= background);
}

#[test]
fn the_colour_targets_are_drawn_where_the_chosen_bars_land() {
    let mut b = Bench::new("targets");
    let width = 2000;
    let (image, square, _) = black_frame_and_render(&mut b);
    // The top edge of the red target box (red at 75 %: Cb -0.0859, Cr 0.375; the box reaches 0.02 in Cb/Cr).
    let on_box = |image: &[u8], level: f64| {
        let [cb, cr] = cb_cr([level, 0.0, 0.0]);
        let [x, y] = square.position(cb, cr + 0.02, Zoom::X1);
        brightest_near(image, width, [x, y])
    };
    assert!(on_box(&image, 0.75) > 60, "a target box at the 75 % red position: {}", on_box(&image, 0.75));
    assert!(on_box(&image, 1.0) <= 40, "not at the 100 % red position: {}", on_box(&image, 1.0));

    let panel = V;
    b.choose(panel, "targets", "100");
    let (image, _, _) = black_frame_and_render(&mut b);
    assert!(on_box(&image, 1.0) > 60, "100 % targets: a box at the 100 % red position: {}", on_box(&image, 1.0));
    assert!(on_box(&image, 0.75) <= 40, "and no longer at the 75 % position: {}", on_box(&image, 0.75));
}

#[test]
fn the_trace_is_tinted_by_the_colour_that_put_it_there_and_the_middle_is_white() {
    let mut b = Bench::new("tint");
    let width = 2000;
    let cases: [([f32; 3], usize); 4] = [([0.75, 0.0, 0.0], 0), ([0.0, 0.75, 0.0], 1), ([0.0, 0.0, 0.75], 2), ([0.75, 0.0, 0.75], 3)];
    for (rgb, which) in cases {
        b.play(64, 36, &solid(64, 36, rgb));
        let (image, panels) = b.render(width, 900, 1.0);
        let square = b.square(&panels, 0);
        let [cb, cr] = cb_cr([rgb[0] as f64, rgb[1] as f64, rgb[2] as f64]);
        let [x, y] = square.position(cb, cr, Zoom::X1);
        let p = pixel(&image, width, [x, y]);
        let (hi, lo) = (p.iter().copied().max().unwrap(), p.iter().copied().min().unwrap());
        assert!(hi > 120, "the {rgb:?} trace is lit: {p:?}");
        if which < 3 {
            assert_eq!(p[which], hi, "the {rgb:?} trace is tinted towards its own colour: {p:?}");
        } else {
            assert!(p[0] > p[1] && p[2] > p[1], "magenta is red and blue, not green: {p:?}");
        }
        assert!(hi as i32 - lo as i32 > 60, "and clearly tinted, not grey: {p:?}");
    }
    // Grey pixels land in the middle: white, not tinted.
    b.play(64, 36, &solid(64, 36, [0.5, 0.5, 0.5]));
    let (image, panels) = b.render(width, 900, 1.0);
    let square = b.square(&panels, 0);
    let [x, y] = square.position(0.0, 0.0, Zoom::X1);
    let mut lit = [0u8; 3];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let p = pixel(&image, width, [x + dx as f32, y + dy as f32]);
            if p[0] as u32 + p[1] as u32 + p[2] as u32 > lit.iter().map(|v| *v as u32).sum::<u32>() { lit = p; }
        }
    }
    assert!(lit[0] > 150 && (lit[0] as i32 - lit[2] as i32).abs() < 40, "the middle is white: {lit:?}");
}

#[test]
fn the_colour_target_boxes_carry_the_colour_they_stand_for() {
    let mut b = Bench::new("target colours");
    let width = 2000;
    let (image, square, _) = black_frame_and_render(&mut b);
    // The top edge of each box, in the order red, yellow, green, cyan, magenta, blue at 75 %.
    let colours: [[f64; 3]; 6] = [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
    for bar in colours {
        let [cb, cr] = cb_cr(bar.map(|v| v * 0.75));
        let [x, y] = square.position(cb, cr + 0.02, Zoom::X1);
        let mut best = [0u8; 3];
        for dy in -1..=1 {
            for dx in -1..=1 {
                let p = pixel(&image, width, [x + dx as f32, y + dy as f32]);
                if p.iter().map(|v| *v as u32).sum::<u32>() > best.iter().map(|v| *v as u32).sum::<u32>() { best = p; }
            }
        }
        for c in 0..3 {
            if bar[c] > 0.5 { assert!(best[c] > 100, "the {bar:?} box has {c} lit: {best:?}"); } else { assert!(best[c] < 40, "the {bar:?} box has {c} dark: {best:?}"); }
        }
    }
}

#[test]
fn colours_beyond_the_100_percent_bars_mark_the_ring_at_their_hue_at_either_zoom_and_the_zoom_alone_never_does() {
    let mut b = Bench::new("clipping");
    let width = 2000;
    let ring_pixel = |image: &[u8], square: &PlotSquare, degrees: f64| {
        let [cx, cy] = square.centre();
        let (s, c) = degrees.to_radians().sin_cos();
        let ring = square.ring_radius();
        // The outermost lit pixel across the ring's thickness there: a clipping mark is thicker than the ring.
        let mut thick = 0;
        for k in -8..=8 {
            let (x, y) = (cx + (ring + k as f32) * c as f32, cy - (ring + k as f32) * s as f32);
            if pixel(image, width, [x, y]).iter().map(|v| *v as u32).sum::<u32>() > 250 {
                thick += 1;
            }
        }
        thick
    };
    // Red at twice full strength is beyond the 100 % bars (Cb -0.23, Cr 1.0: 102.9 degrees).
    b.play(64, 36, &solid(64, 36, [2.0, 0.0, 0.0]));
    for (which, name) in [(0, "100 %"), (1, "200 %")] {
        let counts = if which == 0 { b.x1() } else { b.x2() };
        assert!(counts.clipped_total() > 0, "{name}: clipping is counted");
        let degrees = counts.clipped_peak_degrees().expect("a peak");
        assert!((degrees - 102.9).abs() < 1.0, "{name}: at red's hue: {degrees}");
        let (image, panels) = b.render(width, 900, 1.0);
        let square = b.square(&panels, which);
        assert!(ring_pixel(&image, &square, 102.9) >= 5, "{name}: the ring is thick and lit at red's hue: {}", ring_pixel(&image, &square, 102.9));
        assert!(ring_pixel(&image, &square, 282.9) <= 3, "{name}: and plain on the opposite side: {}", ring_pixel(&image, &square, 282.9));
    }
    // Full-strength colours (even the green and magenta bars that reach furthest) are not clipping at either zoom.
    let (w, h, pixels, _) = bars(1.0);
    b.play(w, h, &pixels);
    assert_eq!(b.x1().clipped_total(), 0);
    assert_eq!(b.x2().clipped_total(), 0);
}

#[test]
fn the_outer_ring_is_a_hue_wheel_in_the_colour_that_lands_at_each_point() {
    let mut b = Bench::new("ring");
    let width = 2000;
    let (image, square, _) = black_frame_and_render(&mut b);
    let ring = square.radius() * (scopecreep::vectorscope::RING / scopecreep::vectorscope::HALF_EXTENT) as f32;
    let at = |degrees: f64| {
        let [cx, cy] = square.centre();
        let (s, c) = degrees.to_radians().sin_cos();
        let (x, y) = (cx + ring * c as f32, cy - ring * s as f32);
        let mut best = [0u8; 3];
        for dy in -2..=2 {
            for dx in -2..=2 {
                let p = pixel(&image, width, [x + dx as f32, y + dy as f32]);
                if p.iter().map(|v| *v as u32).sum::<u32>() > best.iter().map(|v| *v as u32).sum::<u32>() { best = p; }
            }
        }
        best
    };
    // Blue is at about -9 degrees (right), red at about 104, green at about 241 and yellow at about 167.
    let blue = at(-9.0);
    assert!(blue[2] > 120 && blue[0] < 90, "blue where blue lands: {blue:?}");
    let red = at(104.0);
    assert!(red[0] > 120 && red[1] < 90 && red[2] < 90, "red where red lands: {red:?}");
    let green = at(241.0);
    assert!(green[1] > 120 && green[0] < 90 && green[2] < 90, "green where green lands: {green:?}");
    let yellow = at(167.0);
    assert!(yellow[0] > 120 && yellow[1] > 120 && yellow[2] < 90, "yellow where yellow lands: {yellow:?}");
    assert!(blue.iter().max().unwrap() < &200, "muted, never brighter than the trace: {blue:?}");
}

#[test]
fn the_trace_glows_where_it_is_dense_and_a_stray_pixel_is_faint_and_the_soft_spread_can_be_turned_off() {
    let mut b = Bench::new("glow");
    let width = 2000;
    // A dense blob of mid grey-blue pixels with one stray pixel far from it (and from any target or line).
    let (w, h) = (1920u32, 1080u32);
    let mut pixels = solid(w, h, [0.4, 0.45, 0.6]);
    pixels[0] = 0.5;
    pixels[1] = 0.1;
    pixels[2] = 0.3;
    let render = |b: &mut Bench| {
        b.play(w, h, &pixels);
        let (image, panels) = b.render(width, 900, 1.0);
        (image, b.square(&panels, 0))
    };
    let (image, square) = render(&mut b);
    let level = |image: &[u8], at: [f32; 2]| pixel(image, width, at).iter().copied().max().unwrap();
    let [cb, cr] = cb_cr([0.4, 0.45, 0.6]);
    let dense = level(&image, square.position(cb, cr, Zoom::X1));
    let [cb, cr] = cb_cr([0.5, 0.1, 0.3]);
    let stray_at = square.position(cb, cr, Zoom::X1);
    let stray = brightest_any(&image, width, stray_at);
    assert!(dense > 150, "the dense colour glows: {dense}");
    assert!(stray > 15 && stray < dense / 2, "a stray pixel is visible but faint: {stray} against {dense}");
    // The soft spread: a pixel next to the stray one is lit with it on and dark with it off.
    let next = [stray_at[0] + 1.0, stray_at[1]];
    let with = level(&image, next);
    let panel = V;
    b.choose(panel, "spread", "off");
    let (image, square) = render(&mut b);
    let _ = square;
    let without = level(&image, next);
    assert!(with > without + 5 || without <= 20, "spread on lights the neighbour: {with} against {without}");
}

/// The brightest channel in the 5 x 5 pixels around `at`.
fn brightest_any(image: &[u8], width: u32, at: [f32; 2]) -> u8 {
    let mut best = 0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            best = best.max(pixel(image, width, [at[0] + dx as f32, at[1] + dy as f32]).iter().copied().max().unwrap());
        }
    }
    best
}

#[test]
fn the_trace_brightness_setting_makes_a_sparse_colour_brighter_or_dimmer() {
    let mut b = Bench::new("brightness");
    let width = 2000;
    // One red pixel in an otherwise black frame.
    let mut pixels = solid(64, 36, [0.0; 3]);
    pixels[0] = 0.75;
    let [cb, cr] = cb_cr([0.75, 0.0, 0.0]);
    let mut seen = Vec::new();
    for choice in ["dim", "normal", "bright"] {
        let panel = V;
        b.choose(panel, "brightness", choice);
        b.play(64, 36, &pixels);
        let (image, panels) = b.render(width, 900, 1.0);
        let square = b.square(&panels, 0);
        seen.push(brightest_near(&image, width, square.position(cb, cr, Zoom::X1)));
    }
    assert!(seen[0] < seen[1] && seen[1] <= seen[2], "dimmer, normal, brighter: {seen:?}");
}

#[test]
fn the_label_names_the_measured_space_the_weights_the_targets_and_the_angle() {
    let mut b = Bench::new("label");
    let assumed = b.label(V);
    assert!(assumed.starts_with("Vectorscope 100 % and 200 %  ·  Cb/Cr"), "one label names both zooms: {assumed}");
    assert!(assumed.contains("Rec.709 assumed, measured as sent, unconverted"), "{assumed}");
    assert!(assumed.contains("Y = 0.2126 R + 0.7152 G + 0.0722 B"), "{assumed}");
    assert!(assumed.contains("75 % targets"), "{assumed}");
    assert!(assumed.contains("123° (as Resolve 21.2 draws it)"), "{assumed}");

    let setup = ColourSetup::new(Some(Science::Yrgb), "P3-D65".into(), "P3-D65".into(), None, None, None, Some(Levels::Video));
    b.context = DisplayContext { colour_setup: Some(KnownColourSetup { setup, provenance: Provenance::Project }), colour_label: "P3-D65".into(), ..DisplayContext::default() };
    let panel = V;
    b.choose(panel, "targets", "100");
    b.choose(panel, "skin-line", "off");
    let p3 = b.label(V);
    assert!(p3.contains("P3-D65") && !p3.contains("assumed"), "{p3}");
    assert!(p3.contains("Y = 0.2290 R + 0.6917 G + 0.0793 B"), "the same weights as the Luma waveform for this space: {p3}");
    assert!(p3.contains("100 % targets"), "{p3}");
    assert!(!p3.contains("skin-tone"), "no angle is stated when the line is off: {p3}");
}

#[test]
fn the_menu_offers_trace_brightness_targets_and_the_skin_tone_line_with_sensible_defaults() {
    let b = Bench::new("menu");
    let defs = b.set.setting_defs(b.index(V));
    let keys: Vec<_> = defs.iter().map(|d| (d.key, d.choices.iter().map(|c| c.value).collect::<Vec<_>>())).collect();
    assert!(defs[0].amount.is_some() && defs[1].amount.is_some(), "brightness and spread are sliders");
    assert!(defs[0].amount.is_some() && defs[1].amount.is_some(), "brightness and spread are sliders");
    assert_eq!(keys, [("brightness", vec![]), ("spread", vec![]), ("targets", vec!["75", "100"]), ("skin-line", vec!["on", "off"]), ("plots", vec!["both", "100", "200"]), ("tonal", vec!["all", "shadows", "mids", "highlights"]), ("mono", vec!["off", "on"])]);
}

#[test]
fn the_two_plots_sit_side_by_side_each_square_and_centred_in_its_half() {
    for (width, height) in [(900.0f32, 300.0f32), (400.0, 400.0), (200.0, 500.0), (1001.0, 333.0)] {
        let area = [20.0, 30.0, width, height];
        let [left, right] = halves(area);
        assert_eq!(left[0], area[0], "100 % on the left");
        assert_eq!(left[0] + left[2], right[0], "200 % right beside it");
        assert_eq!(right[0] + right[2], area[0] + width, "and together they fill the Panel");
        for half in [left, right] {
            let plot = PlotSquare::new(half, 1.0);
            assert_eq!(plot.plot[2], plot.plot[3], "square");
            let [cx, cy] = plot.centre();
            assert!((cx - (half[0] + half[2] / 2.0)).abs() <= 1.0 && (cy - (half[1] + half[3] / 2.0)).abs() <= 1.0, "centred in its half: {half:?} {plot:?}");
            assert!(plot.plot[0] >= half[0] && plot.plot[0] + plot.plot[2] <= half[0] + half[2], "inside its half");
        }
    }
}

#[test]
fn one_setting_changes_both_plots_and_the_targets_move_on_the_100_percent_plot() {
    let mut b = Bench::new("both");
    let width = 2000;
    // One red pixel in an otherwise black frame, near enough the middle to be on both plots.
    let mut pixels = solid(64, 36, [0.0; 3]);
    pixels[..3].copy_from_slice(&[0.25, 0.0, 0.0]);
    let [cb, cr] = cb_cr([0.25, 0.0, 0.0]);
    let mut seen = Vec::new();
    for choice in ["dim", "bright"] {
        b.choose(V, "brightness", choice);
        b.play(64, 36, &pixels);
        let (image, panels) = b.render(width, 900, 1.0);
        seen.push([(0, Zoom::X1), (1, Zoom::X2)].map(|(which, zoom)| brightest_near(&image, width, b.square(&panels, which).position(cb, cr, zoom))));
    }
    assert!(seen[0][0] < seen[1][0] && seen[0][1] < seen[1][1], "brighter on both plots: {seen:?}");
    // The colour targets (75 % red at Cb -0.086, Cr 0.375) are on the 100 % plot; one choice moves them.
    b.play(64, 36, &solid(64, 36, [0.0; 3]));
    let box_top = |b: &Bench, level: f64| {
        let (image, panels) = b.render(width, 900, 1.0);
        let [cb, cr] = cb_cr([level, 0.0, 0.0]);
        let [x, y] = b.square(&panels, 0).position(cb, cr + 0.02, Zoom::X1);
        brightest_near(&image, width, [x, y])
    };
    assert!(box_top(&b, 0.75) > 60);
    b.choose(V, "targets", "100");
    assert!(box_top(&b, 0.75) <= 40 && box_top(&b, 1.0) > 60);
}

#[test]
fn the_rings_and_the_skin_tone_line_are_in_the_same_place_on_both_plots_and_the_setting_turns_both_off() {
    let mut b = Bench::new("graticule");
    let width = 2000;
    let along = |b: &Bench, which: usize, image: &[u8], panels: &[PanelLayout]| {
        let square = b.square(panels, which);
        let [mx, my] = square.centre();
        let r = square.ring_radius() * 0.7;
        let (s, c) = SKIN_TONE_LINE_DEGREES.to_radians().sin_cos();
        brightest_near(image, width, [mx + r * c as f32, my - r * s as f32])
    };
    b.play(64, 36, &solid(64, 36, [0.0; 3]));
    let (image, panels) = b.render(width, 900, 1.0);
    assert!(along(&b, 0, &image, &panels) > 60 && along(&b, 1, &image, &panels) > 60, "the skin-tone line is on both plots");
    b.choose(V, "skin-line", "off");
    let (image, panels) = b.render(width, 900, 1.0);
    assert!(along(&b, 0, &image, &panels) <= 40 && along(&b, 1, &image, &panels) <= 40, "and off on both");
}

#[test]
fn old_vectorscope_settings_carry_over_to_the_one_panel_and_the_200_percent_ones_are_ignored() {
    let saved = PanelSettings::from_text("Vectorscope 100 %\ttargets\t100\nVectorscope 100 %\tskin-line\toff\nVectorscope 200 %\ttargets\t75\nVectorscope 200 %\tbrightness\tbright\n");
    let mut b = Bench::new("legacy");
    b.settings = saved;
    b.reconfigure();
    let label = b.label(V);
    assert!(label.contains("100 % targets") && !label.contains("skin-tone"), "the 100 % Panel's choices apply: {label}");
    let resolved = b.set.settings_of(b.index(V), &b.settings);
    assert_eq!(resolved["brightness"], "100", "the 200 % Panel's are not used");
}

// ------------------------------------------------------------------ the title bar's chips (#123)

/// The brightest pixel (by sum of channels) in the 3 x 3 pixels around `at`.
fn lit_near(image: &[u8], width: u32, at: [f32; 2]) -> [u8; 3] {
    let mut best = [0u8; 3];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let p = pixel(image, width, [at[0] + dx as f32, at[1] + dy as f32]);
            if p.iter().map(|&v| v as u32).sum::<u32>() > best.iter().map(|&v| v as u32).sum::<u32>() {
                best = p;
            }
        }
    }
    best
}

#[test]
fn mono_draws_the_same_counts_in_grey_and_colour_is_as_before() {
    let mut b = Bench::new("mono");
    let width = 2000;
    // A saturated red patch: strong colour on the plot.
    b.play(64, 36, &solid(64, 36, [0.5, 0.1, 0.1]));
    let [cb, cr] = cb_cr([0.5, 0.1, 0.1]);
    let at = |b: &Bench| {
        let (image, panels) = b.render(width, 900, 1.0);
        lit_near(&image, width, b.square(&panels, 0).position(cb, cr, Zoom::X1))
    };
    let colour = at(&b);
    assert!(colour[0] as i32 - colour[2] as i32 > 40, "the colour trace is red: {colour:?}");
    let counts = b.plots();
    b.choose(V, "mono", "on");
    let mono = at(&b);
    assert!(mono.iter().all(|&v| (v as i32 - mono[0] as i32).abs() <= 2) && mono[0] > 60, "the mono trace is grey and lit: {mono:?}");
    assert_eq!(b.plots(), counts, "the same counts, only the colour differs");
    b.choose(V, "mono", "off");
    assert_eq!(at(&b), colour, "colour as before");
}

#[test]
fn the_plots_chip_shows_one_plot_filling_the_panel_or_both() {
    let area = [10.0, 20.0, 900.0, 400.0];
    assert_eq!(Plots::from_setting(None), Plots::Both);
    assert_eq!(Plots::from_setting(Some("garbage")), Plots::Both);
    assert_eq!(Plots::Both.areas(area), vec![(Zoom::X1, halves(area)[0]), (Zoom::X2, halves(area)[1])]);
    assert_eq!(Plots::from_setting(Some("100")).areas(area), vec![(Zoom::X1, area)]);
    assert_eq!(Plots::from_setting(Some("200")).areas(area), vec![(Zoom::X2, area)]);

    let mut b = Bench::new("plots");
    let width = 2000;
    let mut pixels = solid(64, 36, [0.0; 3]);
    pixels[..3].copy_from_slice(&[0.25, 0.0, 0.0]);
    let [cb, cr] = cb_cr([0.25, 0.0, 0.0]);
    b.play(64, 36, &pixels);
    // Only 200 %: the plot fills the Panel, so the red pixel sits on a bigger square than in the half.
    b.choose(V, "plots", "200");
    let (image, panels) = b.render(width, 900, 1.0);
    let whole = PlotSquare::new(panels[b.index(V)].content, 1.0);
    assert!(whole.plot[2] > b.square(&panels, 1).plot[2], "one plot is bigger than a half");
    assert!(brightest_near(&image, width, whole.position(cb, cr, Zoom::X2)) > 60, "the 200 % plot is drawn, filling the Panel");
    assert_eq!(b.plots().x1.total(), b.plots().x2.total(), "both are still measured");
    assert!(b.label(V).starts_with("Vectorscope 200 %  ·"), "{}", b.label(V));
    // Only 100 %: the 100 % plot's target boxes are there, the 200 % plot is not (its own centre mark is the same, so look at the trace).
    b.choose(V, "plots", "100");
    let (image, panels) = b.render(width, 900, 1.0);
    let whole = PlotSquare::new(panels[b.index(V)].content, 1.0);
    assert!(brightest_near(&image, width, whole.position(cb, cr, Zoom::X1)) > 60);
    // Back to both: the halves again.
    b.choose(V, "plots", "both");
    let (image, panels) = b.render(width, 900, 1.0);
    assert!(brightest_near(&image, width, b.square(&panels, 1).position(cb, cr, Zoom::X2)) > 60);
    assert!(b.label(V).starts_with("Vectorscope 100 % and 200 %"));
}

// ------------------------------------------------------------------ the tonal range chip (#124)

/// A frame in three bands of rows, shadows, mids and highlights, each with a plain colour on its left half and an
/// overdriven one (beyond the ring, so clipping) on its right half.
fn tonal_frame() -> (u32, u32, Vec<f32>, [[f64; 3]; 3]) {
    let plain = [[0.2, 0.0, 0.0], [0.0, 0.6, 0.0], [1.0, 0.9, 0.5]];
    let over = [[1.3, 0.0, 0.0], [0.0, 0.5, 1.9], [1.4, 1.4, 0.0]];
    let (w, h) = (64u32, 36u32);
    let pixels = (0..w * h)
        .flat_map(|i| {
            let (col, band) = (i % w, (i / w / 12) as usize);
            let c = if col < w / 2 { plain[band] } else { over[band] };
            [c[0] as f32, c[1] as f32, c[2] as f32, 1.0]
        })
        .collect();
    (w, h, pixels, plain)
}

fn luma(rgb: [f64; 3]) -> f64 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

#[test]
fn the_tonal_ranges_are_thirds_by_resolves_boundaries_and_cover_every_luma_once() {
    assert_eq!((SHADOWS_BELOW, HIGHLIGHTS_FROM), (0.30, 0.70));
    for y in [f64::MIN, -0.1, 0.0, 0.2999, 0.30, 0.5, 0.6999, 0.70, 1.0, 1.3] {
        let hits = [Tonal::Shadows, Tonal::Mids, Tonal::Highlights].iter().filter(|t| t.includes(y)).count();
        assert_eq!(hits, 1, "luma {y} is in exactly one range");
        assert!(Tonal::All.includes(y));
    }
    assert!(Tonal::Shadows.includes(0.2999) && Tonal::Mids.includes(0.30) && Tonal::Mids.includes(0.6999) && Tonal::Highlights.includes(0.70));
    assert!(![Tonal::Shadows, Tonal::Mids, Tonal::Highlights].iter().any(|t| t.includes(f64::NAN)), "a luma that is not a number is in no range");
    assert_eq!(Tonal::from_setting(None), Tonal::All);
    assert_eq!(Tonal::from_setting(Some("garbage")), Tonal::All);
}

#[test]
fn each_tonal_chip_plots_only_its_ranges_colours_and_clipping_counts_only_those_pixels() {
    let mut b = Bench::new("tonal");
    let (w, h, pixels, plain) = tonal_frame();
    for (rgb, range) in plain.iter().zip([Tonal::Shadows, Tonal::Mids, Tonal::Highlights]) {
        assert!(range.includes(luma(*rgb)), "the test colour {rgb:?} is in {range:?}");
    }
    b.play(w, h, &pixels);
    let all = b.x1();
    let band = (w * 12) as u64;
    let near = |c: &VectorscopeCounts, rgb: [f64; 3]| {
        let [cb, cr] = cb_cr(rgb);
        c.near(cb, cr)
    };
    for rgb in plain {
        assert!(near(&all, rgb) > 0, "All plots {rgb:?}");
    }
    assert_eq!(all.clipped_total(), 3 * band / 2, "All counts every overdriven pixel");

    for (value, index) in [("shadows", 0usize), ("mids", 1), ("highlights", 2)] {
        b.choose(V, "tonal", value);
        b.play(w, h, &pixels); // a tonal range changes what is measured, so the Feed measures again
        let (x1, x2) = (b.x1(), b.x2());
        for (i, rgb) in plain.iter().enumerate() {
            let (n1, n2) = (near(&x1, *rgb), near(&x2, *rgb));
            if i == index {
                assert_eq!(n1 as u64, band / 2, "{value}: its own colour is plotted, every pixel of it");
                assert_eq!(n2 as u64, band / 2, "{value}: at 200 % too");
            } else {
                assert_eq!((n1, n2), (0, 0), "{value}: the colour {rgb:?} of another range is not plotted");
            }
        }
        assert_eq!(x1.clipped_total(), band / 2, "{value}: clipping counts only this range's pixels");
        assert_eq!(x2.clipped_total(), band / 2);
        assert!(b.label(V).contains(&format!("{value} only")), "{}", b.label(V));
    }
    b.choose(V, "tonal", "all");
    b.play(w, h, &pixels);
    assert_eq!(b.x1(), all, "back to All is the same measurement");
    assert!(!b.label(V).contains("only:"), "{}", b.label(V));
}

#[test]
fn brightness_and_spread_are_sliders_whose_old_list_values_and_defaults_draw_as_before() {
    use scopecreep::vectorscope::{BRIGHTNESS_AMOUNT, SPREAD_AMOUNT};
    // Defaults are today's look: Normal and On.
    assert_eq!((BRIGHTNESS_AMOUNT.default, SPREAD_AMOUNT.default), (100.0, 1.0));
    // Old list values map to the numbers that drew the same; junk is the default; out of range is clamped.
    assert_eq!((BRIGHTNESS_AMOUNT.read(Some("normal")), BRIGHTNESS_AMOUNT.read(Some("bright")), BRIGHTNESS_AMOUNT.read(Some("dim"))), (100.0, 400.0, 25.0));
    assert_eq!((SPREAD_AMOUNT.read(Some("on")), SPREAD_AMOUNT.read(Some("off")), SPREAD_AMOUNT.read(Some("x")), SPREAD_AMOUNT.read(Some("9"))), (1.0, 0.0, 1.0, 3.0));
    assert_eq!(SPREAD_AMOUNT.text(1.0), "1.0 px");

    // At the Feed seam: the old values and their numbers draw the same pixels; the label does not change.
    let (w, h) = (1920u32, 1080u32);
    let mut pixels = solid(w, h, [0.4, 0.45, 0.6]);
    for px in pixels[..3 * 2000].chunks_mut(3) {
        px.copy_from_slice(&[0.5, 0.1, 0.3]);
    }
    let draw = |setting: &str, value: &str| {
        let mut b = Bench::new("slider");
        if !setting.is_empty() {
            b.choose(V, setting, value);
        }
        b.play(w, h, &pixels);
        (b.render(1600, 800, 1.0).0, b.label(V))
    };
    let (normal, label) = draw("", "");
    for (setting, old, new) in [("brightness", "normal", "100"), ("brightness", "bright", "400"), ("brightness", "dim", "25"), ("spread", "on", "1"), ("spread", "off", "0")] {
        assert!(draw(setting, old).0 == draw(setting, new).0, "{setting} {old} draws as {new}");
    }
    assert!(draw("brightness", "bright").0 != normal, "brighter draws differently");
    assert!(draw("brightness", "dim").0 != normal, "dimmer draws differently");
    assert!(draw("spread", "off").0 != normal, "spread 0 draws differently");
    assert_eq!(draw("brightness", "250").1, label);
    assert_eq!(draw("spread", "2.5").1, label);
    assert!(draw("spread", "3").0 != draw("spread", "1").0);
}
