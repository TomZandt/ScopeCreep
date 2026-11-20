//! Duplicate, Replace and Close on the page (ADR 0039, #208): what each does to the Panels, the layout tree and
//! the settings together, so the Window only calls them. Pure apart from the displays it makes: no window, no fonts.
use crate::display::DisplaySet;
use crate::layout_tree::{Dir, Edge, LayoutTree, NoRoom};
use crate::settings::PanelSettings;

/// The displays Panel `panel` can be replaced with: every display the Window has, once each, in Panel order, except
/// the one the Panel already is.
pub fn replace_choices(displays: &DisplaySet, panel: usize) -> Vec<String> {
    let own = displays.name(panel);
    let mut names: Vec<String> = Vec::new();
    for name in displays.names() {
        if name != own && !names.iter().any(|n| n == name) {
            names.push(name.to_string());
        }
    }
    names
}

/// Why a Duplicate was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateRefused {
    /// Half of the Panel's cell would be under the least size of a Panel.
    NoRoom(NoRoom),
    /// The display cannot be copied (nothing can make another of it).
    NoCopy,
}

impl DuplicateRefused {
    /// What the Window tells the colourist.
    pub fn notice(&self) -> String {
        match self {
            DuplicateRefused::NoRoom(no_room) => no_room.notice(),
            DuplicateRefused::NoCopy => "This Panel cannot be duplicated.".to_string(),
        }
    }
}

/// The space the page has: where the Panels are laid out, the gap between them, and one Panel's least size.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub area: [f32; 4],
    pub gap: f32,
    pub min: [f32; 2],
}

/// Duplicate Panel `panel`: a new Panel of the same display, to its right ([`Dir::Row`]) or below ([`Dir::Col`]),
/// taking half of its cell, with a copy of its settings and remembered state. It has the focus. Returns the new
/// Panel's index. With no room, or for a display that cannot be copied, nothing changes and the error says which.
pub fn duplicate(
    displays: &DisplaySet,
    tree: &mut LayoutTree,
    settings: &mut PanelSettings,
    panel: usize,
    dir: Dir,
    room: Room,
) -> Result<usize, DuplicateRefused> {
    let Room { area, gap, min } = room;
    if !tree.has_room_beside(panel, dir, area, gap, min) {
        return Err(DuplicateRefused::NoRoom(NoRoom));
    }
    let new = displays.duplicate(panel).ok_or(DuplicateRefused::NoCopy)?;
    let edge = match dir {
        Dir::Row => Edge::Right,
        Dir::Col => Edge::Bottom,
    };
    tree.add_at_edge(new, panel, edge, area, gap, min);
    settings.copy_panel(&displays.key(panel), &displays.key(new));
    Ok(new)
}

/// Replace Panel `panel` with the display called `name`: the new Panel keeps its place, size and focus and starts
/// from that display's defaults; the old Panel is closed (see [`close`]). `held_elsewhere` says whether another
/// page holds a Panel: its settings are kept while one does, and a first instance another page holds is not taken
/// over. Returns the new Panel's index. `None`, and nothing changes, when there is no such display, it is the
/// Panel's own, or the Panel is not on the page.
pub fn replace(
    displays: &DisplaySet,
    tree: &mut LayoutTree,
    settings: &mut PanelSettings,
    panel: usize,
    name: &str,
    held_elsewhere: impl Fn(usize) -> bool,
) -> Option<usize> {
    if !tree.contains(panel) || displays.name(panel) == name {
        return None;
    }
    // The display's first instance, when it is closed, comes back; else a further instance is made.
    let closed_first = (0..displays.len()).find(|i| {
        displays.name(*i) == name
            && displays.key(*i) == name
            && !tree.contains(*i)
            && !held_elsewhere(*i)
    });
    let new = match closed_first {
        Some(i) => i,
        None => displays.make_named(name)?,
    };
    settings.forget_panel(&displays.key(new));
    tree.replace(panel, new);
    if !held_elsewhere(panel) {
        settings.forget_panel(&displays.key(panel));
    }
    Some(new)
}

/// Close Panel `panel`: its sibling takes the space and everything kept for it is forgotten, unless another page
/// still holds the Panel (`held_elsewhere`): then its settings stay for that page. A further instance ("Name 2")
/// is left off the page; [`sweep`] removes it so its number is free again.
pub fn close(
    displays: &DisplaySet,
    tree: &mut LayoutTree,
    settings: &mut PanelSettings,
    panel: usize,
    held_elsewhere: impl Fn(usize) -> bool,
) {
    tree.close(panel);
    if !held_elsewhere(panel) {
        settings.forget_panel(&displays.key(panel));
    }
}

/// Remove every further instance that is not on the page (closed, replaced, or left behind by an undo or another
/// layout), forgetting its settings. Every Panel after one moves up an index, in the tree too. Returns the indices
/// removed, highest first, each as it was when removed, so whoever keeps trees by index (the undo history) can
/// follow. The first instance of a display is never removed: it stays, closed, for the Add list.
pub fn sweep(
    displays: &DisplaySet,
    tree: &mut LayoutTree,
    settings: &mut PanelSettings,
) -> Vec<usize> {
    let gone: Vec<usize> = (0..displays.len())
        .rev()
        .filter(|i| displays.is_duplicate(*i) && !tree.contains(*i))
        .collect();
    for i in &gone {
        settings.forget_panel(&displays.key(*i));
        displays.remove(*i);
        tree.forget_index(*i);
    }
    gone
}
