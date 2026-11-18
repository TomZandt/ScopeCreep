//! What the Window's text says (spec #94, ticket #99): a colourist reads only what changes how they act.
//!
//! - The top bar says the Colour Setup once ([`top_bar`]).
//! - Each Panel's title bar says its name and short tags, only for state that changes how to read it
//!   ([`panel_title`]); hovering it shows the full label as the display gives it.
//! - The status line says playback and performance only ([`fit_status`] picks what fits the width); everything
//!   else is its hover.
//!
//! Claims are made by the top bar and the title bar together, never overstated (ADR 0007). Pure: no GPU, no window.
use crate::colour_setup::{KnownColourSetup, Levels};
use crate::display::DisplayContext;
use crate::output_conversion::{Accuracy, Plan, measurement};
use crate::waveform::LumaWeights;
use scopecreep_feed::Provenance;

/// Between the parts of a line.
pub const SEP: &str = "  ·  ";

/// What a Panel's title bar shows, and what hovering it shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelTitle {
    pub name: String,
    pub tags: Vec<String>,
    /// The full label, as the display gives it.
    pub full: String,
}

impl PanelTitle {
    /// The tags as one line.
    pub fn tags_text(&self) -> String {
        self.tags.join(", ")
    }
}

/// The Colour Setup line across the top of the Window: Timeline → Output, Levels, where it came from, and whether
/// Output Conversion is exact or approximated (ADR 0007's words). `unknown` is what to say without one.
pub fn top_bar(known: Option<&KnownColourSetup>, unknown: &str) -> String {
    let Some(known) = known else { return format!("{unknown}{SEP}{}", Plan::Unknown.label()) };
    let s = &known.setup;
    let mut route = format!("{} → {}", s.timeline, s.output_label());
    if let Some(t) = &s.tone_mapping {
        route += &format!(", {t} tone mapping");
    }
    if let Some(g) = &s.gamut_mapping {
        route += &format!(", {g} gamut mapping");
    }
    let levels = match s.levels {
        Some(Levels::Video) => "Video levels",
        Some(Levels::Full) => "Full levels",
        None => "Video levels assumed",
    };
    let mut from = vec![match known.provenance {
        Provenance::Project => "from project".to_string(),
        Provenance::Manual => "set manually".to_string(),
    }];
    if s.is_aces() {
        from.push(match &s.aces_version {
            Some(v) => format!("ACES {v}"),
            None => format!("assumes ACES {}", crate::colour_setup::ASSUMED_ACES_VERSION),
        });
    }
    [route, levels.to_string(), from.join(", "), Plan::for_setup(Some(s)).label()].join(SEP)
}

/// The tags a picture's conversion needs on a Panel: "approximated" when Output Conversion is, "unconverted" when
/// the picture is shown as sent.
pub fn conversion_tags(context: &DisplayContext) -> Vec<String> {
    match Plan::for_setup(context.colour_setup.as_ref().map(|c| &c.setup)) {
        Plan::Convert { accuracy: Accuracy::Approximated(_), .. } => vec!["approximated".into()],
        Plan::Convert { .. } => Vec::new(),
        Plan::Unknown | Plan::Unsupported(_) => vec!["unconverted".into()],
    }
}

/// The tags a Panel that measures Luma and colour needs: the conversion's, the Measured Space when assumed, and
/// the luma weights when they are not the default.
pub fn measurement_tags(context: &DisplayContext) -> Vec<String> {
    let m = measurement(context.colour_setup.clone(), context.luma);
    let mut tags = conversion_tags(context);
    if m.assumed {
        tags.push(format!("{} assumed", m.space.name()));
    }
    if m.luma == LumaWeights::MatchResolve {
        tags.push("luma weights: match Resolve".into());
    }
    tags
}

/// The waveforms' tags: the measurement's, and the scale's Levels when they are not the usual known video range.
fn waveform_tags(context: &DisplayContext) -> Vec<String> {
    let m = measurement(context.colour_setup.clone(), context.luma);
    let mut tags = measurement_tags(context);
    match (m.levels, m.levels_from) {
        (_, None) => tags.push("video levels assumed".into()),
        (Levels::Full, Some(_)) => tags.push("full levels".into()),
        (Levels::Video, Some(_)) => {}
    }
    tags
}

/// A Panel's title: its name and the tags its own measurement needs, under `context` (its own settings, the Colour
/// Setup and the luma weights), with its full label for the hover. Defaults are not repeated.
pub fn panel_title(display_name: &str, context: &DisplayContext, full: String) -> PanelTitle {
    let setting = |key: &str| context.settings.get(key).map(String::as_str);
    let (name, tags): (String, Vec<String>) = match display_name {
        "Clean picture" => ("Clean picture".into(), conversion_tags(context)),
        crate::waveform::NAME => ("Luma waveform".into(), waveform_tags(context)),
        crate::waveform::COLOUR_NAME => ("Colour waveform".into(), waveform_tags(context)),
        "Black and white" => ("Black and white".into(), measurement_tags(context)),
        "ARRI False Colour" => ("ARRI False Colour".into(), measurement_tags(context)),
        crate::vectorscope::NAME => {
            // Both zooms are drawn side by side, so there is no zoom to say.
            let mut tags = measurement_tags(context);
            if setting(crate::vectorscope::TARGETS_SETTING) == Some("100") {
                tags.push("100 % targets".into());
            }
            (crate::vectorscope::NAME.into(), tags)
        }
        "Saturation heatmap" => {
            let mut tags = Vec::new();
            if let Some(scale) = setting(crate::saturation_heatmap::SCALE_SETTING).filter(|s| *s != crate::saturation_heatmap::DEFAULT_SCALE) {
                tags.push(format!("hot {scale} %"));
            }
            tags.extend(measurement_tags(context));
            ("Saturation heatmap".into(), tags)
        }
        "Skin tone" => {
            let mut tags = conversion_tags(context);
            if let Some(degrees) = setting(crate::skin_tone_view::TOLERANCE_SETTING).filter(|d| *d != crate::skin_tone_view::DEFAULT_TOLERANCE) {
                tags.push(format!("±{degrees}°"));
            }
            ("Skin tone".into(), tags)
        }
        "Channel view" => {
            let channel = match setting("channel") {
                Some("red") => "Red",
                Some("green") => "Green",
                Some("luma") => "Luma",
                _ => "Blue",
            };
            let zoom = setting("loupe_zoom").and_then(|z| z.parse::<u32>().ok()).unwrap_or(crate::channel_view::DEFAULT_ZOOM);
            let mut tags = vec![channel.to_string(), format!("{zoom} %")];
            tags.extend(conversion_tags(context));
            // Luma follows the waveform's weights, so a grey that is not the default says so.
            if setting("channel") == Some("luma") && measurement(context.colour_setup.clone(), context.luma).luma == LumaWeights::MatchResolve {
                tags.push("luma weights: match Resolve".into());
            }
            ("Channel view".into(), tags)
        }
        crate::stops_false_colour::NAME => (crate::stops_false_colour::NAME.into(), crate::stops_false_colour::reading_tags(context)),
        other => (other.to_string(), Vec::new()),
    };
    PanelTitle { name, tags, full }
}

/// Room a line of `text` takes at `scale`, the way the components lay text out.
pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * crate::style::SPACE_CHAR_WIDTH * scale
}

/// The status line for `width` device pixels: the first of `candidates` (most complete first, whole parts only, so
/// never cut mid-word) that fits; the last, which holds the rate and the drops, when none does.
pub fn fit_status(candidates: &[String], width: f32, scale: f32) -> &str {
    candidates.iter().find(|c| text_width(c, scale) <= width).or(candidates.last()).map_or("", String::as_str)
}

/// `line` with its last parts (split at [`SEP`]) dropped, whole parts only, until it fits `width` device pixels;
/// the first part is always kept.
pub fn fit_parts(line: &str, width: f32, scale: f32) -> String {
    let parts: Vec<&str> = line.split(SEP).collect();
    (1..=parts.len()).rev().map(|n| parts[..n].join(SEP)).find(|s| text_width(s, scale) <= width).unwrap_or_else(|| parts[0].to_string())
}

/// A panel's hover: what its title bar leaves out first (the name and tags when they were cut), then its full
/// label with one part per line, long parts wrapped at words to `max_chars`.
pub fn hover_text(title: &PanelTitle, name_and_tags_cut: bool, max_chars: usize) -> String {
    let mut lines = Vec::new();
    if name_and_tags_cut {
        lines.push(if title.tags.is_empty() { title.name.clone() } else { format!("{}  ({})", title.name, title.tags_text()) });
    }
    for part in title.full.split(SEP) {
        let mut line = String::new();
        for word in part.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max_chars {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.join("\n")
}

/// A Panel's title bar laid out in `bar` (device pixels) at `scale`, on one line: the name, then as many of the
/// tags (most important first) as fit beside it. Returns the layout and whether everything fit; when not, the
/// hover carries the rest ([`hover_text`]).
pub fn lay_out_title(title: &PanelTitle, bar: [f32; 4], scale: f32) -> (crate::components::TitleLayout, String, bool) {
    lay_out_toolbar_title(title, &[], false, bar, scale)
}

/// The same with the Panel's toolbar: its `chips` and, when it has one, its settings button stay at the right of
/// the bar. When the Panel is small the name truncates first and the tags go to the hover; the chips never move.
pub fn lay_out_toolbar_title(title: &PanelTitle, chips: &[crate::components::Chip], settings_button: bool, bar: [f32; 4], scale: f32) -> (crate::components::TitleLayout, String, bool) {
    use crate::components::TitleBar;
    for n in (0..=title.tags.len()).rev() {
        let tags = title.tags[..n].join(", ");
        let layout = TitleBar { name: title.name.clone(), tags: tags.clone(), chips: chips.to_vec(), settings_button }.layout(bar, scale);
        if layout.name_cut {
            return (layout, String::new(), false);
        }
        if n == 0 || layout.tags.is_some() {
            return (layout, tags, n == title.tags.len());
        }
    }
    unreachable!("the loop returns at n == 0")
}
