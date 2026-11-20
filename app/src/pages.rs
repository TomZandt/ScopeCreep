//! Pages (#212, spec #196, ADR 0041): the Window's layouts shown as tabs in the Page bar. A page is a name and its own
//! layout tree, focus (the tree's) and undo ([`LayoutHistory`]); the pages and the current page are remembered.
//! Only the current page's Panels measure and draw (the Window gives that to the displays, see
//! [`crate::display::DisplaySet::set_shown`]). Pure: no GPU, no window, no fonts.
//!
//! A page with one Panel on another page's tree is the same Panel; a copy of a page asks its caller for a copy of
//! each Panel ([`Pages::copy_current`]), so the copy's settings can be copied with it (ADR 0039).
use crate::layout_history::LayoutHistory;
use crate::layout_tree::LayoutTree;
use crate::layouts::{Layouts, NAME_LENGTH, SLOTS, clean};

/// The most pages there can be: nine, one for each Ctrl+number that goes to a page, so every page has its hotkey and
/// shows its number. The Layout menu lists every page, so the limit is also what the Window's minimum height has room for.
pub const MAX_PAGES: usize = 9;
/// How many pages show their number and have a Ctrl+number to go to them.
pub const NUMBERED: usize = 9;
/// The first line of the saved pages.
const HEADER: &str = "pages 1";
/// What the first page is called when there are no named layouts to carry over.
pub const MAIN: &str = "Main";

#[derive(Clone, Debug)]
struct Page {
    name: String,
    tree: LayoutTree,
    history: LayoutHistory,
}

impl Page {
    fn new(name: String, tree: LayoutTree) -> Page {
        Page {
            name,
            tree,
            history: LayoutHistory::default(),
        }
    }
}

/// The Window's pages, at least one, and which is shown.
#[derive(Clone, Debug)]
pub struct Pages {
    pages: Vec<Page>,
    current: usize,
}

impl Pages {
    /// One page, Main, holding `tree`.
    pub fn new(tree: LayoutTree) -> Pages {
        Pages {
            pages: vec![Page::new(MAIN.to_string(), tree)],
            current: 0,
        }
    }

    /// The first run after named layouts: Main holds `main` (the arrangement the user had), then each saved layout in
    /// slot order becomes a page named as it was. An empty slot adds nothing and a layout that cannot be read is
    /// dropped (ADR 0041).
    pub fn migrate(main: LayoutTree, layouts: &Layouts, names: &[&str]) -> Pages {
        let mut pages = Pages::new(main);
        for slot in 0..SLOTS {
            if let (Some(name), Some(tree)) =
                (layouts.name(slot), layouts.recall_readable(slot, names))
            {
                pages.pages.push(Page::new(name.to_string(), tree));
            }
        }
        pages
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether the Window holds as many pages as it can ([`MAX_PAGES`]): a new page is refused.
    pub fn is_full(&self) -> bool {
        self.pages.len() >= MAX_PAGES
    }

    /// There is always a page.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The shown page's position.
    pub fn current(&self) -> usize {
        self.current
    }

    /// Page `index`'s name.
    pub fn name(&self, index: usize) -> Option<&str> {
        self.pages.get(index).map(|p| p.name.as_str())
    }

    /// Every page's name, in tab order.
    pub fn names(&self) -> Vec<String> {
        self.pages.iter().map(|p| p.name.clone()).collect()
    }

    /// The shown page's layout.
    pub fn tree(&self) -> &LayoutTree {
        &self.pages[self.current].tree
    }

    pub fn tree_mut(&mut self) -> &mut LayoutTree {
        &mut self.pages[self.current].tree
    }

    /// Show page `index`. False (and nothing changes) when there is no such page.
    pub fn go(&mut self, index: usize) -> bool {
        if index >= self.pages.len() {
            return false;
        }
        self.current = index;
        true
    }

    fn push(&mut self, page: Page) -> bool {
        if self.pages.len() >= MAX_PAGES {
            return false;
        }
        self.pages.push(page);
        self.current = self.pages.len() - 1;
        true
    }

    fn next_name(&self) -> String {
        format!("Page {}", self.pages.len() + 1)
    }

    /// A new page with no Panel, for a Window with `total` Panels, at the end and shown. False at the limit.
    pub fn new_empty(&mut self, total: usize) -> bool {
        let name = self.next_name();
        self.push(Page::new(name, LayoutTree::empty(total)))
    }

    /// A new page holding `tree` (see [`default_tree`]), at the end and shown.
    pub fn new_with(&mut self, tree: LayoutTree) -> bool {
        let name = self.next_name();
        self.push(Page::new(name, tree))
    }

    /// [`Self::new_with`] with a name of its own (a page from a template is named after it).
    pub fn new_named(&mut self, name: &str, tree: LayoutTree) -> bool {
        self.push(Page::new(clean(name), tree))
    }

    /// A copy of the shown page, at the end and shown. `copy_panel` is asked once for each Panel on the page and
    /// answers the Panel the copy holds in its place (so the Panel's settings can be copied with it). The copy
    /// starts with nothing to undo. False at the limit (and `copy_panel` is not asked).
    pub fn copy_current(&mut self, mut copy_panel: impl FnMut(usize) -> usize) -> bool {
        if self.pages.len() >= MAX_PAGES {
            return false;
        }
        let from = &self.pages[self.current];
        let mut tree = from.tree.clone();
        let mapping: Vec<(usize, usize)> = tree
            .panels()
            .into_iter()
            .map(|p| (p, copy_panel(p)))
            .collect();
        tree.remap(|p| {
            mapping
                .iter()
                .find(|(old, _)| *old == p)
                .map_or(p, |(_, new)| *new)
        });
        let name: String = format!("Copy of {}", from.name)
            .chars()
            .take(NAME_LENGTH)
            .collect();
        self.push(Page::new(name, tree))
    }

    /// Rename page `index`. An empty name, or a page that is not there, changes nothing. Returns whether it changed.
    pub fn rename(&mut self, index: usize, name: &str) -> bool {
        let name = clean(name);
        match self.pages.get_mut(index) {
            Some(page) if !name.is_empty() && page.name != name => {
                page.name = name;
                true
            }
            _ => false,
        }
    }

    /// Delete page `index`: never the last one. The shown page stays shown, or when it was the one deleted, the page
    /// that took its place is. Returns whether a page went.
    pub fn delete(&mut self, index: usize) -> bool {
        if self.pages.len() <= 1 || index >= self.pages.len() {
            return false;
        }
        self.pages.remove(index);
        if index < self.current {
            self.current -= 1;
        }
        self.current = self.current.min(self.pages.len() - 1);
        true
    }

    /// Run `change` on the shown page's tree; when it changed the arrangement, that is one step of the page's undo.
    pub fn change<R>(&mut self, change: impl FnOnce(&mut LayoutTree) -> R) -> R {
        let page = &mut self.pages[self.current];
        page.history.change(&mut page.tree, change)
    }

    /// Every page's tree and undo, for the Window to keep Panel numbers right across all of them.
    fn trees_mut(&mut self) -> impl Iterator<Item = (&mut LayoutTree, &mut LayoutHistory)> {
        self.pages.iter_mut().map(|p| (&mut p.tree, &mut p.history))
    }

    /// Give every page Panels of its own, in page order (Main first): a Panel that an earlier page already holds is
    /// swapped for the lowest instance of the same display that no earlier page holds, and `make` (the display's
    /// name, returns the new instance's index) makes one when there is none. So the first page ends up on the first
    /// instances, which are the ones the user's saved settings are kept under, and the next pages on the next, whatever
    /// the file said (a migration leaves the named layouts on the same first instances; a page saved on "Clean
    /// picture 3" comes back on the first free one). Within a page, two Panels of one display get two instances.
    /// `names` is each display's name by Panel index.
    pub fn own_instances(&mut self, names: &[&str], mut make: impl FnMut(&str) -> usize) {
        let mut names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let mut claimed: Vec<usize> = Vec::new();
        for page in &mut self.pages {
            let mut map: Vec<(usize, usize)> = Vec::new();
            for panel in page.tree.panels() {
                if map.iter().any(|(from, _)| *from == panel) {
                    continue;
                }
                let Some(name) = names.get(panel).cloned() else {
                    continue;
                };
                let own = (0..names.len())
                    .find(|i| names[*i] == name && !claimed.contains(i))
                    .unwrap_or_else(|| {
                        let made = make(&name);
                        if made >= names.len() {
                            names.resize(made + 1, String::new());
                        }
                        names[made] = name.clone();
                        made
                    });
                claimed.push(own);
                map.push((panel, own));
            }
            page.tree.remap(|p| {
                map.iter()
                    .find(|(from, _)| *from == p)
                    .map_or(p, |(_, to)| *to)
            });
        }
    }

    /// Whether a page other than the shown one holds Panel `panel`.
    pub fn holds_elsewhere(&self, panel: usize) -> bool {
        self.pages
            .iter()
            .enumerate()
            .any(|(i, p)| i != self.current && p.tree.contains(panel))
    }

    /// Whether any page holds Panel `panel`.
    pub fn holds(&self, panel: usize) -> bool {
        self.pages.iter().any(|p| p.tree.contains(panel))
    }

    /// Whether any page's undo could put Panel `panel` back.
    fn undo_names(&self, panel: usize) -> bool {
        self.pages.iter().any(|p| p.history.names(panel))
    }

    /// Remove every further instance ("Name 2") that no page holds (closed, replaced, or left behind by an undo or
    /// another layout), forgetting its settings; a Panel on another page, or one an undo step on any page would
    /// bring back, is kept until nothing names it. Every Panel after a removed one
    /// moves up an index in every page and in every page's undo. Returns the indices removed, highest first. The
    /// first instance of a display is never removed. Afterwards tell the displays which Panels are on the page shown
    /// ([`Pages::show_on`]).
    pub fn sweep(
        &mut self,
        displays: &crate::display::DisplaySet,
        settings: &mut crate::settings::PanelSettings,
    ) -> Vec<usize> {
        let gone: Vec<usize> = (0..displays.len())
            .rev()
            .filter(|i| displays.is_duplicate(*i) && !self.holds(*i) && !self.undo_names(*i))
            .collect();
        for i in &gone {
            settings.forget_panel(&displays.key(*i));
            displays.remove(*i);
            for (tree, history) in self.trees_mut() {
                tree.forget_index(*i);
                history.forget_index(*i);
            }
        }
        gone
    }

    /// Which of `count` Panels are live: the shown page's shown Panels (ADR 0038). Every other Panel, on another
    /// page or on none, is hidden and costs nothing.
    pub fn live(&self, count: usize) -> Vec<bool> {
        let mut live = self.tree().shown_panels();
        live.resize(count, false);
        live
    }

    /// Tell the displays that only the shown page's shown Panels measure and draw.
    pub fn show_on(&self, displays: &crate::display::DisplaySet) {
        displays.set_shown(&self.live(displays.len()));
    }

    /// A divider drag begins on the shown page.
    pub fn begin_drag(&mut self) {
        let page = &mut self.pages[self.current];
        page.history.begin_drag(&page.tree);
    }

    /// The drag is let go: when it moved the divider, the whole drag is one step.
    pub fn end_drag(&mut self) {
        let page = &mut self.pages[self.current];
        page.history.end_drag(&page.tree);
    }

    /// Whether the shown page has a change to undo.
    pub fn can_undo(&self) -> bool {
        self.pages[self.current].history.can_undo()
    }

    /// Undo the shown page's last change. False when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let page = &mut self.pages[self.current];
        page.history.undo(&mut page.tree)
    }

    /// The pages, to remember between sessions (`names` in display order): the shown page's number, then each
    /// page's name and its layout.
    pub fn to_text(&self, names: &[&str]) -> String {
        let mut text = format!("{HEADER}\ncurrent {}\n", self.current + 1);
        for page in &self.pages {
            text += &format!("page {}\n{}", page.name, page.tree.to_text(names));
        }
        text
    }

    /// The pages `text` holds. A page whose layout cannot be read is dropped; `None` when the text is not this
    /// version's or no page is left (the caller then migrates or starts from Main). A shown page that is gone or
    /// out of range is the first.
    pub fn from_text(names: &[&str], text: &str) -> Option<Pages> {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return None;
        }
        let shown: usize = lines
            .next()?
            .strip_prefix("current ")?
            .trim()
            .parse()
            .ok()?;
        let mut raw: Vec<(String, String)> = Vec::new();
        for line in lines {
            if let Some(name) = line.strip_prefix("page ") {
                raw.push((name.to_string(), String::new()));
            } else if let Some((_, tree)) = raw.last_mut() {
                *tree += line;
                *tree += "\n";
            }
        }
        let mut pages = Vec::new();
        let mut current = 0;
        for (i, (name, tree)) in raw.into_iter().enumerate() {
            let name = clean(&name);
            let Some(tree) = LayoutTree::parse_text(names, &tree).filter(|_| !name.is_empty())
            else {
                continue;
            };
            if i + 1 == shown {
                current = pages.len();
            }
            pages.push(Page::new(name, tree));
            if pages.len() == MAX_PAGES {
                break;
            }
        }
        (!pages.is_empty()).then_some(Pages { pages, current })
    }
}

/// A page's name being typed in its tab: the tab shows it, with a caret, until it is accepted or dropped. The text is
/// the name alone: the number in front of a tab is not part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRename {
    pub page: usize,
    pub text: String,
}

impl PageRename {
    /// Start renaming page `page`, from its current name; `None` when there is no such page.
    pub fn start(pages: &Pages, page: usize) -> Option<PageRename> {
        pages.name(page).map(|name| PageRename {
            page,
            text: name.to_string(),
        })
    }

    /// Typed text: control characters are ignored and the name stops at [`NAME_LENGTH`] characters.
    pub fn type_text(&mut self, typed: &str) {
        for c in typed.chars().filter(|c| !c.is_control()) {
            if self.text.chars().count() < NAME_LENGTH {
                self.text.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        self.text.pop();
    }

    /// Accept the name. Returns whether the page's name changed (an empty name keeps the old one).
    pub fn accept(&self, pages: &mut Pages) -> bool {
        pages.rename(self.page, &self.text)
    }
}

/// The default layout for a page, over each display's first instance: a first instance that `claimed` says another
/// page holds is not shared, a new instance of that display is made for this page (with the display's defaults).
pub fn default_tree(
    displays: &crate::display::DisplaySet,
    claimed: impl Fn(usize) -> bool,
) -> LayoutTree {
    let keys = displays.keys();
    let names = displays.names();
    let firsts: Vec<usize> = (0..keys.len()).filter(|i| keys[*i] == names[*i]).collect();
    let first_names: Vec<&str> = firsts.iter().map(|i| names[*i]).collect();
    let mut tree = LayoutTree::default_for(&first_names);
    let own: Vec<usize> = firsts
        .iter()
        .map(|&p| {
            if claimed(p) {
                displays.make_named(names[p]).unwrap_or(p)
            } else {
                p
            }
        })
        .collect();
    tree.remap(|p| own.get(p).copied().unwrap_or(p));
    tree
}

/// Make again the further instances ("Channel Viewer 2") that saved pages or layouts (`texts`) name and the displays do
/// not have yet, so a saved page can be read with its copies. A key that cannot be made is left, and read as a
/// Panel that no longer exists.
pub fn recreate_saved_copies(displays: &crate::display::DisplaySet, texts: &[&str]) {
    for (base, key, n) in wanted_copies(texts) {
        for _ in 0..n {
            if displays.keys().contains(&key) || displays.make_named(&base).is_none() {
                break;
            }
        }
    }
}

/// The further instances the saved `texts` name, as (display, key, number), fewest copies first. A key saved under a
/// display's former name ("Clean picture 2") is the same instance under the new one ("Channel Viewer 2").
pub fn wanted_copies(texts: &[&str]) -> Vec<(String, String, usize)> {
    let mut wanted: Vec<(usize, String, String)> = Vec::new();
    for line in texts.iter().flat_map(|t| t.lines()) {
        let Some((_, list)) = line.strip_prefix("leaf ").and_then(|r| r.split_once(' ')) else {
            continue;
        };
        for key in list.split('|') {
            let key = crate::settings::modern_key(key);
            let Some((base, n)) = key.rsplit_once(' ') else {
                continue;
            };
            let Ok(n) = n.parse::<usize>() else { continue };
            if (2..=MAX_COPIES).contains(&n) && !wanted.iter().any(|(_, _, k)| *k == key) {
                wanted.push((n, base.to_string(), key.clone()));
            }
        }
    }
    wanted.sort();
    wanted.into_iter().map(|(n, b, k)| (b, k, n)).collect()
}

/// The most copies of one display a saved page can ask to be made again.
const MAX_COPIES: usize = 16;

/// What each Panel's title says for the shown page: the display's name, with the instance's number ("Vectorscope 2")
/// only when the page holds two or more Panels of that display. `names` and `keys` are each Panel's display name and
/// instance key by index; `on_page` the Panels the page holds. Keys and ids are untouched: only what is shown changes.
pub fn title_names(names: &[&str], keys: &[String], on_page: &[usize]) -> Vec<String> {
    (0..names.len())
        .map(|p| {
            let twice = on_page
                .iter()
                .filter(|q| names.get(**q) == names.get(p))
                .count()
                >= 2;
            match keys.get(p).and_then(|k| k.strip_prefix(names[p])) {
                Some(number) if twice => format!("{}{}", names[p], number),
                _ => names[p].to_string(),
            }
        })
        .collect()
}

/// What the Add list offers: each display once, by its first instance (the Panel that stands for the display),
/// whatever instances exist, A to Z by name (case does not matter).
pub fn add_offers(names: &[&str]) -> Vec<usize> {
    let mut offers: Vec<usize> = (0..names.len())
        .filter(|i| !names[..*i].contains(&names[*i]))
        .collect();
    offers.sort_by_key(|i| names[*i].to_lowercase());
    offers
}

/// The instance an Add of the display of Panel `offered` puts on the page: the first instance of that display that no
/// page holds, or `None` when every instance is held (the caller makes a new one). `holds` says whether any page holds
/// an instance.
pub fn instance_to_add(
    names: &[&str],
    offered: usize,
    holds: impl Fn(usize) -> bool,
) -> Option<usize> {
    (0..names.len()).find(|i| names[*i] == names[offered] && !holds(*i))
}
