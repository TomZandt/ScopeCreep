//! The luma waveform's out-of-range edge rows on HiDPI (#32), driven at the Feed seam: the frame
//! `feed-synth --out-of-range` plays goes in through the Feed, the app's Feed monitor hands it to the standard
//! displays exactly as the app does, and the waveform Panel is drawn offscreen as the Window draws it (display,
//! title bar text and chips, the scale's numbers, the border) at scale 1.0, 1.5 and 2.0. No window opens. Needs a
//! GPU.
//!
//! With `SCOPECREEP_CAPTURE=<folder>` each drawn Panel is also written there as
//! `out-of-range-waveform-<scale>x.png` (ImageMagick's `convert` makes the PNG), for a person to judge the text.
//! The text is set as `main.rs` sets it (same faces, colours and places), but by this file, not by `main.rs`.
mod common;
use common::{Rig, TARGET};
use glyphon::{
    Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use scopecreep::colour_setup::Levels;
use scopecreep::components::{self, Align, Face, Prim, State};
use scopecreep::display::{DisplayContext, DisplaySet, ScopeHandles};
use scopecreep::flat_rects::FlatRects;
use scopecreep::panel_grid::PanelLayout;
use scopecreep::settings::PanelSettings;
use scopecreep::style;
use scopecreep::waveform::{NAME, PlotLayout};
use scopecreep::window_text;
use scopecreep_feed::synthetic;

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
/// The waveform Panel's cell in logical pixels: about what it gets in the default 1440 × 840 Window.
const CELL: [f32; 2] = [720.0, 432.0];

/// A piece of text as the Window sets it: words, left, top, width, face, colour, alignment, and the rectangle it
/// is clipped to.
type Words = (String, f32, f32, f32, Face, [u8; 3], Align, [f32; 4]);

struct Bench {
    rig: Rig,
    set: DisplaySet,
    scopes: ScopeHandles,
    settings: PanelSettings,
    index: usize,
}

impl Bench {
    fn new(name: &str) -> Bench {
        let rig = Rig::new(name);
        let (set, scopes) =
            DisplaySet::standard_with_handles(&rig.device, &rig.queue, TARGET, false);
        let settings = PanelSettings::default();
        // feed-synth sends no Colour Setup: the displays are configured as the app configures them then.
        set.configure(&DisplayContext::default(), &settings);
        let index = set.names().iter().position(|n| *n == NAME).unwrap();
        Bench {
            rig,
            set,
            scopes,
            settings,
            index,
        }
    }

    /// feed-synth --out-of-range plays its `frame`-th frame, and the app has a moment to take it.
    fn play_feed_synth(&mut self, frame: u64) {
        let mut card = synthetic::test_card_out_of_range(WIDTH, HEIGHT, 0);
        synthetic::move_test_card_marker(&mut card, WIDTH, HEIGHT, frame);
        let set = &self.set;
        self.rig.play(WIDTH, HEIGHT, &card, |rig| {
            rig.poll(|encoder, frame| set.on_frame(encoder, frame))
        });
    }

    /// The waveform Panel drawn at `scale` as the Window draws it, in a target with a margin round the cell:
    /// the image (RGBA8, rows top first), its size and the Panel's layout.
    fn draw_panel(&self, scale: f32) -> (Vec<u8>, [u32; 2], PanelLayout) {
        let margin = style::SPACE_MARGIN * scale;
        let cell = [margin, margin, CELL[0] * scale, CELL[1] * scale];
        let size = [
            (cell[2] + 2.0 * margin).ceil() as u32,
            (cell[3] + 2.0 * margin).ceil() as u32,
        ];
        let panel = PanelLayout::new(cell, scale);
        let i = self.index;

        // The title bar, laid out as main.rs lays it out: name, tags, chips and the settings button.
        let title = self.set.titles(&DisplayContext::default(), &self.settings)[i].clone();
        let mut toolbar = self.set.toolbar(i, &self.settings);
        let has_popover = !self.set.popover_defs(i).is_empty();
        toolbar.fit_to(panel.title[2], has_popover, scale);
        let (l, tags, _) = window_text::lay_out_toolbar_title(
            &title,
            &toolbar.chips,
            &toolbar.groups,
            has_popover,
            panel.title,
            scale,
        );
        let weight = (style::LINE_WEIGHT * scale).max(1.0);
        let mut prims: Vec<Prim> = panel
            .lines(weight)
            .iter()
            .map(|edge| Prim::Rect {
                rect: *edge,
                colour: style::rgba(style::LINE_BORDER),
            })
            .collect();
        for (c, (chip, r)) in toolbar.chips.iter().zip(&l.chips).enumerate() {
            prims.extend(chip.draw([r[0], r[1]], toolbar.on[c], State::Normal, scale));
        }
        if let Some(button) = l.settings {
            prims.extend(components::settings_button(button, State::Normal, scale));
        }

        // The Panel's text (title and scale), drawn under the rectangles; the chips' text goes over them.
        let mut texts: Vec<Words> = Vec::new();
        let mut over: Vec<Words> = Vec::new();
        let line = |face: Face| face.metrics().0 * scale * face.metrics().1;
        let [_, bar_top, _, bar_h] = panel.title;
        texts.push((
            title.name.clone(),
            l.name[0],
            bar_top + (bar_h - line(Face::Label)) / 2.0,
            l.name[2].max(1.0),
            Face::Label,
            style::TEXT_PRIMARY,
            Align::Left,
            panel.title,
        ));
        if let Some(t) = l.tags {
            texts.push((
                tags,
                t[0],
                bar_top + (bar_h - line(Face::Scale)) / 2.0,
                t[2].max(1.0),
                Face::Scale,
                style::TEXT_MUTED,
                Align::Left,
                panel.title,
            ));
        }
        for mark in self.set.scale_texts(i, panel.content, scale) {
            texts.push((
                mark.text,
                mark.left,
                mark.top - line(Face::Scale) / 2.0,
                mark.width,
                Face::Scale,
                style::TEXT_MUTED,
                if mark.centred {
                    Align::Centre
                } else if mark.right_aligned {
                    Align::Right
                } else {
                    Align::Left
                },
                panel.content,
            ));
        }
        let mut rects = Vec::new();
        for prim in prims {
            match prim {
                Prim::Rect { rect, colour } => rects.push((rect, colour)),
                Prim::Text {
                    text,
                    rect,
                    face,
                    colour,
                    align,
                } => over.push((
                    text,
                    rect[0],
                    rect[1] + (rect[3] - line(face)) / 2.0,
                    rect[2].max(1.0),
                    face,
                    colour,
                    align,
                    rect,
                )),
            }
        }
        let image = self.render(size, scale, panel.content, &rects, [&texts, &over]);
        (image, size, panel)
    }

    /// Clears to the Window's surface, draws the display into its Panel, then the Panel's text, the flat
    /// rectangles and the text over them (the order of `main.rs`), and reads the target back.
    fn render(
        &self,
        [width, height]: [u32; 2],
        scale: f32,
        content: [f32; 4],
        rects: &[([f32; 4], [f32; 4])],
        layers: [&[Words]; 2],
    ) -> Vec<u8> {
        let (device, queue) = (&self.rig.device, &self.rig.queue);
        let mut font_system = FontSystem::new();
        let cache = Cache::new(device);
        let mut viewport = Viewport::new(device, &cache);
        viewport.update(queue, Resolution { width, height });
        let mut atlas = TextAtlas::with_color_mode(device, queue, &cache, TARGET, ColorMode::Web);
        let mut swash = SwashCache::new();
        let renderers = layers.map(|texts| {
            let buffers: Vec<Buffer> = texts
                .iter()
                .map(|(text, _, _, width, face, _, align, _)| {
                    let (size, line) = face.metrics();
                    let mut buffer = Buffer::new(
                        &mut font_system,
                        Metrics::new(size * scale, size * scale * line),
                    );
                    buffer.set_size(Some(*width), None);
                    buffer.set_wrap(glyphon::cosmic_text::Wrap::None);
                    let align = match align {
                        Align::Left => None,
                        Align::Centre => Some(glyphon::cosmic_text::Align::Center),
                        Align::Right => Some(glyphon::cosmic_text::Align::Right),
                    };
                    buffer.set_text(
                        text,
                        &Attrs::new().family(Family::SansSerif),
                        Shaping::Advanced,
                        align,
                    );
                    buffer.shape_until_scroll(&mut font_system, false);
                    buffer
                })
                .collect();
            let areas = buffers
                .iter()
                .zip(texts)
                .map(|(buffer, (_, left, top, _, _, c, _, b))| TextArea {
                    buffer,
                    left: *left,
                    top: *top,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: b[0] as i32,
                        top: b[1] as i32,
                        right: (b[0] + b[2]).ceil() as i32,
                        bottom: (b[1] + b[3]).ceil() as i32,
                    },
                    default_color: Color::rgb(c[0], c[1], c[2]),
                    custom_glyphs: &[],
                });
            let mut renderer =
                TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
            renderer
                .prepare(
                    device,
                    queue,
                    &mut font_system,
                    &mut atlas,
                    &viewport,
                    areas,
                    &mut swash,
                )
                .expect("the text prepares");
            renderer
        });
        let mut flat = FlatRects::new(device, queue, TARGET);
        flat.set([width as f32, height as f32], rects);
        self.rig
            .render_over(width, height, style::clear(style::SURFACE_WINDOW), |pass| {
                self.set.draw(
                    pass,
                    self.index,
                    content,
                    [width as f32, height as f32],
                    scale,
                );
                let [under, over] = &renderers;
                under
                    .render(&atlas, &viewport, pass)
                    .expect("the text draws");
                flat.draw(pass);
                over.render(&atlas, &viewport, pass)
                    .expect("the text over the rectangles draws");
            })
    }
}

/// Writes `image` as a PNG into `$SCOPECREEP_CAPTURE`, when it is set.
fn capture(image: &[u8], [width, height]: [u32; 2], name: &str) {
    let Some(folder) = std::env::var_os("SCOPECREEP_CAPTURE") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let ppm = folder.join(format!("{name}.ppm"));
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.extend(image.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
    std::fs::write(&ppm, bytes).unwrap();
    let png = folder.join(format!("{name}.png"));
    let made = std::process::Command::new("convert")
        .arg(&ppm)
        .arg(&png)
        .status()
        .is_ok_and(|s| s.success());
    std::fs::remove_file(&ppm).unwrap();
    assert!(
        made,
        "convert (ImageMagick) could not write {}",
        png.display()
    );
}

#[test]
fn the_feed_synth_out_of_range_frame_is_counted_in_the_edge_rows_of_its_columns() {
    let mut b = Bench::new("oorcounts");
    b.play_feed_synth(17);
    let counts = b.scopes.luma_waveform.counts();
    assert_eq!(counts.columns(), WIDTH);
    // feed-synth sends no Colour Setup, so the app assumes video levels: a value lands on code
    // value × 876 + 64, and only values past about −0.074 and 1.095 fall off the scale into the edge rows.
    let levels = b.scopes.luma_waveform.measurement().levels;
    assert_eq!(levels, Levels::Video);
    let (gain, offset) = levels.scale_and_offset();
    let strip = HEIGHT / 12;
    let (mut lit_below, mut lit_above) = (0, 0);
    for x in 0..WIDTH {
        let v = -0.25 + 1.5 * f64::from(x as f32 / (WIDTH - 1) as f32);
        let code = v * gain + offset;
        let (below, above) = if code < -0.5 {
            lit_below += 1;
            (strip, 0)
        } else if code >= 1023.5 {
            lit_above += 1;
            (0, strip)
        } else {
            (0, 0)
        };
        assert_eq!(
            (counts.below(x), counts.above(x)),
            (below, above),
            "column {x}"
        );
    }
    assert_eq!(
        (lit_below, lit_above),
        (226, 198),
        "columns lit in the bottom and top edge rows"
    );
}

#[test]
fn the_edge_rows_light_where_feed_synth_goes_out_of_range_at_scale_1_1_5_and_2() {
    let mut b = Bench::new("oorhidpi");
    b.play_feed_synth(17);
    for scale in [1.0f32, 1.5, 2.0] {
        let (image, size, panel) = b.draw_panel(scale);
        capture(&image, size, &format!("out-of-range-waveform-{scale}x"));
        let plot = PlotLayout::fitted(panel.content, scale, [WIDTH, HEIGHT]);
        let brightest_in = |rect: [f32; 4], column: u32| {
            let x = plot.x_of_column(column, WIDTH) as usize;
            (rect[1] as usize..(rect[1] + rect[3]).ceil() as usize)
                .map(|y| image[(y * size[0] as usize + x) * 4])
                .max()
                .unwrap()
        };
        let (above, below) = (plot.above(), plot.below());
        for column in [1760, 1900] {
            assert!(
                brightest_in(above, column) >= 128,
                "{scale}x: above 1 at column {column} lights the top edge row: {}",
                brightest_in(above, column)
            );
            assert!(
                brightest_in(below, column) < 64,
                "{scale}x: nothing below 0 at column {column}: {}",
                brightest_in(below, column)
            );
        }
        for column in [20, 180] {
            assert!(
                brightest_in(below, column) >= 128,
                "{scale}x: below 0 at column {column} lights the bottom edge row: {}",
                brightest_in(below, column)
            );
            assert!(
                brightest_in(above, column) < 64,
                "{scale}x: nothing above 1 at column {column}: {}",
                brightest_in(above, column)
            );
        }
        for column in [700, 1200] {
            assert!(
                brightest_in(above, column) < 64 && brightest_in(below, column) < 64,
                "{scale}x: in range at column {column}, both edge rows stay dark"
            );
        }
        // The scale's numbers were drawn: some text-grey pixels left of the plot, in the gutter.
        let gutter = (panel.content[0] as usize..plot.plot()[0] as usize)
            .flat_map(|x| {
                (plot.plot()[1] as usize..(plot.plot()[1] + plot.plot()[3]) as usize)
                    .map(move |y| (x, y))
            })
            .filter(|&(x, y)| image[(y * size[0] as usize + x) * 4] > 60)
            .count();
        assert!(gutter > 0, "{scale}x: the scale's numbers are drawn");
    }
}
