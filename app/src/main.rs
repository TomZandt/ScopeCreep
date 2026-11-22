//! The scopecreep app: one window holding a grid of Panels, one per display (ADR 0011): the Picture View of the
//! latest Feed frame and the luma waveform for now, each with a quiet label under it, and a quiet status line
//! along the bottom (Resolve's playback rate, dropped frames, the Sender driving the Feed). The borders between
//! Panels can be dragged and are remembered. Every display shows the Feed after Output Conversion, in the Output
//! Colour Space, whenever the Colour Setup has one (`output_conversion.rs`). Right-click a Panel for its settings,
//! which are remembered per Panel (`~/.config/scopecreep/panel-settings`). The menu bar along the very top holds
//! the app's own commands (the Layout menu, `menu_bar.rs`).
//!
//!   scopecreep [--feed <path>] [--no-scripting]
//!
//!   --feed           default: $SCOPECREEP_FEED, else /dev/shm/scopecreep-feed
//!   --no-scripting   never read the project by Resolve scripting (the Colour Setup then comes only from the
//!                    Sender node's manual override). Resolve's Python: $SCOPECREEP_RESOLVE_PYTHON, else
//!                    /opt/resolve/bin/ResolvePython.
//!
//! Two threads. The Feed thread polls the Feed every millisecond (the ring is short and the Sender never waits),
//! puts the frames to show on the GPU once, converts them once, hands that one frame to every display, and wakes
//! the window. The window thread only draws.
use glyphon::{
    Attrs, Buffer, Cache, Color, ColorMode, Family, FontSystem, Metrics, Resolution, Shaping,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use scopecreep::auto_resize;
use scopecreep::commands::{Command, Hotkey, Hotkeys, Key};
use scopecreep::components::{self, Align, BarKind, MenuBar, Prim, State, Tooltip};
use scopecreep::display::{DisplaySet, Pointer};
use scopecreep::dock::{self, Zone};
use scopecreep::feed_loop::{FeedLoop, Texts};
use scopecreep::flat_rects::FlatRects;
use scopecreep::gpu_frame::FrameUploader;
use scopecreep::hotkeys_panel::HotkeysPanel;
use scopecreep::layout::MARGIN;
use scopecreep::layout_tree::{Border, Dir, Edge, LayoutTree, Press};
use scopecreep::layouts::Layouts;
use scopecreep::menu_bar::{self, AddEntry, BarState, LayoutMode, MenuAction, Template};
use scopecreep::monitor::Monitor;
use scopecreep::output_conversion::OutputConversion;
use scopecreep::page_bar::{self, BarPart, PageBar};
use scopecreep::page_edit::{self, Room};
use scopecreep::pages::{self, PageRename, Pages};
use scopecreep::panel_grid::{self, PanelLayout, min_panel_size};
use scopecreep::resolve_helper::ScriptHelper;
use scopecreep::settings::{Amount, EditKey, Menu, PanelAction, PanelSettings};
use scopecreep::style;
use scopecreep::tab_strip;
use scopecreep::toolbar::{ChipAction, Toolbar};
use scopecreep::view_strip::{self, StripPart, ViewStrip};
use scopecreep::window_text;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as LogicalKey, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, Window, WindowId};

/// Neutral, dark, and the same on every channel: nothing around the picture may tint the eye.
/// Chrome colours and sizes come from the style tokens (ADR 0026); this gives text colours to glyphon.
fn text_colour(c: [u8; 3]) -> Color {
    Color::rgb(c[0], c[1], c[2])
}

struct Wake;

/// What the pointer rests on that shows a hover: a Panel's title bar, or the status line.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Hover {
    Title(usize),
    /// Chip `.1` of Panel `.0`'s toolbar.
    Chip(usize, usize),
    /// Panel `.0`'s settings button.
    SettingsButton(usize),
    /// Fit or Link in the view strip.
    Strip(StripPart),
    Status,
}

/// What of a title bar's toolbar the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ToolbarHit {
    Chip(usize, usize),
    Settings(usize),
}

/// Where the border positions are remembered: `$XDG_CONFIG_HOME/scopecreep/panel-grid`, else
/// `~/.config/scopecreep/panel-grid`.
/// The Panel keys as the grid and the saved layouts take them. A layout names Panels by key, which is the display's
/// name for the first instance of a display, so a layout saved before instances still recalls (ADR 0039).
fn as_strs(keys: &[String]) -> Vec<&str> {
    keys.iter().map(String::as_str).collect()
}

fn grid_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("scopecreep").join("panel-grid"))
}

/// Where the pages are remembered (ADR 0041): the layout of every page and which is shown. `panel-grid` and
/// `layouts` (the nine named layouts) are only read, once, to make the first pages.
fn pages_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("pages"))
}

/// Remember the pages.
fn write_pages(pages: &Pages, keys: &[String]) {
    if let Some(path) = pages_file()
        && let Err(e) = scopecreep::settings::save(&path, &pages.to_text(&as_strs(keys)))
    {
        eprintln!("ScopeCreep: cannot remember the pages: {e}");
    }
}

/// Where the Panels' settings are remembered, next to the border positions.
fn settings_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("panel-settings"))
}

/// Where the named layouts are remembered, next to the border positions.
fn layouts_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("layouts"))
}

/// Where the hotkeys the user changed are remembered, next to the layouts (ADR 0036).
fn hotkeys_file() -> Option<PathBuf> {
    grid_file().map(|p| p.with_file_name("hotkeys"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut feed = scopecreep_feed::default_path();
    let mut scripting = true;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--feed" => feed = args.next().ok_or("--feed needs a path")?.into(),
            "--no-scripting" => scripting = false,
            "--version" => {
                println!(
                    "ScopeCreep {} (Feed protocol {}.{})",
                    env!("CARGO_PKG_VERSION"),
                    scopecreep_feed::VERSION_MAJOR,
                    scopecreep_feed::VERSION_MINOR
                );
                return Ok(());
            }
            _ => {
                return Err(format!(
                    "unknown argument {arg}. Usage: scopecreep [--feed <path>] [--no-scripting]"
                )
                .into());
            }
        }
    }
    let event_loop = EventLoop::<Wake>::with_user_event().build()?;
    let mut app = App {
        feed,
        scripting,
        proxy: event_loop.create_proxy(),
        window: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    feed: PathBuf,
    scripting: bool,
    proxy: EventLoopProxy<Wake>,
    window: Option<AppWindow>,
}

/// How a piece of text is set: everything besides the words and the room that changes how it is shaped.
#[derive(Clone, Copy, PartialEq)]
struct TextStyle {
    /// Logical pixels.
    size: f32,
    /// Line height as a multiple of `size`.
    line: f32,
    align: Option<glyphon::cosmic_text::Align>,
    wrap: bool,
}

impl TextStyle {
    /// The quiet labels, the status line and the menu.
    const LABEL: TextStyle = TextStyle {
        size: style::TYPE_LABEL,
        line: style::TYPE_LABEL_LINE,
        align: None,
        wrap: true,
    };
    /// A notice on its own over the Panel.
    const NOTICE: TextStyle = TextStyle {
        size: style::TYPE_NOTICE,
        line: style::TYPE_NOTICE_LINE,
        align: Some(glyphon::cosmic_text::Align::Center),
        wrap: true,
    };
}

/// A piece of text kept shaped until what it says, how it is set, or the room it has, changes.
struct Text {
    buffer: Buffer,
    key: Option<(String, u32, u32, TextStyle)>,
}

impl Text {
    fn new(font_system: &mut FontSystem) -> Text {
        Text {
            buffer: Buffer::new(
                font_system,
                Metrics::new(
                    style::TYPE_LABEL,
                    style::TYPE_LABEL * style::TYPE_LABEL_LINE,
                ),
            ),
            key: None,
        }
    }

    /// Shape `text` in `style` (at `scale`) in `width` device pixels, unless it already is.
    fn set(
        &mut self,
        font_system: &mut FontSystem,
        text: &str,
        style: TextStyle,
        scale: f32,
        width: f32,
    ) {
        let width = width.round() as u32;
        if self.key.as_ref().is_some_and(|(t, w, s, st)| {
            t == text && *w == width && *s == scale.to_bits() && *st == style
        }) {
            return;
        }
        let attrs = Attrs::new().family(Family::SansSerif);
        self.buffer.set_metrics(Metrics::new(
            style.size * scale,
            style.size * scale * style.line,
        ));
        self.buffer.set_size(Some((width as f32).max(1.0)), None);
        self.buffer.set_wrap(if style.wrap {
            glyphon::cosmic_text::Wrap::WordOrGlyph
        } else {
            glyphon::cosmic_text::Wrap::None
        });
        self.buffer
            .set_text(text, &attrs, Shaping::Advanced, style.align);
        self.buffer.shape_until_scroll(font_system, false);
        self.key = Some((text.to_string(), width, scale.to_bits(), style));
    }
}

/// What a Panel's text is made of: its label (one line, in the title bar), and the numbers on its display's scale.
struct PanelText {
    label: Text,
    tags: Text,
    scale: Vec<Text>,
}

struct AppWindow {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    displays: DisplaySet,
    /// The pages: each its own layout tree, focus and undo (ADR 0041).
    pages: Pages,
    /// The page tab last clicked and when, for a double-click to rename it.
    last_tab_click: Option<(usize, std::time::Instant)>,
    /// The button (device pixels) a bar menu drops from instead of its title, while the menu was opened from it: the
    /// Page bar's **+** or Add panel, an edge **+**, or the empty page's button.
    bar_anchor: Option<[f32; 4]>,
    /// Every Panel's settings, shared with the Feed thread (which gives them to the displays).
    settings: Arc<Mutex<PanelSettings>>,
    /// The right-click menu, while open, with where it opened.
    menu: Option<(Menu, [f32; 2])>,
    /// The menu bar (#147): which of its menus is open, the title under the pointer, and what the Layout menu
    /// shows. A bar menu and a Panel's popover are never open together.
    bar: BarState,
    bar_hover: Option<usize>,
    layout_mode: LayoutMode,
    /// The Hotkeys panel, while it shows in place of the Settings menu (#148).
    hotkeys_panel: Option<HotkeysPanel>,
    /// The left button is down (a menu entry under it shows pressed).
    left_down: bool,
    /// The Panel filling the Window (Ctrl+F, #180), if any. Display state: never saved.
    full: Option<usize>,
    /// The hotkey of each command (ADR 0036).
    hotkeys: Hotkeys,
    /// A page's name being typed in its tab.
    renaming: Option<PageRename>,
    modifiers: ModifiersState,
    flat: FlatRects,
    menu_renderer: TextRenderer,
    menu_texts: Vec<Text>,
    /// Where each Panel was last drawn (device pixels), for a right-click to find its Panel in.
    cells: Vec<[f32; 4]>,
    /// Where each Panel's display was last drawn (device pixels), for the pointer to hand to displays that use it.
    contents: Vec<[f32; 4]>,
    /// The Panel whose display holds the pointer (a Loupe being dragged), if any.
    pointer_panel: Option<usize>,
    /// The Panel a middle-button press went down on, which gets the movement until the button is let go.
    middle_panel: Option<usize>,
    /// A notice from the layout (an add with no room), shown until the next click or key.
    layout_notice: Option<String>,
    /// Where the empty page's Add panel button was last drawn (device pixels), while the page is empty.
    add_button: Option<[f32; 4]>,
    /// The side of a Panel whose edge + opened the Add list: the display picked goes there, not beside the focus.
    edge_add: Option<(usize, Edge)>,
    /// The border being dragged, if any.
    drag: Option<Border>,
    /// The Panel being dragged by its title bar, to swap with the one it is let go on.
    panel_drag: Option<usize>,
    /// Where the title press behind `panel_drag` began, and whether the pointer has moved far enough since for it to
    /// be a drag (a press that never does is a click and changes nothing).
    panel_drag_start: [f32; 2],
    panel_drag_live: bool,
    /// The press behind `panel_drag` was on a tab: a click that is no drag shows that tab (#211).
    panel_drag_tab: bool,
    /// The tab strip of each Panel that is the shown one of a tab group with more than one Panel, and where it is
    /// (device pixels), for the pointer. None for every other Panel.
    tab_strips: Vec<Option<(tab_strip::TabStrip, [f32; 4])>>,
    /// Remembered display state (a Loupe's position) changed and is not written yet.
    state_unsaved: bool,
    cursor: [f32; 2],
    /// Where the grid and its gaps were last drawn (device pixels), for the pointer to find borders in.
    grid_area: [f32; 4],
    grid_gap: f32,
    scale: f32,
    texts: Arc<Mutex<Texts>>,
    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,
    panel_texts: Vec<PanelText>,
    status: Text,
    notice: Text,
    /// What the pointer rests on that shows a hover (#99).
    hover: Option<Hover>,
    /// Each Panel's title bar and the status line as last drawn (device pixels), and the text a hover shows for each.
    title_bars: Vec<[f32; 4]>,
    title_hovers: Vec<String>,
    /// Each Panel's toolbar and where its chips and settings button were last drawn (device pixels).
    toolbars: Vec<Toolbar>,
    title_layouts: Vec<components::TitleLayout>,
    /// The view strip as last drawn (ADR 0040): what its buttons hit.
    view_strip: ViewStrip,
    status_bar: [f32; 4],
    status_hover: String,
    stop: Arc<AtomicBool>,
    feed_thread: Option<std::thread::JoinHandle<()>>,
    // Last, so it outlives the surface made from it.
    window: Arc<Window>,
}

impl AppWindow {
    fn new(
        window: Arc<Window>,
        event_loop: &ActiveEventLoop,
        feed: PathBuf,
        scripting: bool,
        proxy: EventLoopProxy<Wake>,
    ) -> Result<AppWindow, Box<dyn std::error::Error>> {
        // WGPU_BACKEND and friends are honoured, for trying another graphics backend.
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                Box::new(event_loop.owned_display_handle()),
            ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| format!("no drawing surface: {e}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| format!("no usable GPU: {e}"))?;
        let filterable = adapter
            .features()
            .contains(wgpu::Features::FLOAT32_FILTERABLE);
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("scopecreep"),
            required_features: if filterable {
                wgpu::Features::FLOAT32_FILTERABLE
            } else {
                wgpu::Features::empty()
            },
            // Resolve fills most of the graphics memory (Stage 0: 7.7 of 8 GB): take only what is used.
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| {
            format!(
                "{} ({:?}) would not give a device: {e}",
                info.name, info.backend
            )
        })?;
        device.on_uncaptured_error(Arc::new(|e| eprintln!("ScopeCreep: graphics error: {e}")));
        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        // Not an sRGB surface: the Feed's values are shown as they are, with no extra encoding.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("the window surface is not supported by the GPU")?;
        config.format = format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let displays = DisplaySet::standard(&device, &queue, format, filterable);
        // The Window can't be made smaller than the default layout and every menu need (#172).
        let [min_w, min_h] = scopecreep::layout::min_window(
            &LayoutTree::default_for(&as_strs(&displays.keys())),
            &menu_bar::largest_menus(),
        );
        window.set_min_inner_size(Some(LogicalSize::new(min_w, min_h)));
        let settings = Arc::new(Mutex::new(
            settings_file()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map_or_else(PanelSettings::default, |text| {
                    PanelSettings::from_text(&text)
                }),
        ));
        displays.set_link(
            scopecreep::zoom::link_remembered(&settings.lock().unwrap()),
            0,
        );
        let read = |path: Option<PathBuf>| path.and_then(|p| std::fs::read_to_string(p).ok());
        let (pages_text, layouts_text, grid_text) =
            (read(pages_file()), read(layouts_file()), read(grid_file()));
        // Further instances ("Channel Viewer 2") that a saved page names are made again before it is read.
        let saved: Vec<&str> = [&pages_text, &layouts_text, &grid_text]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        pages::recreate_saved_copies(&displays, &saved);
        let keys = displays.keys();
        let names = as_strs(&keys);
        let mut pages = pages_text
            .as_deref()
            .and_then(|text| Pages::from_text(&names, text))
            .unwrap_or_else(|| {
                // The first run with pages: Main holds the arrangement there was, then the named layouts follow.
                let main = grid_text.as_deref().map_or_else(
                    || LayoutTree::default_for(&names),
                    |text| LayoutTree::from_text(&names, text),
                );
                let layouts = layouts_text
                    .as_deref()
                    .map_or_else(Layouts::default, Layouts::from_text);
                Pages::migrate(main, &layouts, &names)
            });
        // Every page has Panels of its own, Main on the first instances (where the saved settings are kept); what no
        // page holds is swept.
        pages.own_instances(&displays.names(), |name| {
            displays.make_named(name).unwrap_or(0)
        });
        pages.sweep(&displays, &mut settings.lock().unwrap());
        pages.show_on(&displays);
        write_pages(&pages, &displays.keys());
        let mut font_system = FontSystem::new();
        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let mut atlas = TextAtlas::with_color_mode(
            &device,
            &queue,
            &cache,
            format,
            if format.is_srgb() {
                ColorMode::Accurate
            } else {
                ColorMode::Web
            },
        );
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let menu_renderer =
            TextRenderer::new(&mut atlas, &device, wgpu::MultisampleState::default(), None);
        let flat = FlatRects::new(&device, &queue, format);
        let status = Text::new(&mut font_system);
        let notice = Text::new(&mut font_system);
        let panel_texts = (0..displays.len())
            .map(|_| PanelText {
                label: Text::new(&mut font_system),
                tags: Text::new(&mut font_system),
                scale: Vec::new(),
            })
            .collect();

        let mut feed_loop = FeedLoop::new(
            displays.clone(),
            settings.clone(),
            OutputConversion::new(&device, &queue),
        );
        let texts = feed_loop.texts();
        let stop = Arc::new(AtomicBool::new(false));
        let feed_thread = {
            let stop = stop.clone();
            let mut uploader = FrameUploader::new(&device, &queue);
            std::thread::Builder::new().name("feed".into()).spawn(move || {
                // The Resolve scripting helper is owned by this thread's Monitor: it is killed when the app quits.
                let mut monitor = if scripting { Monitor::with_project_reader(&feed, Box::new(ScriptHelper::from_env())) } else { Monitor::new(&feed) };
                // SCOPECREEP_FRAME_LOG=<file>: one line per Feed frame read and shown, with the Sender's timing (diagnostics).
                if let Some(path) = std::env::var_os("SCOPECREEP_FRAME_LOG").filter(|p| !p.is_empty()) {
                    match std::fs::File::create(&path) {
                        Ok(f) => monitor.set_trace(Box::new(std::io::LineWriter::new(f))),
                        Err(e) => eprintln!("ScopeCreep: cannot write the frame log: {e}"),
                    }
                }
                // SCOPECREEP_STATUS_LOG=1: one line a second on stderr with what the app made of the Feed (diagnostics).
                let status_log = std::env::var_os("SCOPECREEP_STATUS_LOG").is_some_and(|v| v != "0");
                let mut next_log = scopecreep_feed::now_ns();
                while !stop.load(Ordering::Relaxed) {
                    if status_log && scopecreep_feed::now_ns() >= next_log {
                        next_log += 1_000_000_000;
                        let s = monitor.status();
                        let fed = s.feed_stats.map_or(0, |f| f.frames_published);
                        eprintln!(
                            "status feed={:?} fed={} viewer_frames={} scope_frames={} repeats={} cache_ignored={} dropped={} frame={:?} project={:?} timeline={:?} colour={:?} project_state={:?} reads={} rate={} line={:?} notice={:?}",
                            s.feed,
                            fed,
                            s.viewer_frames,
                            feed_loop.scope_frames,
                            s.repeat_renders,
                            s.cache_renders_ignored,
                            s.dropped,
                            s.picture.as_ref().map(|p| p.frame_time),
                            s.project.as_ref().map(|p| p.project.as_str()),
                            s.project.as_ref().and_then(|p| p.timeline.as_deref()),
                            s.colour_setup().map(|c| c.label()),
                            s.project_state,
                            s.project_reads,
                            s.playback_rate.map_or("-".to_string(), |r| format!("{r:.3}")),
                            s.status_line(),
                            s.notice()
                        );
                    }
                    if feed_loop.step(&mut monitor, &mut uploader, scopecreep_feed::now_ns()).any() && proxy.send_event(Wake).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                // `monitor` is dropped here: the app detaches from the Feed and the Sender stops copying.
            })?
        };
        Ok(AppWindow {
            device,
            queue,
            surface,
            config,
            displays,
            pages,
            last_tab_click: None,
            bar_anchor: None,
            settings,
            menu: None,
            bar: BarState::default(),
            bar_hover: None,
            layout_mode: LayoutMode::Show,
            hotkeys_panel: None,
            left_down: false,
            full: None,
            hotkeys: hotkeys_file()
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map_or_else(Hotkeys::default, |text| Hotkeys::from_text(&text)),
            renaming: None,
            modifiers: ModifiersState::empty(),
            flat,
            menu_renderer,
            menu_texts: Vec::new(),
            cells: Vec::new(),
            contents: Vec::new(),
            pointer_panel: None,
            middle_panel: None,
            layout_notice: None,
            add_button: None,
            edge_add: None,
            drag: None,
            panel_drag: None,
            panel_drag_start: [0.0; 2],
            panel_drag_live: false,
            panel_drag_tab: false,
            tab_strips: Vec::new(),
            state_unsaved: false,
            cursor: [0.0, 0.0],
            grid_area: [0.0; 4],
            grid_gap: 0.0,
            scale: 1.0,
            texts,
            font_system,
            swash_cache: SwashCache::new(),
            viewport,
            atlas,
            text_renderer,
            panel_texts,
            status,
            notice,
            hover: None,
            title_bars: Vec::new(),
            title_hovers: Vec::new(),
            toolbars: Vec::new(),
            title_layouts: Vec::new(),
            view_strip: ViewStrip {
                enabled: false,
                readout: String::new(),
                link: true,
            },
            status_bar: [0.0; 4],
            status_hover: String::new(),
            stop,
            feed_thread: Some(feed_thread),
            window,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    /// What a press at the pointer would start: a border drag or a Panel drag.
    /// The tab or overflow chip under the pointer: the Panel shown by its tab group, and what was hit. A tab is
    /// named by its Panel.
    fn tab_hit(&self) -> Option<(usize, tab_strip::TabHit)> {
        self.tab_strips.iter().enumerate().find_map(|(shown, s)| {
            let (strip, area) = s.as_ref()?;
            let group = self.pages.tree().tab_group(shown)?;
            Some((
                shown,
                match strip.hit(*area, self.cursor, self.scale)? {
                    tab_strip::TabHit::Tab(n) => tab_strip::TabHit::Tab(*group.tabs.get(n)?),
                    tab_strip::TabHit::Overflow => tab_strip::TabHit::Overflow,
                },
            ))
        })
    }

    fn press_under_pointer(&self) -> Option<Press> {
        // A Panel filling the Window has no borders to drag and nothing to swap with.
        if self.full.is_some() {
            return None;
        }
        self.pages.tree().press_at(
            self.grid_area,
            self.grid_gap,
            self.cursor,
            style::SPACE_GRAB * self.scale,
            self.scale,
        )
    }

    /// Remember the pages.
    fn save_grid(&self) {
        write_pages(&self.pages, &self.displays.keys());
    }

    /// Rest the hover on what is under the pointer: a Panel's title bar or the status line (never while a drag or
    /// a Loupe holds the pointer).
    fn update_hover(&mut self) {
        let busy = self.pointer_panel.is_some()
            || self.middle_panel.is_some()
            || self.drag.is_some()
            || self.panel_drag.is_some();
        let hover = if busy {
            None
        } else if components::contains(self.status_bar, self.cursor) {
            Some(Hover::Status)
        } else if let Some(part) = self.strip_hit() {
            Some(Hover::Strip(part))
        } else {
            match self.toolbar_hit() {
                Some(ToolbarHit::Chip(panel, chip)) => Some(Hover::Chip(panel, chip)),
                Some(ToolbarHit::Settings(panel)) => Some(Hover::SettingsButton(panel)),
                None => self
                    .title_bars
                    .iter()
                    .position(|t| components::contains(*t, self.cursor))
                    .map(Hover::Title),
            }
        };
        if hover != self.hover {
            self.hover = hover;
            self.window.request_redraw();
        }
    }

    /// The view strip's button under the pointer (none while no picture Panel is shown).
    fn strip_hit(&self) -> Option<StripPart> {
        self.view_strip
            .hit(self.config.width as f32, self.cursor, self.scale)
    }

    /// The chip or settings button of a title bar under the pointer.
    fn toolbar_hit(&self) -> Option<ToolbarHit> {
        self.title_layouts
            .iter()
            .enumerate()
            .find_map(|(panel, l)| {
                if let Some(chip) = l
                    .chips
                    .iter()
                    .position(|c| components::contains(*c, self.cursor))
                {
                    return Some(ToolbarHit::Chip(panel, chip));
                }
                l.settings
                    .filter(|b| components::contains(*b, self.cursor))
                    .map(|_| ToolbarHit::Settings(panel))
            })
    }

    /// Open Panel `panel`'s popover at `at` (none when every setting of the Panel is a chip, or it has none).
    fn open_popover(&mut self, panel: usize, at: [f32; 2]) {
        let current = self
            .displays
            .popover_settings(panel, &self.settings.lock().unwrap());
        let defs = match self.toolbars.get(panel) {
            Some(toolbar) => self.displays.popover_for(panel, toolbar),
            None => self.displays.popover_defs(panel),
        };
        self.menu = Some((Menu::for_panel(panel, &defs, &current), at));
    }

    /// The strip's Fit: every picture Panel back to Fit. Zoom is display state: it is never put in the settings or
    /// saved.
    fn fit_all(&mut self) {
        self.displays.fit_all(&self.contents, self.scale);
        self.window.request_redraw();
    }

    /// Switch the Window's Link from the strip (turning it on takes the view of the first picture Panel) and
    /// remember it; zoom and pan are not remembered.
    fn set_link(&mut self, on: bool) {
        self.displays
            .set_link_from_strip(on, &self.contents, self.scale);
        let changed = scopecreep::zoom::remember_link(&mut self.settings.lock().unwrap(), on);
        if changed {
            self.save_settings();
        }
        self.window.request_redraw();
    }

    /// Set a Panel's setting from the popover or a chip, remembering it if it changed.
    fn choose(&mut self, panel: usize, key: &str, value: &str) {
        let name = self.displays.key(panel);
        let changed = {
            let mut settings = self.settings.lock().unwrap();
            let before = settings.version();
            let value = self.displays.stored_value(panel, &settings, key, value);
            settings.set(&name, key, &value);
            settings.version() != before
        };
        if changed {
            self.save_settings();
        }
        self.window.request_redraw();
    }

    /// An amount the popover changed: remember it as the Panel's setting and show the settings again.
    fn apply_change(&mut self, panel: usize, change: scopecreep::settings::Change) {
        self.choose(panel, change.key, &Amount::store(change.value));
        self.refresh_menu();
    }

    /// The open popover reads the Panel's settings again (a value changed, or the Panel was reset).
    fn refresh_menu(&mut self) {
        let Some((menu, _)) = self.menu.as_ref() else {
            return;
        };
        let panel = menu.panel;
        let (current, at_default) = {
            let settings = self.settings.lock().unwrap();
            (
                self.displays.popover_settings(panel, &settings),
                self.displays.at_default(panel, &settings),
            )
        };
        if let Some((menu, _)) = self.menu.as_mut() {
            menu.sync(&current);
            menu.panel_at_default = at_default;
        }
        self.window.request_redraw();
    }

    fn pointer_moved(&mut self, x: f32, y: f32) {
        self.cursor = [x, y];
        if self.menu.as_ref().is_some_and(|(m, _)| m.dragging()) {
            // A slider being dragged follows the pointer on every move: its value, the popover's readout and
            // the Panel's setting.
            let (rect, scale) = (self.menu_rect(), self.scale);
            if let (Some(rect), Some((menu, _))) = (rect, self.menu.as_mut()) {
                let panel = menu.panel;
                if let Some(change) = menu.drag_to(rect, x, scale) {
                    self.apply_change(panel, change);
                }
            }
            return;
        }
        let over_title = self.bar_title_at_pointer();
        if over_title != self.bar_hover {
            self.bar_hover = over_title;
            self.window.request_redraw();
        }
        if self.bar.open.is_some() {
            // A middle-button drag that began before the menu opened still follows the pointer.
            if let Some(panel) = self.middle_panel {
                self.display_pointer(panel, Pointer::Move(self.cursor));
            }
            if let Some(title) = over_title.filter(|t| self.bar.open != Some(*t)) {
                self.bar.hover_title(title);
                self.reset_bar_menus();
            }
            // The entry under the pointer lights.
            self.window.request_redraw();
            return;
        }
        self.update_hover();
        if let Some(panel) = self.pointer_panel.or(self.middle_panel) {
            self.display_pointer(panel, Pointer::Move(self.cursor));
        } else if let Some(border) = &self.drag {
            self.pages.tree_mut().drag(
                border,
                self.cursor,
                self.grid_area,
                self.grid_gap,
                min_panel_size(self.scale),
            );
            self.window.request_redraw();
        } else if self.panel_drag.is_some() {
            if !self.panel_drag_live {
                self.panel_drag_live =
                    dock::is_drag(self.panel_drag_start, self.cursor, self.scale);
            }
            self.window.request_redraw();
        } else {
            // The edge + signs show and light with the pointer.
            self.window.request_redraw();
            self.window.set_cursor(match self.press_under_pointer() {
                Some(Press::Border(Border { dir: Dir::Row, .. })) => CursorIcon::ColResize,
                Some(Press::Border(Border { dir: Dir::Col, .. })) => CursorIcon::RowResize,
                Some(Press::Panel(_)) => CursorIcon::Grab,
                None => CursorIcon::Default,
            });
        }
    }

    /// Right button: open the menu of the Panel under the pointer (none for a Panel without settings, or off a
    /// Panel), or close the one that is open. A menu of the menu bar just closes.
    fn right_click(&mut self) {
        if self.bar.open.is_some() {
            self.close_bar();
            return;
        }
        let was_open = self.menu.take().is_some();
        let hit = self
            .cells
            .iter()
            .position(|c| Menu::contains(*c, self.cursor));
        if let Some(panel) = hit {
            self.open_popover(panel, self.cursor);
        }
        if was_open || self.menu.is_some() {
            self.window.request_redraw();
        }
    }

    /// The open bar menu, as it is now.
    fn bar_menu(&self) -> Option<menu_bar::Menu> {
        match self.bar.open? {
            menu_bar::LAYOUT if self.layout_mode == LayoutMode::Add => {
                Some(menu_bar::add_menu(&self.add_entries()))
            }
            menu_bar::LAYOUT => Some(menu_bar::layout_menu_with_undo(
                &self.pages.names(),
                self.pages.current(),
                &self.hotkeys,
                self.layout_mode,
                self.pages.can_undo(),
            )),
            menu_bar::VIEW => Some(menu_bar::view_menu(
                &self.hotkeys,
                self.displays.legends_on(&self.settings.lock().unwrap()),
            )),
            menu_bar::SETTINGS => Some(match &self.hotkeys_panel {
                Some(panel) => panel.menu(&self.hotkeys),
                None => menu_bar::settings_menu(),
            }),
            _ => None,
        }
    }

    /// Where the open bar menu drops, under its title.
    fn bar_menu_rect(&self, menu: &menu_bar::Menu) -> Option<[f32; 4]> {
        let window = [self.config.width as f32, self.config.height as f32];
        Some(match self.bar_anchor {
            Some(anchor) => menu.drop.rect_near(anchor, window, self.scale),
            None => {
                let title = *menu_bar::bar()
                    .title_rects(0.0, 0.0, self.scale)
                    .get(self.bar.open?)?;
                menu.drop.rect(title, window, self.scale)
            }
        })
    }

    /// The menu bar's title under the pointer.
    fn bar_title_at_pointer(&self) -> Option<usize> {
        menu_bar::bar().hit(0.0, 0.0, self.cursor, self.scale)
    }

    /// Close the bar's menu (a name being typed is kept, as a click elsewhere does in any desktop menu).
    fn close_bar(&mut self) {
        self.bar.close();
        self.reset_bar_menus();
    }

    /// A bar menu closed or another opened: a name being typed is kept, a hotkey being captured dropped, and each
    /// menu opens as it first shows.
    fn reset_bar_menus(&mut self) {
        self.edge_add = None;
        self.bar_anchor = None;
        self.finish_rename(true);
        self.layout_mode = LayoutMode::Show;
        self.hotkeys_panel = None;
        self.window.request_redraw();
    }

    /// Remember the hotkeys.
    fn save_hotkeys(&self) {
        if let Some(path) = hotkeys_file()
            && let Err(e) = scopecreep::settings::save(&path, &self.hotkeys.to_text())
        {
            eprintln!("ScopeCreep: cannot remember the hotkeys: {e}");
        }
    }

    /// Left button with a bar menu open: a click on a title opens or closes its menu, on an entry does what it
    /// says, on the menu's padding, a heading or a greyed entry nothing; anywhere else closes the menu.
    fn bar_click(&mut self) {
        if let Some(title) = self.bar_title_at_pointer() {
            self.bar.click_title(title);
            self.reset_bar_menus();
            return;
        }
        let Some(menu) = self.bar_menu() else {
            return self.close_bar();
        };
        let Some(rect) = self.bar_menu_rect(&menu) else {
            return self.close_bar();
        };
        if !components::contains(rect, self.cursor) {
            return self.close_bar();
        }
        let action = menu
            .drop
            .hit(rect, self.cursor, self.scale)
            .and_then(|entry| menu.action(entry));
        // Any click in the menu ends a name being typed, keeping it.
        self.finish_rename(true);
        match action {
            Some(MenuAction::Run(Command::AddPanel)) => {
                self.edge_add = None;
                self.layout_mode = LayoutMode::Add;
            }
            Some(MenuAction::Run(command)) => {
                self.run(command);
                self.close_bar();
            }
            Some(MenuAction::Add(offered)) => {
                let (panel, made) = self.instance_to_add(offered);
                self.add_panel(panel);
                if made {
                    self.panels_changed();
                }
                self.close_bar();
            }
            Some(MenuAction::Mode(mode)) => self.layout_mode = mode,
            Some(MenuAction::Page(page)) => {
                self.close_bar();
                self.go_to_page(page);
            }
            Some(MenuAction::NewPage(template)) => {
                self.close_bar();
                self.new_page(template);
            }
            Some(MenuAction::RenamePage) => {
                self.close_bar();
                self.renaming = PageRename::start(&self.pages, self.pages.current());
            }
            Some(MenuAction::OpenHotkeys) => self.hotkeys_panel = Some(HotkeysPanel::default()),
            Some(MenuAction::Hotkeys(action)) => {
                if let Some(panel) = self.hotkeys_panel.as_mut()
                    && panel.click(action, &mut self.hotkeys)
                {
                    self.save_hotkeys();
                }
            }
            None => {}
        }
        self.window.request_redraw();
    }

    fn close_menu(&mut self) {
        self.menu = None;
        self.window.request_redraw();
    }

    /// End a rename being typed: keep the name (`accept`) or drop it.
    fn finish_rename(&mut self, accept: bool) {
        if let Some(rename) = self.renaming.take()
            && accept
            && rename.accept(&mut self.pages)
        {
            self.save_grid();
        }
    }

    /// Show page `page` (Ctrl+number, a tab or the Layout menu): its own tree, focus and undo; only its Panels
    /// measure and draw.
    fn go_to_page(&mut self, page: usize) {
        if self.pages.go(page) {
            self.full = None;
            self.panels_changed();
        }
    }

    /// Which Panels a page other than the shown one holds, by Panel.
    fn held_elsewhere(&self) -> Vec<bool> {
        (0..self.displays.len())
            .map(|p| self.pages.holds_elsewhere(p))
            .collect()
    }

    /// The default layout's Panels, each its own: a first instance another page holds is not shared, a new
    /// instance of that display is made for this page.
    fn default_tree(&self) -> LayoutTree {
        let held = self.held_elsewhere();
        pages::default_tree(&self.displays, |p| held.get(p).copied().unwrap_or(false))
    }

    /// A new page from a template, shown (ADR 0041). Past the limit nothing is made and a notice says so.
    fn new_page(&mut self, template: Template) {
        // Past the limit, before any display is made for the page: a refused page leaves nothing behind.
        if self.pages.is_full() {
            self.layout_notice = Some(format!(
                "A Window holds at most {} pages.",
                pages::MAX_PAGES
            ));
            self.window.request_redraw();
            return;
        }
        let made = match template {
            Template::Empty => self.pages.new_empty(self.displays.len()),
            Template::Default => {
                let tree = self.default_tree();
                self.pages.new_with(tree)
            }
            Template::From(t) => {
                // Each Panel its own: the display's first instance unless another page holds it, then a new one.
                let keys = self.displays.keys();
                let names = self.displays.names();
                let panels: Vec<usize> = t
                    .displays()
                    .iter()
                    .filter_map(|d| {
                        let first = (0..keys.len()).find(|&i| keys[i] == *d && names[i] == *d)?;
                        if self.pages.holds_elsewhere(first) {
                            self.displays.make_named(d).or(Some(first))
                        } else {
                            Some(first)
                        }
                    })
                    .collect();
                let items = self.displays.items();
                let aspect = auto_resize::aspect_of(self.texts.lock().unwrap().frame);
                let (area, gap, scale) = (self.grid_area, self.grid_gap, self.scale);
                let tree = t.tree(&panels, &items, aspect, area, gap, scale);
                self.pages.new_named(t.name(), tree)
            }
            Template::Copy => {
                let mut settings = self.settings.lock().unwrap();
                let displays = &self.displays;
                self.pages.copy_current(|panel| {
                    let Some(copy) = displays.duplicate(panel) else {
                        return panel;
                    };
                    settings.copy_panel(&displays.key(panel), &displays.key(copy));
                    copy
                })
            }
        };
        if made {
            self.full = None;
            self.layout_notice = None;
            self.panels_changed();
        } else {
            self.layout_notice = Some(format!(
                "A Window holds at most {} pages.",
                pages::MAX_PAGES
            ));
            self.window.request_redraw();
        }
    }

    /// Delete the current page (never the last); the Panels only it held are forgotten.
    fn delete_page(&mut self) {
        if self.pages.delete(self.pages.current()) {
            self.full = None;
            self.panels_changed();
        }
    }

    /// The Page bar as it reads now.
    fn page_bar(&self) -> PageBar {
        PageBar {
            names: self.pages.names(),
            current: self.pages.current(),
            rename: self.renaming.clone(),
        }
    }

    /// The Page bar's part under the pointer.
    fn page_bar_hit(&self) -> Option<BarPart> {
        self.page_bar().hit(
            page_bar::top(self.scale),
            self.config.width as f32,
            self.cursor,
            self.scale,
        )
    }

    /// Where the menu a click on `part` opens drops from.
    fn bar_anchor_of(&self, part: BarPart) -> Option<[f32; 4]> {
        self.page_bar().anchor(
            part,
            page_bar::top(self.scale),
            self.config.width as f32,
            self.scale,
        )
    }

    /// A click on the Page bar: a tab goes to its page (a second click on it soon after renames it in place), **+**
    /// offers New page from, Add panel opens the Add list.
    fn page_bar_click(&mut self, part: BarPart) {
        self.menu = None;
        self.hover = None;
        match part {
            BarPart::Tab(page) => {
                let double = self.last_tab_click.is_some_and(|(p, at)| {
                    p == page && at.elapsed() < std::time::Duration::from_millis(400)
                });
                if double {
                    self.last_tab_click = None;
                    self.go_to_page(page);
                    self.renaming = PageRename::start(&self.pages, page);
                } else {
                    self.last_tab_click = Some((page, std::time::Instant::now()));
                    self.go_to_page(page);
                }
            }
            BarPart::Plus => {
                self.bar.open = Some(menu_bar::LAYOUT);
                self.reset_bar_menus();
                self.layout_mode = LayoutMode::NewFrom;
                self.bar_anchor = self.bar_anchor_of(part);
            }
            BarPart::AddPanel => {
                self.open_add_list();
                self.bar_anchor = self.bar_anchor_of(part);
            }
            BarPart::AutoResize => self.run(Command::AutoResize),
            BarPart::AutoLayout => self.run(Command::AutoLayout),
        }
        self.window.request_redraw();
    }

    /// A key: a hotkey does its command (by default Ctrl+1 to 9 shows a layout and Ctrl+Shift+1 to 9 saves one;
    /// the Window is still, nothing shows); while a layout name is being typed, the keys type it. winit gives keys
    /// only to the focused window, so the app never takes keys from Resolve.
    fn key(&mut self, event: &winit::event::KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let pressed = match event.physical_key {
            PhysicalKey::Code(code) if !self.modifiers.super_key() => {
                Key::from_code(code).map(|key| Hotkey {
                    key,
                    ctrl: self.modifiers.control_key(),
                    shift: self.modifiers.shift_key(),
                    alt: self.modifiers.alt_key(),
                })
            }
            _ => None,
        };
        // The Hotkeys panel's key-capture field takes every combination (a modifier alone waits for its key).
        if let Some(panel) = self.hotkeys_panel.as_mut().filter(|p| p.takes_keys()) {
            if let Some(pressed) = pressed {
                if panel.key(pressed, &mut self.hotkeys) {
                    self.save_hotkeys();
                }
                self.window.request_redraw();
            }
            return;
        }
        if self.renaming.is_some() {
            match &event.logical_key {
                LogicalKey::Named(NamedKey::Enter) => self.finish_rename(true),
                LogicalKey::Named(NamedKey::Escape) => self.renaming = None,
                LogicalKey::Named(NamedKey::Backspace) => {
                    if let Some(r) = self.renaming.as_mut() {
                        r.backspace();
                    }
                }
                _ => {
                    if let (Some(r), Some(text)) = (self.renaming.as_mut(), event.text.as_ref()) {
                        r.type_text(text);
                    }
                }
            }
            self.window.request_redraw();
            return;
        }
        // A value box being typed into takes the keys (Escape cancels it, it does not close the popover).
        if self
            .menu
            .as_ref()
            .is_some_and(|(m, _)| m.editing().is_some())
        {
            let big = self.modifiers.shift_key();
            let key = match &event.logical_key {
                LogicalKey::Named(NamedKey::Enter) => Some(EditKey::Enter),
                LogicalKey::Named(NamedKey::Escape) => Some(EditKey::Escape),
                LogicalKey::Named(NamedKey::Backspace) => Some(EditKey::Backspace),
                LogicalKey::Named(NamedKey::ArrowUp) => Some(EditKey::Up(big)),
                LogicalKey::Named(NamedKey::ArrowDown) => Some(EditKey::Down(big)),
                _ => None,
            };
            let typed: Vec<EditKey> = match key {
                Some(k) => vec![k],
                None => event
                    .text
                    .as_ref()
                    .map(|t| t.chars().map(EditKey::Char).collect())
                    .unwrap_or_default(),
            };
            for k in typed {
                let change = self.menu.as_mut().and_then(|(m, _)| m.key(k));
                if let Some(change) = change {
                    let panel = self.menu.as_ref().map_or(0, |(m, _)| m.panel);
                    self.apply_change(panel, change);
                }
            }
            self.window.request_redraw();
            return;
        }
        if event.logical_key == LogicalKey::Named(NamedKey::Escape) {
            if self.panel_drag.take().is_some() {
                // Escape cancels a drag: the layout is as it was.
                self.panel_drag_live = false;
                self.window.set_cursor(CursorIcon::Default);
                self.window.request_redraw();
            } else if self.bar.open.is_some() {
                self.close_bar();
            } else if self.menu.is_some() {
                self.close_menu();
            } else if self.full.take().is_some() {
                self.window.request_redraw();
            }
            return;
        }
        if let Some(command) = pressed.and_then(|h| self.hotkeys.command_for(h)) {
            self.run(command);
        }
    }

    /// Do a command, from its hotkey or its menu entry.
    fn run(&mut self, command: Command) {
        // Showing a page, the default layout or an undone one brings the grid back.
        if matches!(
            command,
            Command::DefaultLayout | Command::GoToPage(_) | Command::UndoLayout
        ) {
            self.full = None;
        }
        self.layout_notice = None;
        match command {
            Command::AddPanel => {
                self.open_add_list();
                return;
            }
            Command::ClosePanel => {
                if let Some(panel) = self.pages.tree().focused() {
                    self.close_panel(panel);
                }
                return;
            }
            Command::FillWindow => {
                self.full = match self.full {
                    Some(_) => None,
                    None => self
                        .cells
                        .iter()
                        .position(|c| Menu::contains(*c, self.cursor)),
                };
                self.window.request_redraw();
                return;
            }
            Command::DefaultLayout => {
                let tree = self.default_tree();
                self.pages.change(|g| *g = tree);
            }
            Command::GoToPage(page) => {
                self.go_to_page(page);
                return;
            }
            Command::NewPage => {
                self.new_page(Template::Empty);
                return;
            }
            Command::DuplicatePage => {
                self.new_page(Template::Copy);
                return;
            }
            Command::DeletePage => {
                self.delete_page();
                return;
            }
            Command::UndoLayout => {
                self.pages.undo();
            }
            Command::AutoLayout => {
                // The current page only, once, for the footage on screen (16:9 with no Feed): one undo step. With
                // no arrangement that fits, the page stays as it was and a notice names the least size.
                let items = self.displays.items();
                let aspect = auto_resize::aspect_of(self.texts.lock().unwrap().frame);
                let (area, gap, scale) = (self.grid_area, self.grid_gap, self.scale);
                let mut result = Ok(());
                self.pages
                    .change(|g| result = g.auto_layout(&items, aspect, area, gap, scale));
                self.layout_notice = result.err().map(|no_fit| no_fit.notice());
            }
            Command::AutoResize => {
                // The current page only, once, for the footage on screen (16:9 with no Feed): one undo step.
                let shapes = self.displays.shapes();
                let aspect = auto_resize::aspect_of(self.texts.lock().unwrap().frame);
                let (area, gap, scale) = (self.grid_area, self.grid_gap, self.scale);
                self.pages
                    .change(|g| g.auto_resize(&shapes, aspect, area, gap, scale));
            }
            Command::ZoomFit => {
                // The Panel under the pointer; over a Scope, or off the Panels, nothing.
                if let Some(panel) = self
                    .contents
                    .iter()
                    .position(|c| Menu::contains(*c, self.cursor))
                    && self
                        .displays
                        .toggle_fit(panel, self.contents[panel], self.scale)
                {
                    self.window.request_redraw();
                }
                return;
            }
            Command::ToggleLegends => {
                self.displays
                    .toggle_legends(&mut self.settings.lock().unwrap());
                self.save_settings();
                self.window.request_redraw();
                return;
            }
        }
        self.panels_changed();
    }

    /// What the Add list offers: each display once, whatever instances there are (ADR 0038, #206).
    fn add_entries(&self) -> Vec<AddEntry> {
        let names = self.displays.names();
        pages::add_offers(&names)
            .into_iter()
            .map(|panel| AddEntry {
                panel,
                name: names[panel].to_string(),
                enabled: true,
            })
            .collect()
    }

    /// The instance an Add of the display `offered` stands for puts on the page: one no page holds, else a new one.
    /// Returns it and whether it was made now.
    fn instance_to_add(&self, offered: usize) -> (usize, bool) {
        let names = self.displays.names();
        match pages::instance_to_add(&names, offered, |i| self.pages.holds(i)) {
            Some(i) => (i, false),
            None => (
                self.displays.make_named(names[offered]).unwrap_or(offered),
                true,
            ),
        }
    }

    /// Open the Add list, dropped from the Layout title.
    fn open_add_list(&mut self) {
        self.edge_add = None;
        self.menu = None;
        self.hover = None;
        self.finish_rename(true);
        self.hotkeys_panel = None;
        self.bar.open = Some(menu_bar::LAYOUT);
        self.layout_mode = LayoutMode::Add;
        self.window.request_redraw();
    }

    /// Add Panel `panel` beside the focused one, and remember the layout. With no room the layout stays as it was
    /// and a notice names the least size of a Panel.
    fn add_panel(&mut self, panel: usize) {
        self.full = None;
        let (area, gap, min) = (self.grid_area, self.grid_gap, min_panel_size(self.scale));
        let added = match self.edge_add.take() {
            Some((target, edge)) => self
                .pages
                .change(|g| g.add_at_edge(panel, target, edge, area, gap, min)),
            None => self.pages.change(|g| g.add(panel, area, gap, min)),
        };
        self.layout_notice = added.notice();
        self.save_grid();
        self.window.request_redraw();
    }

    /// Close Panel `panel`: its sibling takes the space and everything kept for it is forgotten. The layout is
    /// remembered.
    fn close_panel(&mut self, panel: usize) {
        self.full = None;
        self.layout_notice = None;
        {
            let mut settings = self.settings.lock().unwrap();
            let held = self.held_elsewhere();
            self.pages.change(|g| {
                page_edit::close(&self.displays, g, &mut settings, panel, |p| {
                    held.get(p).copied().unwrap_or(false)
                })
            });
        }
        self.panels_changed();
    }

    /// The space the Panels were last laid out in, for Duplicate's room check.
    fn room(&self) -> Room {
        Room {
            area: self.grid_area,
            gap: self.grid_gap,
            min: min_panel_size(self.scale),
        }
    }

    /// Duplicate Panel `panel` to its right or below, with a copy of its settings: one undo step. With no room the
    /// layout stays as it was and a notice names the least size of a Panel.
    fn duplicate_panel(&mut self, panel: usize, dir: Dir) {
        self.full = None;
        let room = self.room();
        let result = {
            let mut settings = self.settings.lock().unwrap();
            self.pages.change(|g| {
                page_edit::duplicate(&self.displays, g, &mut settings, panel, dir, room)
            })
        };
        match result {
            Ok(_) => {
                self.layout_notice = None;
                self.panels_changed();
            }
            Err(refused) => {
                self.layout_notice = Some(refused.notice());
                self.window.request_redraw();
            }
        }
    }

    /// Replace Panel `panel` with the display called `name`, in its place and size, on that display's defaults: one
    /// undo step.
    fn replace_panel(&mut self, panel: usize, name: &str) {
        self.full = None;
        self.layout_notice = None;
        let replaced = {
            let mut settings = self.settings.lock().unwrap();
            let held = self.held_elsewhere();
            self.pages.change(|g| {
                page_edit::replace(&self.displays, g, &mut settings, panel, name, |p| {
                    held.get(p).copied().unwrap_or(false)
                })
            })
        };
        if replaced.is_some() {
            self.panels_changed();
        }
    }

    /// Remove the further instances that are not on the page any more (closed, replaced, or left off by an undo or
    /// another layout), keeping the undo history's Panel indices right.
    fn sweep_panels(&mut self) {
        let mut settings = self.settings.lock().unwrap();
        self.pages.sweep(&self.displays, &mut settings);
        self.pages.show_on(&self.displays);
    }

    /// The Panels changed (made, removed or replaced): whatever the window kept by Panel index is dropped or made
    /// again for the new set, and the layout and settings are remembered.
    fn panels_changed(&mut self) {
        self.sweep_panels();
        let count = self.displays.len();
        while self.panel_texts.len() < count {
            self.panel_texts.push(PanelText {
                label: Text::new(&mut self.font_system),
                tags: Text::new(&mut self.font_system),
                scale: Vec::new(),
            });
        }
        self.panel_texts.truncate(count);
        self.pointer_panel = None;
        self.middle_panel = None;
        self.panel_drag = None;
        self.drag = None;
        self.hover = None;
        self.save_grid();
        self.save_settings();
        self.window.request_redraw();
    }

    fn menu_rect(&self) -> Option<[f32; 4]> {
        let (menu, at) = self.menu.as_ref()?;
        Some(menu.rect(
            *at,
            [self.config.width as f32, self.config.height as f32],
            self.scale,
        ))
    }

    /// Left button while a menu is open: a click on a choice picks it (and remembers it); a click on a heading or
    /// the menu's padding leaves it open; any other click closes it.
    fn menu_click(&mut self) {
        let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) else {
            return;
        };
        let (panel, cursor, scale) = (menu.panel, self.cursor, self.scale);
        // A press on a slider takes it (the menu stays open while it is dragged), on an arrow steps, on the number
        // types, on a reset icon goes back to the default; the Panel reset puts every setting back.
        let big = self.modifiers.shift_key();
        let pressed = match self.menu.as_mut() {
            Some((menu, _)) => menu.press(rect, cursor, scale, big),
            None => return,
        };
        for change in &pressed.changes {
            self.apply_change(panel, *change);
        }
        if pressed.reset_panel {
            let name = self.displays.key(panel);
            self.settings.lock().unwrap().reset_settings(&name);
            self.save_settings();
            self.refresh_menu();
        }
        if pressed.consumed {
            self.window.request_redraw();
            return;
        }
        let Some((menu, _)) = self.menu.as_ref() else {
            return;
        };
        if menu.close_at(rect, cursor, scale) {
            self.menu = None;
            self.close_panel(panel);
            return;
        }
        if let Some(action) = menu.panel_action_at(rect, cursor, scale) {
            let at = self.menu.as_ref().map_or(cursor, |(_, at)| *at);
            match action {
                PanelAction::ReplaceWith => {
                    let names = page_edit::replace_choices(&self.displays, panel);
                    self.menu = Some((Menu::replace_list(panel, &names), at));
                    self.window.request_redraw();
                    return;
                }
                PanelAction::Back => self.open_popover(panel, at),
                PanelAction::ReplaceBy(name) => {
                    self.menu = None;
                    self.replace_panel(panel, &name);
                }
                PanelAction::DuplicateRight => {
                    self.menu = None;
                    self.duplicate_panel(panel, Dir::Row);
                }
                PanelAction::DuplicateBelow => {
                    self.menu = None;
                    self.duplicate_panel(panel, Dir::Col);
                }
            }
            self.window.request_redraw();
            return;
        }
        let choice = menu.choice_at(rect, cursor, scale);
        if let Some((key, value)) = choice {
            self.choose(panel, key, value);
        }
        if choice.is_none() && Menu::contains(rect, cursor) {
            return;
        }
        self.menu = None;
        self.window.request_redraw();
    }

    /// Write the Panel settings to their file. The text is made under the lock and written after it is let go, so
    /// the Feed thread never waits for the disk.
    fn save_settings(&self) {
        let Some(path) = settings_file() else { return };
        let text = self.settings.lock().unwrap().to_text();
        if let Err(e) = scopecreep::settings::save(&path, &text) {
            eprintln!("ScopeCreep: cannot remember the Panel settings: {e}");
        }
    }

    /// Give a display the pointer: what it asks to remember is kept with the Panel settings (and written to the
    /// file once the movement is done), and the window draws again. Returns whether the display took the event.
    fn display_pointer(&mut self, panel: usize, event: Pointer) -> bool {
        let Some(area) = self.contents.get(panel).copied() else {
            return false;
        };
        let result = self.displays.pointer(panel, area, self.scale, event);
        if result.used {
            let name = self.displays.key(panel);
            {
                let mut settings = self.settings.lock().unwrap();
                for (key, value) in &result.remember {
                    self.state_unsaved |= settings.set_state(&name, key, value);
                }
            }
            // A movement is written once it is done, and only if something changed.
            if self.state_unsaved && !matches!(event, Pointer::Move(_)) {
                self.state_unsaved = false;
                self.save_settings();
            }
            self.window.request_redraw();
        }
        result.used
    }

    /// The scroll wheel: over a display that uses it (a Loupe's zoom), else nothing.
    fn wheel(&mut self, lines: f32) {
        if self.menu.is_some() || self.bar.open.is_some() {
            return;
        }
        let panel = self
            .contents
            .iter()
            .position(|c| Menu::contains(*c, self.cursor))
            .filter(|p| self.displays.takes_wheel(*p, self.contents[*p], self.scale));
        if let Some(panel) = panel {
            self.display_pointer(
                panel,
                Pointer::Wheel {
                    at: self.cursor,
                    lines,
                },
            );
        }
    }

    /// Middle button: the Panel under the pointer gets the press, the movement and the release, as the left button
    /// does for a display that uses it. A menu open over it takes no middle button.
    fn middle_button(&mut self, state: ElementState) {
        match state {
            ElementState::Released => {
                if let Some(panel) = self.middle_panel.take() {
                    self.display_pointer(panel, Pointer::MiddleUp);
                }
            }
            ElementState::Pressed => {
                if self.menu.is_some() || self.bar.open.is_some() {
                    return;
                }
                let panel = self
                    .contents
                    .iter()
                    .position(|c| Menu::contains(*c, self.cursor));
                if let Some(panel) = panel
                    && self.display_pointer(panel, Pointer::MiddleDown(self.cursor))
                {
                    self.middle_panel = Some(panel);
                }
            }
        }
    }

    fn button(&mut self, state: ElementState) {
        self.left_down = state == ElementState::Pressed;
        // Letting go always ends a drag, even with a menu open over it (a right click can open one meanwhile).
        if state == ElementState::Released {
            if let Some((menu, _)) = self.menu.as_mut() {
                menu.release();
            }
            if let Some(panel) = self.pointer_panel.take() {
                self.display_pointer(panel, Pointer::Up);
            }
            if self.drag.take().is_some() {
                self.pages.end_drag();
                self.save_grid();
            }
            if let Some(from) = self.panel_drag.take() {
                let live = std::mem::take(&mut self.panel_drag_live);
                // A drag let go on another Panel: Shift swaps the two, else the outer quarter of an edge docks on
                // it. Anywhere else (or a press that never became a drag) changes nothing.
                let tabs_of_from = self
                    .pages
                    .tree()
                    .tab_group(from)
                    .map_or(0, |g| g.tabs.len());
                let over = self
                    .pages
                    .tree()
                    .panel_at(self.grid_area, self.grid_gap, self.cursor)
                    // A Panel dropped on itself does nothing, unless it is a tab: dragged out to an edge it docks.
                    .filter(|to| live && (*to != from || tabs_of_from > 1));
                if !live && self.panel_drag_tab {
                    // A click on a tab shows that Panel.
                    self.layout_notice = None;
                    self.pages.tree_mut().show(from);
                }
                if let Some(to) = over {
                    self.layout_notice = None;
                    if self.modifiers.shift_key() {
                        self.pages.change(|g| g.swap(from, to));
                        self.save_grid();
                    } else {
                        match dock::zone_at(self.cells[to], self.cursor) {
                            Zone::Edge(edge) => {
                                let (area, gap, min) =
                                    (self.grid_area, self.grid_gap, min_panel_size(self.scale));
                                let placed = self
                                    .pages
                                    .change(|g| g.dock(from, to, edge, area, gap, min));
                                self.layout_notice = placed.dock_notice();
                                self.save_grid();
                            }
                            Zone::Middle => {
                                self.pages.change(|g| g.join_tab(from, to));
                                self.save_grid();
                            }
                        }
                    }
                }
                self.window.set_cursor(CursorIcon::Default);
                self.window.request_redraw();
            }
            return;
        }
        // The menu bar: with one of its menus open, the click is the menu's; on a title, it opens that menu (and
        // closes a Panel's popover).
        if self.bar.open.is_some() {
            self.bar_click();
            return;
        }
        // A Panel's popover pushed up over the bar keeps its own clicks.
        if self.menu.is_some()
            && self
                .menu_rect()
                .is_some_and(|r| Menu::contains(r, self.cursor))
        {
            self.menu_click();
            return;
        }
        if let Some(title) = self.bar_title_at_pointer() {
            self.menu = None;
            self.hover = None;
            self.bar.click_title(title);
            self.reset_bar_menus();
            return;
        }
        // The Page bar: a click on the tab being renamed stays in it, any other ends the rename.
        let page_part = self.page_bar_hit();
        if let (Some(BarPart::Tab(page)), Some(rename)) = (page_part, &self.renaming)
            && rename.page == page
        {
            return;
        }
        self.finish_rename(true);
        if let Some(part) = page_part {
            self.page_bar_click(part);
            return;
        }
        if let Some(part) = self.strip_hit() {
            self.menu = None;
            self.hover = None;
            match part {
                StripPart::Fit => self.fit_all(),
                StripPart::Link => self.set_link(!self.displays.link_on()),
            }
            return;
        }
        if self.menu.is_some() {
            self.menu_click();
            return;
        }
        match state {
            ElementState::Pressed => {
                self.layout_notice = None;
                // The empty page's one button.
                if let Some(button) = self.add_button
                    && components::contains(button, self.cursor)
                {
                    self.open_add_list();
                    self.bar_anchor = Some(button);
                    return;
                }
                // The + on a hovered Panel's edge opens the Add list for that side.
                if self.full.is_none()
                    && let Some(panel) =
                        self.pages
                            .tree()
                            .panel_at(self.grid_area, self.grid_gap, self.cursor)
                    && let Some(edge) = self
                        .cells
                        .get(panel)
                        .and_then(|c| components::edge_plus_at(*c, self.cursor, self.scale))
                {
                    self.open_add_list();
                    self.edge_add = Some((panel, edge));
                    self.bar_anchor = components::edge_plus_rects(self.cells[panel], self.scale)
                        .into_iter()
                        .find(|(e, _)| *e == edge)
                        .map(|(_, r)| r);
                    return;
                }
                // A click on any Panel focuses it.
                if let Some(panel) =
                    self.pages
                        .tree()
                        .panel_at(self.grid_area, self.grid_gap, self.cursor)
                {
                    self.pages.tree_mut().set_focus(panel);
                    self.window.request_redraw();
                }
                // A chip or the settings button on a title bar takes the press before the bar can be dragged.
                match self.toolbar_hit() {
                    Some(ToolbarHit::Chip(panel, chip)) => {
                        if let Some(action) = self
                            .toolbars
                            .get(panel)
                            .and_then(|t| t.action(chip))
                            .cloned()
                        {
                            match action {
                                ChipAction::Set(key, value) => self.choose(panel, key, &value),
                            }
                        }
                        return;
                    }
                    Some(ToolbarHit::Settings(panel)) => {
                        if let Some(button) = self.title_layouts.get(panel).and_then(|l| l.settings)
                        {
                            self.open_popover(panel, [button[0], button[1] + button[3]]);
                            self.window.request_redraw();
                        }
                        return;
                    }
                    None => {}
                }
                // A display that uses the pointer (a Loupe) gets it first; else a border may be grabbed.
                let panel = self
                    .contents
                    .iter()
                    .position(|c| Menu::contains(*c, self.cursor))
                    .filter(|p| self.displays.takes_pointer(*p));
                if let Some(panel) = panel
                    && self.display_pointer(panel, Pointer::Down(self.cursor))
                {
                    self.pointer_panel = Some(panel);
                    return;
                }
                // A tab: a click shows it, a drag takes it out to dock elsewhere (the others stay).
                if self.full.is_none()
                    && let Some(hit) = self.tab_hit()
                {
                    match hit {
                        (_, tab_strip::TabHit::Tab(panel)) => {
                            self.panel_drag = Some(panel);
                            self.panel_drag_tab = true;
                            self.panel_drag_start = self.cursor;
                            self.panel_drag_live = false;
                            self.window.set_cursor(CursorIcon::Grabbing);
                        }
                        (shown, tab_strip::TabHit::Overflow) => {
                            if let Some(Some((strip, area))) = self.tab_strips.get(shown) {
                                let group = self.pages.tree().tab_group(shown);
                                let layout = strip.layout(*area, self.scale);
                                if let (Some(group), Some(next)) =
                                    (group, strip.next_hidden(&layout.hidden))
                                {
                                    self.pages.tree_mut().show(group.tabs[next]);
                                }
                            }
                        }
                    }
                    self.window.request_redraw();
                    return;
                }
                match self.press_under_pointer() {
                    Some(Press::Border(border)) => {
                        self.pages.begin_drag();
                        self.drag = Some(border);
                    }
                    Some(Press::Panel(panel)) => {
                        self.panel_drag = Some(panel);
                        self.panel_drag_tab = false;
                        self.panel_drag_start = self.cursor;
                        self.panel_drag_live = false;
                        self.window.set_cursor(CursorIcon::Grabbing);
                    }
                    None => {}
                }
            }
            ElementState::Released => {}
        }
    }

    fn draw(&mut self) {
        let scale = self.window.scale_factor() as f32;
        self.scale = scale;
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let margin = MARGIN * scale;
        let texts = self.texts.lock().unwrap().clone();
        // Along the bottom, under the Panels: the status line, and above it, while a picture is on screen, a
        // notice in the same quiet size (it never covers the picture).
        let notice_text = self.layout_notice.clone().or_else(|| texts.notice.clone());
        let notice_in_status =
            notice_text.is_some() && (texts.has_picture || self.layout_notice.is_some());
        let row = style::SPACE_STATUS_ROW * scale;
        let status_height = row * if notice_in_status { 2.0 } else { 1.0 };
        // The Panels fill the window above the status line, with a margin round them and between them.
        // The Colour Setup is said once, in the bar across the top.
        // Over it all, the thin menu bar (#147).
        let bar_height = MenuBar::height(scale);
        let top_height = BarKind::Top.height(scale);
        let page_bar_height = PageBar::height(scale);
        let area_top = bar_height + top_height + page_bar_height + margin;
        let area = [
            margin,
            area_top,
            (width - 2.0 * margin).max(1.0),
            (height - status_height - area_top).max(1.0),
        ];
        let cells = panel_grid::cells_filled(self.pages.tree(), area, margin, self.full);
        // A number shows only where the page holds two or more Panels of a display.
        let shown_names = pages::title_names(
            &self.displays.names(),
            &self.displays.keys(),
            &self.pages.tree().panels(),
        );
        self.cells = cells.clone();
        self.grid_area = area;
        self.grid_gap = margin;

        let line = style::TYPE_LABEL * scale * style::TYPE_LABEL_LINE;
        // Each Panel: a thin border, its label on one line in the title bar (clipped to it), the display under
        // the bar, and the numbers on the display's scale.
        let mut panels = Vec::new();
        // Where each name and its tags sit in the title bar (the tags go to the hover when they do not fit).
        let mut title_parts: Vec<([f32; 4], Option<[f32; 4]>)> = Vec::new();
        self.title_hovers.clear();
        let settings_now = self.settings.lock().unwrap().clone();
        let mut toolbars: Vec<Toolbar> = Vec::new();
        let mut layouts: Vec<components::TitleLayout> = Vec::new();
        let mut tab_strips: Vec<Option<(tab_strip::TabStrip, [f32; 4])>> = Vec::new();
        // Only the shown Panel of each tab group measures and draws (ADR 0038).
        self.pages.show_on(&self.displays);
        let max_chars = ((width / (style::SPACE_CHAR_WIDTH * scale)) as usize)
            .saturating_sub(8)
            .clamp(40, 110);
        for (i, cell) in cells.iter().enumerate() {
            let mut title = texts.titles.get(i).cloned().unwrap_or_default();
            if let Some(shown) = shown_names.get(i) {
                title.name.clone_from(shown);
            }
            let panel = PanelLayout::new(*cell, scale);
            // A picture Panel keeps its zoom as a read-only tag with its other tags (ADR 0040).
            if let Some(zoom) = self.displays.zoom_tag(i, panel.content, scale) {
                title.tags.push(zoom);
            }
            let mut toolbar = self.displays.toolbar(i, &settings_now);
            toolbar.fit_to(panel.title[2], true, scale);
            // Every Panel's popover holds Close, so every title bar has the settings button.
            let has_popover = true;
            let (l, tags, all_shown) = window_text::lay_out_toolbar_title(
                &title,
                &toolbar.chips,
                &toolbar.groups,
                has_popover,
                panel.title,
                scale,
            );
            toolbars.push(toolbar);
            // A tab group of more than one Panel shows its tabs where the name would be.
            let strip = self
                .pages
                .tree()
                .tab_group(i)
                .filter(|g| g.tabs.len() > 1 && panel_grid::shown(*cell) && g.tabs[g.active] == i)
                .map(|g| {
                    let labels = g
                        .tabs
                        .iter()
                        .map(|t| shown_names.get(*t).cloned().unwrap_or_default())
                        .collect();
                    (
                        tab_strip::TabStrip {
                            labels,
                            active: g.active,
                        },
                        tab_strip::room(&l, scale),
                    )
                });
            tab_strips.push(strip);
            self.panel_texts[i].label.set(
                &mut self.font_system,
                &title.name,
                TextStyle {
                    wrap: false,
                    ..TextStyle::LABEL
                },
                scale,
                l.name[2].max(1.0),
            );
            let tags_style = TextStyle {
                size: style::TYPE_SCALE,
                line: style::TYPE_SCALE_LINE,
                align: None,
                wrap: false,
            };
            self.panel_texts[i].tags.set(
                &mut self.font_system,
                &tags,
                tags_style,
                scale,
                l.tags.map_or(1.0, |t| t[2].max(1.0)),
            );
            self.title_hovers.push(window_text::hover_text(
                &title,
                l.name_cut || !all_shown,
                max_chars,
            ));
            title_parts.push((l.name, l.tags));
            layouts.push(l);
            panels.push(panel);
        }
        self.title_bars = panels.iter().map(|p| p.title).collect();
        self.toolbars = toolbars;
        self.title_layouts = layouts;
        self.tab_strips = tab_strips;
        self.contents = panels.iter().map(|p| p.content).collect();
        // Hidden cells have no content, so no zoom to read.
        self.view_strip = self.displays.view_strip(&self.contents, scale);
        let status_width = width - 2.0 * margin;
        // Playback and performance only: the longest choice that fits, whole parts only (never cut mid-word).
        let status_line = window_text::fit_status(&texts.status, status_width, scale);
        self.status.set(
            &mut self.font_system,
            status_line,
            TextStyle {
                wrap: false,
                ..TextStyle::LABEL
            },
            scale,
            status_width,
        );
        self.status_bar = [0.0, height - row, width, row];
        self.status_hover = texts.status_hover.clone();
        let top_line = window_text::fit_parts(
            &texts.top_bar,
            width - 2.0 * style::SPACE_BAR_PAD * scale,
            scale,
        );
        // The Colour Setup bar is drawn with the overlay (its text above its fill, #217).
        // Overlay text draws over every fill, so while a menu is open over the bar the line is left out.
        let bar_covered = self.bar.open.is_some() || self.menu.is_some();
        let top_bar_prims = window_text::top_bar_prims(
            if bar_covered { "" } else { &top_line },
            bar_height,
            width,
            scale,
        );
        let (notice_style, notice_width) = if notice_in_status {
            (TextStyle::LABEL, status_width)
        } else {
            (
                TextStyle::NOTICE,
                (panels
                    .iter()
                    .find(|p| panel_grid::shown(p.frame))
                    .map_or(1.0, |p| p.content[2])
                    - 2.0 * margin)
                    .max(1.0),
            )
        };
        self.notice.set(
            &mut self.font_system,
            notice_text.as_deref().unwrap_or(""),
            notice_style,
            scale,
            notice_width,
        );
        let notice_height: f32 = self
            .notice
            .buffer
            .layout_runs()
            .map(|run| run.line_height)
            .sum();

        let whole = TextBounds {
            left: 0,
            top: 0,
            right: width as i32,
            bottom: height as i32,
        };
        let status_top = height - row + (row - line) / 2.0 - 0.25 * margin;
        let (notice_left, notice_top) = if notice_in_status {
            (margin, status_top - row)
        } else {
            let [left, top, _, height] = panels
                .iter()
                .find(|p| panel_grid::shown(p.frame))
                .map_or([margin, margin, 1.0, 1.0], |p| p.content);
            (
                left + margin,
                (top + (height - notice_height) / 2.0).max(top),
            )
        };
        // The scale's numbers, per Panel, shaped once and reused until they or the room change.
        let scale_mark_line = style::TYPE_SCALE * scale * style::TYPE_SCALE_LINE;
        let mut marks: Vec<(usize, scopecreep::display::ScaleText)> = Vec::new();
        if texts.has_picture {
            for (i, panel) in panels.iter().enumerate() {
                if !panel_grid::shown(panel.frame) {
                    continue;
                }
                for mark in self.displays.scale_texts(i, panel.content, scale) {
                    marks.push((i, mark));
                }
            }
        }
        for (i, panel_text) in self.panel_texts.iter_mut().enumerate() {
            let wanted = marks.iter().filter(|(p, _)| *p == i).count();
            while panel_text.scale.len() < wanted {
                panel_text.scale.push(Text::new(&mut self.font_system));
            }
        }
        let mut used = vec![0usize; self.panel_texts.len()];
        let mut mark_areas = Vec::new();
        for (i, mark) in &marks {
            let n = used[*i];
            used[*i] += 1;
            let style = TextStyle {
                size: style::TYPE_SCALE,
                line: style::TYPE_SCALE_LINE,
                align: if mark.centred {
                    Some(glyphon::cosmic_text::Align::Center)
                } else {
                    mark.right_aligned
                        .then_some(glyphon::cosmic_text::Align::Right)
                },
                wrap: false,
            };
            self.panel_texts[*i].scale[n].set(
                &mut self.font_system,
                &mark.text,
                style,
                scale,
                mark.width,
            );
            mark_areas.push((*i, n, mark.left, mark.top - scale_mark_line / 2.0));
        }

        let mut areas = vec![
            TextArea {
                buffer: &self.status.buffer,
                left: margin,
                top: status_top,
                scale: 1.0,
                bounds: whole,
                default_color: text_colour(style::TEXT_SECONDARY),
                custom_glyphs: &[],
            },
            TextArea {
                buffer: &self.notice.buffer,
                left: notice_left,
                top: notice_top,
                scale: 1.0,
                bounds: whole,
                default_color: text_colour(style::TEXT_PRIMARY),
                custom_glyphs: &[],
            },
        ];
        let tags_line = style::TYPE_SCALE * scale * style::TYPE_SCALE_LINE;
        for (i, ((panel, text), (name, tags))) in panels
            .iter()
            .zip(&self.panel_texts)
            .zip(&title_parts)
            .enumerate()
        {
            if !panel_grid::shown(panel.frame)
                || self.tab_strips.get(i).is_some_and(Option::is_some)
            {
                continue;
            }
            let [left, top, w, h] = panel.title;
            // A label never spills out of its title bar.
            let bounds = TextBounds {
                left: left as i32,
                top: top as i32,
                right: (left + w).ceil() as i32,
                bottom: (top + h).ceil() as i32,
            };
            areas.push(TextArea {
                buffer: &text.label.buffer,
                left: name[0],
                top: top + (h - line) / 2.0,
                scale: 1.0,
                bounds,
                default_color: text_colour(style::TEXT_PRIMARY),
                custom_glyphs: &[],
            });
            if let Some(tags) = tags {
                areas.push(TextArea {
                    buffer: &text.tags.buffer,
                    left: tags[0],
                    top: top + (h - tags_line) / 2.0,
                    scale: 1.0,
                    bounds,
                    default_color: text_colour(style::TEXT_MUTED),
                    custom_glyphs: &[],
                });
            }
        }
        for (i, n, left, top) in mark_areas {
            let c = panels[i].content;
            let bounds = TextBounds {
                left: c[0] as i32,
                top: c[1] as i32,
                right: (c[0] + c[2]).ceil() as i32,
                bottom: (c[1] + c[3]).ceil() as i32,
            };
            areas.push(TextArea {
                buffer: &self.panel_texts[i].scale[n].buffer,
                left,
                top,
                scale: 1.0,
                bounds,
                default_color: text_colour(style::TEXT_MUTED),
                custom_glyphs: &[],
            });
        }
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.config.width,
                height: self.config.height,
            },
        );
        let prepared = self.text_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        );
        if let Err(e) = prepared {
            eprintln!("ScopeCreep: text could not be prepared: {e}");
        }

        // The right-click menu, drawn as the popover component, over everything: its rectangles, then its own text
        // (so it sits above the Panels' text).
        let mut rects: Vec<([f32; 4], [f32; 4])> = Vec::new();
        let mut over_empty: Vec<Prim> = Vec::new();
        // Each Panel's thin border and the line under its title bar.
        for panel in panels.iter().filter(|p| panel_grid::shown(p.frame)) {
            for edge in panel.lines((style::LINE_WEIGHT * scale).max(1.0)) {
                rects.push((edge, style::rgba(style::LINE_BORDER)));
            }
        }
        // The focused Panel has a quiet outline.
        if !self.pages.tree().is_empty()
            && let Some(focused) = self.pages.tree().focused().filter(|f| *f < cells.len())
            && panel_grid::shown(cells[focused])
        {
            for prim in components::focus_outline(cells[focused], scale) {
                if let Prim::Rect { rect, colour } = prim {
                    rects.push((rect, [colour[0], colour[1], colour[2], 1.0]));
                }
            }
        }
        // A page with no Panel: one large Add panel button.
        self.add_button = self
            .pages
            .tree()
            .is_empty()
            .then(|| components::add_panel_button_rect(area, scale));
        if self.add_button.is_some() {
            let state = if self.bar.open.is_none()
                && self.menu.is_none()
                && self
                    .add_button
                    .is_some_and(|b| components::contains(b, self.cursor))
            {
                State::Hover
            } else {
                State::Normal
            };
            over_empty = components::add_panel_button(area, state, scale);
        }
        // While a Panel is dragged, the drop is shown before release, quietly: with Shift the Panel it would swap
        // with is outlined, else the exact half an edge dock would give the dragged Panel.
        // The middle of a tab group is a tab: the whole Panel is outlined and says "Tab here".
        let mut drop_label: Option<(&'static str, [f32; 4])> = None;
        if let Some(outline) = self
            .panel_drag
            .filter(|_| self.panel_drag_live)
            .and_then(|from| {
                let tabs = self
                    .pages
                    .tree()
                    .tab_group(from)
                    .map_or(0, |g| g.tabs.len());
                let to = self
                    .pages
                    .tree()
                    .panel_at(area, margin, self.cursor)
                    .filter(|to| *to != from || tabs > 1)?;
                if self.modifiers.shift_key() {
                    return Some(cells[to]);
                }
                let zone = dock::zone_at(cells[to], self.cursor);
                if zone == Zone::Middle && to == from {
                    return None;
                }
                drop_label = zone.label().map(|l| (l, cells[to]));
                Some(dock::overlay(cells[to], zone, margin))
            })
        {
            let weight = (style::LINE_WEIGHT * scale).max(1.0);
            for edge in PanelLayout::new(outline, scale)
                .lines(weight)
                .iter()
                .take(4)
            {
                rects.push((*edge, style::rgba(style::STATE_FOCUS)));
            }
        }
        let mut menu_areas: Vec<(usize, f32, f32, [u8; 3])> = Vec::new();
        // The menu over the Panels, or else the hover of what the pointer rests on, over everything.
        let mut over: Vec<Prim> = over_empty;
        over.extend(top_bar_prims);
        if let Some((label, cell)) = drop_label {
            over.push(components::text(
                label,
                cell,
                components::Face::Label,
                style::TEXT_PRIMARY,
                Align::Centre,
            ));
        }
        // Each tab group's tabs, the one under the pointer lit.
        let tab_pointer = if self.menu.is_some() || self.bar.open.is_some() {
            None
        } else {
            self.tab_hit()
        };
        for (shown, strip) in self.tab_strips.iter().enumerate() {
            if let Some((strip, at)) = strip {
                let hover = tab_pointer.and_then(|(s, h)| {
                    if s != shown {
                        return None;
                    }
                    let group = self.pages.tree().tab_group(shown)?;
                    Some(match h {
                        tab_strip::TabHit::Tab(p) => {
                            tab_strip::TabHit::Tab(group.tabs.iter().position(|t| *t == p)?)
                        }
                        tab_strip::TabHit::Overflow => tab_strip::TabHit::Overflow,
                    })
                });
                over.extend(strip.draw(*at, hover, scale));
            }
        }
        // The + on each edge of the Panel under the pointer, quiet until then.
        if self.full.is_none()
            && self.menu.is_none()
            && self.bar.open.is_none()
            && self.drag.is_none()
            && self.panel_drag.is_none()
            && let Some(hovered) = self.pages.tree().panel_at(area, margin, self.cursor)
            && let Some(frame) = cells
                .get(hovered)
                .copied()
                .filter(|c| panel_grid::shown(*c) && components::edge_plus_shown(*c, self.cursor))
        {
            let lit = components::edge_plus_at(frame, self.cursor, scale);
            over.extend(components::edge_plus_draw(frame, lit, scale));
        }
        // Each title bar's chips and settings button, lit under the pointer.
        let pointer_over = if self.menu.is_some() || self.bar.open.is_some() {
            None
        } else {
            self.toolbar_hit()
        };
        for (i, (toolbar, l)) in self.toolbars.iter().zip(&self.title_layouts).enumerate() {
            if !self.cells.get(i).is_some_and(|c| panel_grid::shown(*c)) {
                continue;
            }
            for (c, (chip, r)) in toolbar.chips.iter().zip(&l.chips).enumerate() {
                let state = if pointer_over == Some(ToolbarHit::Chip(i, c)) {
                    State::Hover
                } else {
                    State::Normal
                };
                over.extend(chip.draw([r[0], r[1]], toolbar.on[c], state, scale));
            }
            if let Some(button) = l.settings {
                let open = self.menu.as_ref().is_some_and(|(m, _)| m.panel == i);
                let state = if open {
                    State::Pressed
                } else if pointer_over == Some(ToolbarHit::Settings(i)) {
                    State::Hover
                } else {
                    State::Normal
                };
                over.extend(components::settings_button(button, state, scale));
            }
        }
        over.extend(menu_bar::bar().draw(0.0, 0.0, width, self.bar.open, self.bar_hover, scale));
        // The Page bar between the Colour Setup line and the Panels (ADR 0041).
        let page_bar = self.page_bar();
        let page_top = page_bar::top(scale);
        let page_hover = if self.menu.is_some() || self.bar.open.is_some() {
            None
        } else {
            page_bar.hit(page_top, width, self.cursor, scale)
        };
        over.extend(page_bar.draw(page_top, width, page_hover, scale));
        let strip_hover = if self.menu.is_some() || self.bar.open.is_some() {
            None
        } else {
            self.strip_hit()
        };
        over.extend(self.view_strip.draw(width, strip_hover, scale));
        let bar_menu = self
            .bar_menu()
            .and_then(|m| self.bar_menu_rect(&m).map(|r| (m, r)));
        if let Some((menu, rect)) = bar_menu {
            let hover = menu.drop.hit(rect, self.cursor, scale);
            over.extend(menu.drop.draw(rect, hover, self.left_down, scale));
        } else if let (Some((menu, _)), Some(rect)) = (self.menu.as_ref(), self.menu_rect()) {
            over.extend(components::popover(menu, rect, self.cursor, scale));
            over.extend(components::popover_tooltip(
                menu,
                rect,
                self.cursor,
                [width, height],
                scale,
            ));
        } else if let Some(hover) = self.hover {
            let (anchor, text) = match hover {
                Hover::Chip(p, c) => (
                    self.title_layouts
                        .get(p)
                        .and_then(|l| l.chips.get(c))
                        .copied(),
                    self.toolbars
                        .get(p)
                        .and_then(|t| t.tooltips.get(c))
                        .cloned(),
                ),
                Hover::SettingsButton(p) => (
                    self.title_layouts.get(p).and_then(|l| l.settings),
                    Some("Settings".to_string()),
                ),
                Hover::Title(i) => (
                    self.title_bars.get(i).copied(),
                    self.title_hovers.get(i).cloned(),
                ),
                Hover::Strip(part) => {
                    let l = view_strip::ViewStrip::layout(width, scale);
                    match part {
                        StripPart::Fit => (Some(l.fit), Some(view_strip::FIT_TOOLTIP.to_string())),
                        StripPart::Link => {
                            (Some(l.link), Some(view_strip::LINK_TOOLTIP.to_string()))
                        }
                    }
                }
                Hover::Status => (Some(self.status_bar), Some(self.status_hover.clone())),
            };
            if let (Some(anchor), Some(text)) = (anchor, text.filter(|t| !t.is_empty())) {
                let tip = Tooltip::new(&text);
                over.extend(tip.draw(tip.rect(anchor, [width, height], scale), scale));
            }
        }
        let mut texts_used = 0;
        for prim in over {
            match prim {
                Prim::Rect { rect, colour } => rects.push((rect, colour)),
                Prim::Text {
                    text,
                    rect: at,
                    colour,
                    align,
                    ..
                } => {
                    if self.menu_texts.len() <= texts_used {
                        self.menu_texts.push(Text::new(&mut self.font_system));
                    }
                    let align = match align {
                        Align::Left => None,
                        Align::Centre => Some(glyphon::cosmic_text::Align::Center),
                        Align::Right => Some(glyphon::cosmic_text::Align::Right),
                    };
                    self.menu_texts[texts_used].set(
                        &mut self.font_system,
                        &text,
                        TextStyle {
                            wrap: false,
                            align,
                            ..TextStyle::LABEL
                        },
                        scale,
                        at[2],
                    );
                    menu_areas.push((texts_used, at[0], at[1] + (at[3] - line) / 2.0, colour));
                    texts_used += 1;
                }
            }
        }
        self.flat.set([width, height], &rects);
        let menu_text_areas: Vec<TextArea> = menu_areas
            .iter()
            .map(|(i, left, top, colour)| TextArea {
                buffer: &self.menu_texts[*i].buffer,
                left: *left,
                top: *top,
                scale: 1.0,
                bounds: whole,
                default_color: text_colour(*colour),
                custom_glyphs: &[],
            })
            .collect();
        if let Err(e) = self.menu_renderer.prepare(
            &self.device,
            &self.queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            menu_text_areas,
            &mut self.swash_cache,
        ) {
            eprintln!("ScopeCreep: menu text could not be prepared: {e}");
        }

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            _ => {
                // Outdated, suboptimal or lost: set the surface up again and draw on the next wake.
                self.surface.configure(&self.device, &self.config);
                self.window.request_redraw();
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("window"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(style::clear(style::SURFACE_WINDOW)),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for (i, panel) in panels.iter().enumerate() {
                // A Panel hidden while another fills the Window is not drawn (its cell is off screen).
                if !panel_grid::shown(panel.frame) {
                    continue;
                }
                self.displays
                    .draw(&mut pass, i, panel.content, [width, height], scale);
            }
            if let Err(e) = self
                .text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
            {
                eprintln!("ScopeCreep: text could not be drawn: {e}");
            }
            self.flat.draw(&mut pass);
            if let Err(e) = self
                .menu_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
            {
                eprintln!("ScopeCreep: menu text could not be drawn: {e}");
            }
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
        self.atlas.trim();
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.feed_thread.take() {
            let _ = thread.join();
        }
    }
}

impl ApplicationHandler<Wake> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("ScopeCreep")
            .with_inner_size(LogicalSize::new(1280.0, 760.0));
        #[cfg(target_os = "linux")]
        let attributes = {
            use winit::platform::wayland::WindowAttributesExtWayland;
            attributes.with_name("scopecreep", "scopecreep")
        };
        let made = event_loop
            .create_window(attributes)
            .map_err(Into::into)
            .and_then(|window| {
                AppWindow::new(
                    Arc::new(window),
                    event_loop,
                    self.feed.clone(),
                    self.scripting,
                    self.proxy.clone(),
                )
            });
        match made {
            Ok(window) => self.window = Some(window),
            Err(e) => {
                eprintln!("ScopeCreep: cannot open the window: {e}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, _wake: Wake) {
        if let Some(w) = &self.window {
            w.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(w) = &mut self.window else { return };
        match event {
            WindowEvent::Resized(size) => w.resize(size.width, size.height),
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = w.window.inner_size();
                w.resize(size.width, size.height);
            }
            WindowEvent::CursorMoved { position, .. } => {
                w.pointer_moved(position.x as f32, position.y as f32)
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => w.button(state),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => w.right_click(),
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Middle,
                ..
            } => w.middle_button(state),
            WindowEvent::ModifiersChanged(m) => w.modifiers = m.state(),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => w.key(&event),
            WindowEvent::MouseWheel { delta, .. } => w.wheel(match delta {
                MouseScrollDelta::LineDelta(_, y) => y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
            }),
            WindowEvent::CursorLeft { .. } => {
                // Nothing stays lit once the pointer has left: no hover, no bar title, no menu entry.
                w.cursor = [-1.0, -1.0];
                if w.hover.take().is_some() || w.bar_hover.take().is_some() || w.bar.open.is_some()
                {
                    w.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => w.draw(),
            WindowEvent::CloseRequested => {
                w.close();
                event_loop.exit();
            }
            _ => {}
        }
    }
}
