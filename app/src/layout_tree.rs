//! Where the Panels go: a layout tree (ADR 0038). Its leaves are tab groups (one or more Panels, one shown) and its
//! inner nodes are splits, side by side or stacked, each holding the share of its space (net of the gap) that its
//! first side takes. The tree always fills the Panel area, so no cell is ever empty. This is the one place that
//! decides where each display goes. Pure: no GPU, no window, no fonts.
//!
//! A Panel is named here by its index (the display's place in the display order). Rectangles are left, top, width,
//! height in device pixels. What sits inside one Panel (border, title bar, display) is [`crate::panel_grid`].

use crate::auto_layout::Item;
use crate::auto_resize::{self, Shape};

mod arrange;
use crate::panel_grid::{MIN_PANEL, PanelLayout, off_screen};
use crate::style;

/// The window the saved layouts are checked against (logical pixels, the Panel area and the gap between Panels):
/// the default window size. A saved split that would leave a Panel under its minimum here is moved (#179).
const REFERENCE_AREA: [f32; 2] = style::SPACE_REFERENCE_AREA;
const REFERENCE_GAP: f32 = style::SPACE_REFERENCE_GAP;
/// The first line of a saved layout. A grid saved as "layout 1" is converted when it is a sequence of cuts.
const HEADER: &str = "layout 2";
const OLD_HEADER: &str = "layout 1";
/// A saved layout deeper than this, or with more tab groups, is rubbish.
const MAX_DEPTH: usize = 24;
const MAX_LEAVES: usize = 64;
/// A Panel the saved text names that no longer exists.
const UNKNOWN: usize = usize::MAX;
/// Keeps a share away from 0 and 1 so a minimum can always be divided by it.
const MIN_RATIO: f32 = 0.001;

/// Which way a split divides its space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    /// Side by side: the first child on the left.
    Row,
    /// Stacked: the first child on top.
    Col,
}

/// A side of a tab group: where a Panel is docked, or where an edge add puts the new tab group (ADR 0038).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// A divider between the two sides of one split that can be dragged. Names the split by its path from the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Border {
    pub dir: Dir,
    path: Vec<bool>,
}

/// What a press at a point starts: a border drag or a Panel drag (#97). Never both.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Press {
    Border(Border),
    /// Dragging the Panel (display index) by its title bar, to swap it with another.
    Panel(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Leaf {
        tabs: Vec<usize>,
        active: usize,
    },
    Split {
        dir: Dir,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
}

impl Node {
    fn leaf(panel: usize) -> Node {
        Node::Leaf {
            tabs: vec![panel],
            active: 0,
        }
    }

    /// A page with nothing on it.
    const EMPTY: Node = Node::Leaf {
        tabs: Vec::new(),
        active: 0,
    };

    fn at(&self, path: &[bool]) -> Option<&Node> {
        match (path.split_first(), self) {
            (None, _) => Some(self),
            (Some((second, rest)), Node::Split { a, b, .. }) => {
                (if *second { b } else { a }).at(rest)
            }
            _ => None,
        }
    }

    fn at_mut(&mut self, path: &[bool]) -> Option<&mut Node> {
        match (path.split_first(), self) {
            (None, node) => Some(node),
            (Some((second, rest)), Node::Split { a, b, .. }) => {
                (if *second { b } else { a }).at_mut(rest)
            }
            _ => None,
        }
    }

    /// The least size this node needs for every Panel in it to be at least `min`, given the shares it holds.
    fn need(&self, gap: f32, min: [f32; 2]) -> [f32; 2] {
        match self {
            Node::Leaf { .. } => min,
            Node::Split { dir, ratio, a, b } => {
                let (na, nb) = (a.need(gap, min), b.need(gap, min));
                let r = ratio.clamp(MIN_RATIO, 1.0 - MIN_RATIO);
                match dir {
                    Dir::Row => [
                        gap + f32::max(na[0] / r, nb[0] / (1.0 - r)),
                        f32::max(na[1], nb[1]),
                    ],
                    Dir::Col => [
                        f32::max(na[0], nb[0]),
                        gap + f32::max(na[1] / r, nb[1] / (1.0 - r)),
                    ],
                }
            }
        }
    }

    /// Visit every node with its rectangle in `rect` and its path.
    fn walk(
        &self,
        rect: [f32; 4],
        gap: f32,
        path: &mut Vec<bool>,
        f: &mut dyn FnMut(&Node, [f32; 4], &[bool]),
    ) {
        f(self, rect, path);
        if let Node::Split { dir, ratio, a, b } = self {
            let [ra, rb] = halves(*dir, rect, gap, *ratio);
            path.push(false);
            a.walk(ra, gap, path, f);
            path.pop();
            path.push(true);
            b.walk(rb, gap, path, f);
            path.pop();
        }
    }

    /// What this node's cell shows if it is one tab group: the shape of its shown Panel. A split is no shape.
    fn shape(&self, shapes: &[Shape]) -> Shape {
        match self {
            Node::Leaf { tabs, active } => tabs
                .get(*active)
                .and_then(|p| shapes.get(*p))
                .copied()
                .unwrap_or(Shape::Other),
            Node::Split { .. } => Shape::Other,
        }
    }

    /// Auto resize (#213): top down, set the ratio of each split with exactly one picture-shaped side so that
    /// side's cell has the footage's aspect; then go on into both sides with the new rectangles.
    fn fit(&mut self, rect: [f32; 4], gap: f32, fit: &Fit) {
        let Node::Split { dir, ratio, a, b } = self else {
            return;
        };
        let (sa, sb) = (a.shape(fit.shapes), b.shape(fit.shapes));
        if sa.is_picture() != sb.is_picture() {
            let (shape, first) = if sa.is_picture() {
                (sa, true)
            } else {
                (sb, false)
            };
            let (along_width, other, total) = match dir {
                Dir::Row => (true, rect[3], rect[2] - gap),
                Dir::Col => (false, rect[2], rect[3] - gap),
            };
            let needed = auto_resize::cell_extent(shape, fit.aspect, other, along_width, fit.scale);
            let new = auto_resize::share(needed, total, first);
            // A split already there to within a hair is not changed, so a second press is no change at all.
            if (new - *ratio).abs() > 1e-4 {
                *ratio = new;
            }
        }
        // Auto layout holds every share where neither side goes under its minimum.
        if let Some(min) = fit.min {
            let (fa, fb) = (a.floor(gap, min), b.floor(gap, min));
            let (total, lo, hi) = match dir {
                Dir::Row => (rect[2] - gap, fa[0], fb[0]),
                Dir::Col => (rect[3] - gap, fa[1], fb[1]),
            };
            if total > 0.0 && lo + hi <= total {
                *ratio = ratio.min(1.0 - hi / total).max(lo / total);
            }
        }
        let [ra, rb] = halves(*dir, rect, gap, *ratio);
        a.fit(ra, gap, fit);
        b.fit(rb, gap, fit);
    }

    /// The least size this node needs for every Panel in it to be at least `min`, whatever its shares: the Panels'
    /// minimums side by side with the gaps, or the larger when stacked.
    fn floor(&self, gap: f32, min: [f32; 2]) -> [f32; 2] {
        match self {
            Node::Leaf { .. } => min,
            Node::Split { dir, a, b, .. } => {
                let (na, nb) = (a.floor(gap, min), b.floor(gap, min));
                match dir {
                    Dir::Row => [na[0] + gap + nb[0], na[1].max(nb[1])],
                    Dir::Col => [na[0].max(nb[0]), na[1] + gap + nb[1]],
                }
            }
        }
    }

    fn panels(&self, out: &mut Vec<usize>) {
        match self {
            Node::Leaf { tabs, .. } => out.extend(tabs),
            Node::Split { a, b, .. } => {
                a.panels(out);
                b.panels(out);
            }
        }
    }

    /// The Panel that takes over from `panel` when it closes: another tab of its tab group, else the first Panel
    /// of its sibling.
    fn neighbour(&self, panel: usize) -> Option<usize> {
        match self {
            Node::Leaf { tabs, active } => {
                if !tabs.contains(&panel) {
                    return None;
                }
                let shown = tabs.get(*active).copied();
                if shown != Some(panel) {
                    return shown;
                }
                // The tab that takes the closed one's place: the next, else the one before.
                let at = tabs.iter().position(|t| *t == panel)?;
                tabs.get(at + 1)
                    .or_else(|| at.checked_sub(1).and_then(|b| tabs.get(b)))
                    .copied()
            }
            Node::Split { a, b, .. } => {
                let (inside, other) = if a.holds(panel) { (a, b) } else { (b, a) };
                let alone = matches!(&**inside, Node::Leaf { tabs, .. } if tabs.len() == 1);
                if alone {
                    let mut list = Vec::new();
                    other.panels(&mut list);
                    list.first().copied()
                } else {
                    inside.neighbour(panel)
                }
            }
        }
    }

    /// The tab group holding `panel`: its tabs and which one is shown.
    fn group_mut(&mut self, panel: usize) -> Option<(&mut Vec<usize>, &mut usize)> {
        match self {
            Node::Leaf { tabs, active } => {
                if tabs.contains(&panel) {
                    Some((tabs, active))
                } else {
                    None
                }
            }
            Node::Split { a, b, .. } => {
                if a.holds(panel) {
                    a.group_mut(panel)
                } else {
                    b.group_mut(panel)
                }
            }
        }
    }

    fn holds(&self, panel: usize) -> bool {
        let mut list = Vec::new();
        self.panels(&mut list);
        list.contains(&panel)
    }

    fn map_panels(&mut self, f: &impl Fn(usize) -> usize) {
        match self {
            Node::Leaf { tabs, .. } => tabs.iter_mut().for_each(|t| *t = f(*t)),
            Node::Split { a, b, .. } => {
                a.map_panels(f);
                b.map_panels(f);
            }
        }
    }

    /// This node without the Panels `gone` says are gone: an emptied tab group is removed and its sibling takes
    /// the space. None when nothing is left.
    fn pruned(self, gone: &impl Fn(usize) -> bool) -> Option<Node> {
        match self {
            Node::Leaf { tabs, active } => {
                let shown = tabs.get(active).copied();
                let tabs: Vec<usize> = tabs.into_iter().filter(|t| !gone(*t)).collect();
                // The shown tab stays shown; when it is the one that goes, the tab that takes its place is.
                let active = shown
                    .and_then(|s| tabs.iter().position(|t| *t == s))
                    .unwrap_or_else(|| active.min(tabs.len().saturating_sub(1)));
                (!tabs.is_empty()).then_some(Node::Leaf { tabs, active })
            }
            Node::Split { dir, ratio, a, b } => match (a.pruned(gone), b.pruned(gone)) {
                (Some(a), Some(b)) => Some(Node::Split {
                    dir,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (one, other) => one.or(other),
            },
        }
    }

    fn write(&self, names: &[&str], text: &mut String) {
        match self {
            Node::Leaf { tabs, .. } if tabs.is_empty() => *text += "empty\n",
            Node::Leaf { tabs, active } => {
                let list: Vec<&str> = tabs
                    .iter()
                    .map(|t| names.get(*t).copied().unwrap_or(""))
                    .collect();
                *text += &format!("leaf {active} {}\n", list.join("|"));
            }
            Node::Split { dir, ratio, a, b } => {
                let dir = if *dir == Dir::Row { "row" } else { "col" };
                *text += &format!("split {dir} {ratio}\n");
                a.write(names, text);
                b.write(names, text);
            }
        }
    }
}

/// What Auto resize fits to.
struct Fit<'a> {
    shapes: &'a [Shape],
    aspect: f32,
    scale: f32,
    /// Auto layout: the least size of a Panel, which no share may break.
    min: Option<[f32; 2]>,
}

/// The two sides of a split of `rect`: the first takes `ratio` of the space net of `gap`.
fn halves(dir: Dir, rect: [f32; 4], gap: f32, ratio: f32) -> [[f32; 4]; 2] {
    let [x, y, w, h] = rect;
    match dir {
        Dir::Row => {
            let first = ratio * (w - gap).max(0.0);
            [
                [x, y, first, h],
                [x + first + gap, y, (w - first - gap).max(0.0), h],
            ]
        }
        Dir::Col => {
            let first = ratio * (h - gap).max(0.0);
            [
                [x, y, w, first],
                [x, y + first + gap, w, (h - first - gap).max(0.0)],
            ]
        }
    }
}

/// The default layout, in whole numbers: columns and rows (as weights), then each Panel by display name at
/// `(column, row, columns, rows)`. 5 x 4: the clean picture 3 x 2 at the top left, the luma waveform and the
/// Vectorscope (two cells wide) to its right, then two rows of Picture Views below (ARRI False Colour three wide
/// with Stops False Colour, Skin tone with the Saturation heatmap three wide), every cell used. It is turned into a
/// tree by the same conversion an old saved grid goes through.
const DEFAULT_COLUMNS: [f32; 5] = [1.0, 1.0, 1.0, 1.0, 1.0];
const DEFAULT_ROWS: [f32; 4] = [14.0, 14.0, 11.0, 11.0];
const DEFAULT_PLACES: [(&str, [usize; 4]); 7] = [
    ("Clean picture", [0, 0, 3, 2]),
    ("Luma waveform", [3, 0, 2, 1]),
    ("Vectorscope", [3, 1, 2, 1]),
    ("ARRI False Colour", [0, 2, 3, 1]),
    ("Stops False Colour", [3, 2, 2, 1]),
    ("Saturation heatmap", [0, 3, 3, 1]),
    ("Skin tone", [3, 3, 2, 1]),
];

/// The names of the displays the default layout is made for, in the order the default layout lists them.
pub fn default_names() -> Vec<&'static str> {
    DEFAULT_PLACES.iter().map(|(name, _)| *name).collect()
}

/// A Panel's cell of a grid: its first column and row, and how many columns and rows it spans.
#[derive(Clone, Copy)]
struct Cell {
    panel: usize,
    column: usize,
    row: usize,
    columns: usize,
    rows: usize,
}

/// The grid being turned into a tree: weights of its columns and rows, and the size of one weight in the
/// reference window.
struct Grid<'a> {
    columns: &'a [f32],
    rows: &'a [f32],
    column_unit: f32,
    row_unit: f32,
}

impl Grid<'_> {
    /// The size of columns `a..b` (`across`) or rows with the gaps between them, in the reference window.
    fn extent(&self, across: bool, a: usize, b: usize) -> f32 {
        let (weights, unit) = if across {
            (self.columns, self.column_unit)
        } else {
            (self.rows, self.row_unit)
        };
        weights[a..b].iter().sum::<f32>() * unit + REFERENCE_GAP * (b - a - 1) as f32
    }

    /// `items` (which fill columns `c` and rows `r` exactly) as a tree, by cutting edge to edge. The first cut
    /// found is taken, side by side before stacked, so the tree is the same every time.
    fn build(&self, items: &[Cell], c: (usize, usize), r: (usize, usize)) -> Option<Node> {
        if let [one] = items {
            return Some(Node::leaf(one.panel));
        }
        let boxed = |n: Option<Node>| n.map(Box::new);
        for k in c.0 + 1..c.1 {
            if items
                .iter()
                .all(|i| i.column + i.columns <= k || i.column >= k)
            {
                let (left, right): (Vec<Cell>, Vec<Cell>) =
                    items.iter().partition(|i| i.column < k);
                return Some(Node::Split {
                    dir: Dir::Row,
                    ratio: self.extent(true, c.0, k)
                        / (self.extent(true, c.0, c.1) - REFERENCE_GAP),
                    a: boxed(self.build(&left, (c.0, k), r))?,
                    b: boxed(self.build(&right, (k, c.1), r))?,
                });
            }
        }
        for k in r.0 + 1..r.1 {
            if items.iter().all(|i| i.row + i.rows <= k || i.row >= k) {
                let (top, bottom): (Vec<Cell>, Vec<Cell>) = items.iter().partition(|i| i.row < k);
                return Some(Node::Split {
                    dir: Dir::Col,
                    ratio: self.extent(false, r.0, k)
                        / (self.extent(false, r.0, r.1) - REFERENCE_GAP),
                    a: boxed(self.build(&top, c, (r.0, k)))?,
                    b: boxed(self.build(&bottom, c, (k, r.1)))?,
                });
            }
        }
        None
    }
}

/// A grid as a tree, when its Panels tile it exactly and are a sequence of cuts (edge to edge, each splitting
/// what is left in two). The shares are those the grid has in the reference window, so the arrangement is the
/// same there.
fn tree_from_grid(columns: &[f32], rows: &[f32], cells: &[Cell]) -> Option<Node> {
    if cells.is_empty() {
        return None;
    }
    let unit = |w: &[f32], total: f32| {
        (total - REFERENCE_GAP * w.len().saturating_sub(1) as f32) / w.iter().sum::<f32>()
    };
    // Every grid cell is used exactly once.
    let mut used = vec![vec![0u8; columns.len()]; rows.len()];
    for c in cells {
        if c.columns == 0
            || c.rows == 0
            || c.column + c.columns > columns.len()
            || c.row + c.rows > rows.len()
        {
            return None;
        }
        for row in &mut used[c.row..c.row + c.rows] {
            for n in &mut row[c.column..c.column + c.columns] {
                *n += 1;
            }
        }
    }
    if used.iter().flatten().any(|n| *n != 1) {
        return None;
    }
    let grid = Grid {
        columns,
        rows,
        column_unit: unit(columns, REFERENCE_AREA[0]),
        row_unit: unit(rows, REFERENCE_AREA[1]),
    };
    grid.build(cells, (0, columns.len()), (0, rows.len()))
}

/// The layout: a tree of tab groups filling the Panel area. A Panel the Window has but the tree does not hold is
/// closed (not on the page); a page with no Panel at all is empty.
#[derive(Clone, Debug)]
pub struct LayoutTree {
    root: Node,
    /// The Panel that has the focus (Add panel splits its tab group). Session only: never saved, and not part of
    /// which layout this is.
    focus: Option<usize>,
    /// How many Panels the Window has, closed ones included: [`LayoutTree::cells`] has a cell for each.
    total: usize,
}

impl PartialEq for LayoutTree {
    /// The same arrangement: the focus does not make a layout another layout.
    fn eq(&self, other: &LayoutTree) -> bool {
        self.root == other.root
    }
}

/// Where an add or a dock put the Panel. With no room for a split (it would leave a Panel under its minimum size)
/// the Panel joins the target tab group as a new tab, so no Panel is refused or lost (ADR 0038).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    /// A new tab group beside the target.
    Split,
    /// A new tab of the target tab group, for want of room.
    Tabbed,
}

impl Placed {
    /// What the Window tells the colourist after an add: names the least size a Panel can be (logical pixels).
    /// None when the Panel got a place of its own.
    pub fn notice(self) -> Option<String> {
        (self == Placed::Tabbed).then(|| {
            format!(
                "No room for another Panel here, so it opened as a tab: a Panel needs at least {} x {}.",
                MIN_PANEL[0], MIN_PANEL[1]
            )
        })
    }

    /// The same, for a dock.
    pub fn dock_notice(self) -> Option<String> {
        (self == Placed::Tabbed).then(|| {
            format!(
                "No room to dock here, so it joined as a tab: a Panel needs at least {} x {}.",
                MIN_PANEL[0], MIN_PANEL[1]
            )
        })
    }
}

/// A Duplicate that does not fit: it is refused, since a copy as a tab beside its own original helps no one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoRoom;

impl NoRoom {
    /// What the Window tells the colourist: names the least size a Panel can be (logical pixels).
    pub fn notice(&self) -> String {
        format!(
            "No room for another Panel here: a Panel needs at least {} x {}.",
            MIN_PANEL[0], MIN_PANEL[1]
        )
    }
}

/// Auto layout with no arrangement that keeps every Panel at its least size: the page is left as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoFit;

impl NoFit {
    /// What the Window tells the colourist: names the least size a Panel can be (logical pixels).
    pub fn notice(&self) -> String {
        format!(
            "Auto layout left the page as it was, nothing fits: a Panel needs at least {} x {}.",
            MIN_PANEL[0], MIN_PANEL[1]
        )
    }
}

/// A tab group as the title bar shows it: its Panels in tab order and the index of the shown one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabGroup {
    pub tabs: Vec<usize>,
    pub active: usize,
}

impl LayoutTree {
    /// A plain layout for `panels` displays, split in halves along the longer side so every Panel is about the
    /// same size. Used when the displays are not the ones the default layout is made for.
    pub fn automatic(panels: usize) -> LayoutTree {
        fn go(lo: usize, hi: usize, w: f32, h: f32) -> Node {
            let n = hi - lo;
            if n == 1 {
                return Node::leaf(lo);
            }
            let first = n.div_ceil(2);
            let ratio = first as f32 / n as f32;
            let dir = if w >= h { Dir::Row } else { Dir::Col };
            let (wa, ha, wb, hb) = match dir {
                Dir::Row => (w * ratio, h, w * (1.0 - ratio), h),
                Dir::Col => (w, h * ratio, w, h * (1.0 - ratio)),
            };
            Node::Split {
                dir,
                ratio,
                a: Box::new(go(lo, lo + first, wa, ha)),
                b: Box::new(go(lo + first, hi, wb, hb)),
            }
        }
        let root = if panels == 0 {
            Node::EMPTY
        } else {
            go(0, panels, REFERENCE_AREA[0], REFERENCE_AREA[1])
        };
        LayoutTree::new(root, panels)
    }

    fn new(root: Node, total: usize) -> LayoutTree {
        LayoutTree {
            root,
            focus: None,
            total,
        }
    }

    /// The default layout for the displays called `names` (in display order). Falls back to [`Self::automatic`]
    /// unless these are exactly the displays the default layout is made for, so the tree fills either way.
    pub fn default_for(names: &[&str]) -> LayoutTree {
        let cells: Option<Vec<Cell>> = names
            .iter()
            .enumerate()
            .map(|(panel, name)| {
                DEFAULT_PLACES.iter().find(|(n, _)| n == name).map(
                    |(_, [column, row, columns, rows])| Cell {
                        panel,
                        column: *column,
                        row: *row,
                        columns: *columns,
                        rows: *rows,
                    },
                )
            })
            .collect();
        cells
            .filter(|c| c.len() == DEFAULT_PLACES.len())
            .and_then(|c| tree_from_grid(&DEFAULT_COLUMNS, &DEFAULT_ROWS, &c))
            .map_or_else(
                || LayoutTree::automatic(names.len()),
                |root| LayoutTree::new(root, names.len()),
            )
    }

    /// Auto resize (#213): change ratios only, top down. Where a split has exactly one picture-shaped side (a
    /// Picture View or the luma waveform, by `shapes` in Panel order) its ratio is set so that side's cell leaves
    /// room for a picture of `aspect` (title bar, legend strip and, for the waveform, the scale gutter taken off),
    /// clamped to 15 to 85 %. `area`, `gap` and `scale` are the Window's. Places and nesting never change.
    pub fn auto_resize(
        &mut self,
        shapes: &[Shape],
        aspect: f32,
        area: [f32; 4],
        gap: f32,
        scale: f32,
    ) {
        let fit = Fit {
            shapes,
            aspect,
            scale,
            min: Some(crate::panel_grid::min_panel_size(scale)),
        };
        self.root.fit(area, gap, &fit);
    }

    /// Auto layout (#214): re-arrange the page once. The tab groups keep their reading order and stay whole; the
    /// search cuts that list into splits, fits picture cells as Auto resize does, drops any cut that breaks the
    /// least size of a Panel and keeps the best score ([`crate::auto_layout`]). `items` are by Panel, in Panel
    /// order. With no cut that fits the tree is left as it was and the error names the minimum. Focus and tabs
    /// are kept.
    pub fn auto_layout(
        &mut self,
        items: &[Item],
        aspect: f32,
        area: [f32; 4],
        gap: f32,
        scale: f32,
    ) -> Result<(), NoFit> {
        if let Some(root) = arrange::arrange(&self.root, items, aspect, area, gap, scale)? {
            self.root = root;
        }
        Ok(())
    }

    /// A page with no Panel, for a Window with `total` Panels.
    pub fn empty(total: usize) -> LayoutTree {
        LayoutTree::new(Node::EMPTY, total)
    }

    /// The Panels the page holds, in the order of the tree.
    pub fn panels(&self) -> Vec<usize> {
        self.panel_list()
    }

    /// Give every Panel on the page the number `f` says (a copy of a page holds copies of its Panels); the focus
    /// follows its Panel.
    pub fn remap(&mut self, f: impl Fn(usize) -> usize) {
        self.root.map_panels(&f);
        self.focus = self.focus.map(&f);
        let most = self.panel_list().iter().map(|p| p + 1).max().unwrap_or(0);
        self.total = self.total.max(most);
    }

    fn panel_list(&self) -> Vec<usize> {
        let mut all = Vec::new();
        self.root.panels(&mut all);
        all
    }

    /// How many Panels the tree holds.
    pub fn len(&self) -> usize {
        self.panel_list().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The least area (width, height, with `gap` between Panels) in which every Panel is at least `min`: the
    /// shares the tree holds stay as they are, so each side of a split needs its own minimum divided by its share.
    pub fn min_area(&self, gap: f32, min: [f32; 2]) -> [f32; 2] {
        self.root.need(gap, min)
    }

    /// Each Panel's rectangle, in display order, filling `area` with `gap` between Panels. A Panel in a tab group
    /// that is not the shown one has an empty cell far off screen.
    pub fn cells(&self, area: [f32; 4], gap: f32) -> Vec<[f32; 4]> {
        let count = self.cells_len();
        let mut out = vec![off_screen(); count];
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |node, rect, _| {
                if let Node::Leaf { tabs, active } = node
                    && let Some(p) = tabs.get(*active)
                {
                    out[*p] = rect;
                }
            });
        out
    }

    /// The divider under `point`, if any: within `slop` of the gap between the two sides of a split.
    pub fn border_at(
        &self,
        area: [f32; 4],
        gap: f32,
        point: [f32; 2],
        slop: f32,
    ) -> Option<Border> {
        let mut best: Option<(f32, Border)> = None;
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |node, rect, path| {
                let Node::Split { dir, ratio, .. } = node else {
                    return;
                };
                let [first, _] = halves(*dir, rect, gap, *ratio);
                let (across, along, middle, lo, len) = match dir {
                    Dir::Row => (
                        point[0],
                        point[1],
                        first[0] + first[2] + gap / 2.0,
                        rect[1],
                        rect[3],
                    ),
                    Dir::Col => (
                        point[1],
                        point[0],
                        first[1] + first[3] + gap / 2.0,
                        rect[0],
                        rect[2],
                    ),
                };
                let distance = (across - middle).abs();
                if distance <= gap / 2.0 + slop
                    && along >= lo
                    && along <= lo + len
                    && best.as_ref().is_none_or(|(d, _)| distance < *d)
                {
                    let border = Border {
                        dir: *dir,
                        path: path.to_vec(),
                    };
                    best = Some((distance, border));
                }
            });
        best.map(|(_, b)| b)
    }

    /// What a press at `point` starts: a border drag when it is on a border (that wins), a Panel drag when it is on
    /// a Panel's title bar (at `scale` device pixels per logical pixel), else nothing.
    pub fn press_at(
        &self,
        area: [f32; 4],
        gap: f32,
        point: [f32; 2],
        slop: f32,
        scale: f32,
    ) -> Option<Press> {
        if let Some(border) = self.border_at(area, gap, point, slop) {
            return Some(Press::Border(border));
        }
        let panel = self.panel_at(area, gap, point)?;
        let t = PanelLayout::new(self.cells(area, gap)[panel], scale).title;
        let in_bar = point[0] >= t[0]
            && point[0] < t[0] + t[2]
            && point[1] >= t[1]
            && point[1] < t[1] + t[3];
        in_bar.then_some(Press::Panel(panel))
    }

    /// The Panel whose cell holds `point`, if any (a gap holds none).
    pub fn panel_at(&self, area: [f32; 4], gap: f32, point: [f32; 2]) -> Option<usize> {
        self.cells(area, gap).iter().position(|c| {
            point[0] >= c[0] && point[0] < c[0] + c[2] && point[1] >= c[1] && point[1] < c[1] + c[3]
        })
    }

    /// Swap two Panels: each takes the other's place in the tree.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.root.map_panels(&|p| match p {
            p if p == a => b,
            p if p == b => a,
            p => p,
        });
    }

    /// Dock Panel `dragged` on `edge` of the tab group that shows Panel `target` (#210): that edge is split 50/50
    /// with the dragged Panel taking the half on `edge`, and the dragged Panel's old tab group gives up its space to
    /// its neighbour (or just loses the tab; a tab dragged out leaves the others as they were). A drop on the
    /// Panel's own single-Panel tab group does nothing. When half of the target would be under `min` the dragged
    /// Panel joins the target tab group as a tab instead ([`Placed::Tabbed`]). The dragged Panel has the focus
    /// after.
    pub fn dock(
        &mut self,
        dragged: usize,
        target: usize,
        edge: Edge,
        area: [f32; 4],
        gap: f32,
        min: [f32; 2],
    ) -> Placed {
        if !self.contains(dragged) || !self.contains(target) {
            return Placed::Split;
        }
        // A Panel of the target tab group that stays put, to find it again once the dragged Panel has left.
        let mut group = Vec::new();
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |node, _, _| {
                if let Node::Leaf { tabs, .. } = node
                    && tabs.contains(&target)
                {
                    group = tabs.clone();
                }
            });
        let Some(anchor) = group.iter().copied().find(|p| *p != dragged) else {
            return Placed::Split;
        };
        let active_target = group.contains(&target) && target != dragged;
        let mut next = self.clone();
        let root = std::mem::replace(&mut next.root, Node::EMPTY);
        next.root = root.pruned(&|p| p == dragged).unwrap_or(Node::EMPTY);
        let anchor = if active_target { target } else { anchor };
        let mut found = None;
        next.root
            .walk(area, gap, &mut Vec::new(), &mut |node, rect, path| {
                if let Node::Leaf { tabs, .. } = node
                    && tabs.contains(&anchor)
                {
                    found = Some((path.to_vec(), rect));
                }
            });
        let Some((path, rect)) = found else {
            return Placed::Split;
        };
        let (dir, along, least) = match edge {
            Edge::Left | Edge::Right => (Dir::Row, rect[2], min[0]),
            Edge::Top | Edge::Bottom => (Dir::Col, rect[3], min[1]),
        };
        if (along - gap) / 2.0 < least - 0.001 {
            // No room to split: the dragged Panel becomes a tab of the target group.
            let mut tabbed = self.clone();
            tabbed.join_tab(dragged, target);
            *self = tabbed;
            return Placed::Tabbed;
        }
        let Some(node) = next.root.at_mut(&path) else {
            return Placed::Split;
        };
        let old = std::mem::replace(node, Node::EMPTY);
        let new = Box::new(Node::leaf(dragged));
        let (a, b) = match edge {
            Edge::Left | Edge::Top => (new, Box::new(old)),
            Edge::Right | Edge::Bottom => (Box::new(old), new),
        };
        *node = Node::Split {
            dir,
            ratio: 0.5,
            a,
            b,
        };
        next.focus = Some(dragged);
        *self = next;
        Placed::Split
    }

    /// Dock Panel `dragged` in the middle of the tab group that shows Panel `target`: it becomes a tab there and is
    /// the one shown (#211). Its old tab group gives up its space to its neighbour, or just loses the tab. Dropped
    /// on its own tab group it is only shown.
    pub fn join_tab(&mut self, dragged: usize, target: usize) {
        if !self.contains(dragged) || !self.contains(target) {
            return;
        }
        let same = self
            .root
            .group_mut(dragged)
            .is_some_and(|(tabs, _)| tabs.contains(&target));
        if same {
            self.show(dragged);
            return;
        }
        let root = std::mem::replace(&mut self.root, Node::EMPTY);
        self.root = root.pruned(&|p| p == dragged).unwrap_or(Node::EMPTY);
        self.push_tab(dragged, target);
    }

    /// Put `panel` last among the tabs of the group holding `target`, shown and focused.
    fn push_tab(&mut self, panel: usize, target: usize) {
        if let Some((tabs, active)) = self.root.group_mut(target) {
            tabs.push(panel);
            *active = tabs.len() - 1;
            self.focus = Some(panel);
            self.total = self.total.max(panel + 1);
        }
    }

    /// Show Panel `panel` in its tab group and give it the focus (a click on its tab). Nothing happens when it is
    /// not on the page.
    pub fn show(&mut self, panel: usize) {
        if let Some((tabs, active)) = self.root.group_mut(panel)
            && let Some(at) = tabs.iter().position(|t| *t == panel)
        {
            *active = at;
            self.focus = Some(panel);
        }
    }

    /// The tab group holding Panel `panel`: its tabs in order and the index of the shown one.
    pub fn tab_group(&self, panel: usize) -> Option<TabGroup> {
        let mut found = None;
        self.root
            .walk([0.0; 4], 0.0, &mut Vec::new(), &mut |node, _, _| {
                if let Node::Leaf { tabs, active } = node
                    && tabs.contains(&panel)
                {
                    found = Some(TabGroup {
                        tabs: tabs.clone(),
                        active: *active,
                    });
                }
            });
        found
    }

    /// Which Panels are shown (one per tab group; none closed, none in a hidden tab), in display order. Only
    /// these measure and draw (ADR 0038).
    pub fn shown_panels(&self) -> Vec<bool> {
        let mut out = vec![false; self.cells_len()];
        self.root
            .walk([0.0; 4], 0.0, &mut Vec::new(), &mut |node, _, _| {
                if let Node::Leaf { tabs, active } = node
                    && let Some(p) = tabs.get(*active)
                {
                    out[*p] = true;
                }
            });
        out
    }

    fn cells_len(&self) -> usize {
        self.panel_list()
            .into_iter()
            .max()
            .map_or(0, |m| m + 1)
            .max(self.total)
    }

    /// Whether the page holds Panel `panel` (a Panel that is not on it is closed).
    pub fn contains(&self, panel: usize) -> bool {
        self.panel_list().contains(&panel)
    }

    /// The Panel with the focus: the one last clicked or added, else the first Panel of the page. None on an empty
    /// page.
    pub fn focused(&self) -> Option<usize> {
        let all = self.panel_list();
        self.focus
            .filter(|f| all.contains(f))
            .or_else(|| all.first().copied())
    }

    /// Give Panel `panel` the focus (nothing happens when it is not on the page).
    pub fn set_focus(&mut self, panel: usize) {
        if self.contains(panel) {
            self.focus = Some(panel);
        }
    }

    /// Add Panel `panel` to the page: the focused tab group is split along its longer side (wider than tall: the new
    /// Panel on the right; otherwise below) at 50/50, and the new Panel has the focus. On an empty page it fills
    /// `area`. A Panel already on the page is only focused. When half of the focused cell would be under `min`
    /// (one Panel's least width and height) the new Panel joins the focused tab group as a tab ([`Placed::Tabbed`]).
    pub fn add(&mut self, panel: usize, area: [f32; 4], gap: f32, min: [f32; 2]) -> Placed {
        if self.contains(panel) {
            self.show(panel);
            return Placed::Split;
        }
        let Some(target) = self.focused() else {
            self.root = Node::leaf(panel);
            self.focus = Some(panel);
            self.total = self.total.max(panel + 1);
            return Placed::Split;
        };
        let Some(dir) = self.long_side(target, area, gap) else {
            self.push_tab(panel, target);
            return Placed::Tabbed;
        };
        self.split_beside(panel, target, dir, false, area, gap, min)
    }

    /// Add Panel `panel` on the `edge` side of the tab group holding Panel `target` (the **+** on a Panel's edge):
    /// the new tab group takes half of it on that side (net of the gap) and has the focus. A Panel already on the
    /// page is only shown. When half of the cell would be under `min` the new Panel joins that tab group as a tab
    /// ([`Placed::Tabbed`]).
    pub fn add_at_edge(
        &mut self,
        panel: usize,
        target: usize,
        edge: Edge,
        area: [f32; 4],
        gap: f32,
        min: [f32; 2],
    ) -> Placed {
        if self.contains(panel) {
            self.show(panel);
            return Placed::Split;
        }
        if !self.contains(target) {
            return self.add(panel, area, gap, min);
        }
        let (dir, first) = match edge {
            Edge::Left => (Dir::Row, true),
            Edge::Right => (Dir::Row, false),
            Edge::Top => (Dir::Col, true),
            Edge::Bottom => (Dir::Col, false),
        };
        self.split_beside(panel, target, dir, first, area, gap, min)
    }

    /// Whether Panel `target` has room for a second Panel beside it in `dir` (right for [`Dir::Row`], below for
    /// [`Dir::Col`]): half of its cell, net of the gap, holds one Panel's least size `min`.
    pub fn has_room_beside(
        &self,
        target: usize,
        dir: Dir,
        area: [f32; 4],
        gap: f32,
        min: [f32; 2],
    ) -> bool {
        let edge = match dir {
            Dir::Row => Edge::Right,
            Dir::Col => Edge::Bottom,
        };
        self.clone()
            .add_at_edge(usize::MAX - 1, target, edge, area, gap, min)
            == Placed::Split
    }

    /// Replace (ADR 0039): Panel `new` takes Panel `old`'s place and size in the tree (and its focus). Nothing
    /// happens when `old` is not on the page or `new` already is.
    pub fn replace(&mut self, old: usize, new: usize) {
        if !self.contains(old) || self.contains(new) {
            return;
        }
        self.root.map_panels(&|p| if p == old { new } else { p });
        if self.focus == Some(old) {
            self.focus = Some(new);
        }
        self.total = self.total.max(new + 1);
    }

    /// The display of index `removed` is gone for good (a closed duplicate): every Panel after it moves up one
    /// index, here and in the focus. `removed` is not on the page.
    pub fn forget_index(&mut self, removed: usize) {
        self.root
            .map_panels(&|p| if p > removed { p - 1 } else { p });
        self.focus = self
            .focus
            .filter(|f| *f != removed)
            .map(|f| if f > removed { f - 1 } else { f });
        self.total = self.total.saturating_sub(1);
    }

    /// The longer side of the tab group holding `panel`.
    fn long_side(&self, panel: usize, area: [f32; 4], gap: f32) -> Option<Dir> {
        let mut found = None;
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |node, rect, _| {
                if let Node::Leaf { tabs, .. } = node
                    && tabs.contains(&panel)
                {
                    let dir = if rect[2] >= rect[3] {
                        Dir::Row
                    } else {
                        Dir::Col
                    };
                    found = Some(dir);
                }
            });
        found
    }

    /// Split the tab group holding `target` in `dir` at 50/50, the new Panel first (left or above) or second, and
    /// focus it; when half of the cell would be under `min` the new Panel is a tab of that group instead.
    #[allow(clippy::too_many_arguments)]
    fn split_beside(
        &mut self,
        panel: usize,
        target: usize,
        dir: Dir,
        new_first: bool,
        area: [f32; 4],
        gap: f32,
        min: [f32; 2],
    ) -> Placed {
        let mut found = None;
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |node, rect, path| {
                if let Node::Leaf { tabs, .. } = node
                    && tabs.contains(&target)
                {
                    found = Some((path.to_vec(), rect));
                }
            });
        let Some((path, rect)) = found else {
            self.push_tab(panel, target);
            return Placed::Tabbed;
        };
        let (along, least) = match dir {
            Dir::Row => (rect[2], min[0]),
            Dir::Col => (rect[3], min[1]),
        };
        let node = self.root.at_mut(&path);
        let Some(node) = node.filter(|_| (along - gap) / 2.0 >= least - 0.001) else {
            self.push_tab(panel, target);
            return Placed::Tabbed;
        };
        let old = std::mem::replace(node, Node::EMPTY);
        let new = Node::leaf(panel);
        let (a, b) = if new_first { (new, old) } else { (old, new) };
        *node = Node::Split {
            dir,
            ratio: 0.5,
            a: Box::new(a),
            b: Box::new(b),
        };
        self.focus = Some(panel);
        self.total = self.total.max(panel + 1);
        Placed::Split
    }

    /// Close Panel `panel`: its sibling takes its space. When it had the focus, its neighbour has it now. The last
    /// Panel closed leaves an empty page. Nothing happens when the Panel is not on the page.
    pub fn close(&mut self, panel: usize) {
        if !self.contains(panel) {
            return;
        }
        let had_focus = self.focused() == Some(panel);
        let neighbour = self.root.neighbour(panel);
        let root = std::mem::replace(&mut self.root, Node::EMPTY);
        self.root = root.pruned(&|p| p == panel).unwrap_or(Node::EMPTY);
        if had_focus {
            self.focus = neighbour;
        }
    }

    /// Move `border` to `to`: only the split it belongs to changes, so who is next to whom stays as it was, and
    /// neither side goes under what its Panels need (`min` is one Panel's least width and height).
    pub fn drag(&mut self, border: &Border, to: [f32; 2], area: [f32; 4], gap: f32, min: [f32; 2]) {
        let mut found = None;
        self.root
            .walk(area, gap, &mut Vec::new(), &mut |_, rect, path| {
                if path == border.path.as_slice() {
                    found = Some(rect);
                }
            });
        let Some(rect) = found else { return };
        let Some(Node::Split { dir, ratio, a, b }) = self.root.at(&border.path) else {
            return;
        };
        let (need_a, need_b) = (a.need(gap, min), b.need(gap, min));
        let (start, total, position, na, nb) = match dir {
            Dir::Row => (rect[0], rect[2], to[0], need_a[0], need_b[0]),
            Dir::Col => (rect[1], rect[3], to[1], need_a[1], need_b[1]),
        };
        let avail = total - gap;
        if avail <= 0.0 {
            return;
        }
        let wanted = position - gap / 2.0 - start;
        let first = if na + nb > avail {
            *ratio * avail
        } else {
            wanted.clamp(na, avail - nb)
        };
        let new_ratio = (first / avail).clamp(MIN_RATIO, 1.0 - MIN_RATIO);
        if let Some(Node::Split { ratio, .. }) = self.root.at_mut(&border.path) {
            *ratio = new_ratio;
        }
    }

    /// The layout, to remember between sessions (`names` in display order).
    pub fn to_text(&self, names: &[&str]) -> String {
        let mut text = format!("{HEADER}\n");
        self.root.write(names, &mut text);
        text
    }

    /// The layout `text` holds for the displays called `names`. A grid saved before the tree ("layout 1") is
    /// converted when it is a sequence of cuts. Anything else that is not this version's, does not make sense, or
    /// does not hold exactly these displays once the ones that no longer exist are dropped, gives the default
    /// layout. A split saved so that a Panel is under its minimum in the reference window is moved off it; one that
    /// cannot be is rubbish.
    pub fn from_text(names: &[&str], text: &str) -> LayoutTree {
        Self::parse(names, text).unwrap_or_else(|| LayoutTree::default_for(names))
    }

    /// The layout `text` holds, or `None` when it is rubbish (where [`LayoutTree::from_text`] gives the default
    /// layout): what a saved page or layout is checked with, so one that cannot be read can be dropped.
    pub fn parse_text(names: &[&str], text: &str) -> Option<LayoutTree> {
        Self::parse(names, text)
    }

    fn parse(names: &[&str], text: &str) -> Option<LayoutTree> {
        let mut lines = text.lines();
        let known = |name: &str| names.iter().position(|n| *n == name).unwrap_or(UNKNOWN);
        let header = lines.next()?;
        let root = match header {
            HEADER => {
                let mut leaves = 0;
                let root = parse_node(&mut lines, &known, 0, &mut leaves)?;
                lines.next().is_none().then_some(root)?
            }
            OLD_HEADER => parse_old(&mut lines, &known)?,
            _ => return None,
        };
        // An empty page is saved as such.
        if matches!(&root, Node::Leaf { tabs, .. } if tabs.is_empty()) {
            return Some(LayoutTree::new(Node::EMPTY, names.len()));
        }
        let root = root.pruned(&|p| p == UNKNOWN)?;
        // Some of these displays (the others are closed), each once.
        let mut tree = LayoutTree::new(root, names.len());
        let mut all = tree.panel_list();
        all.sort_unstable();
        if all.windows(2).any(|w| w[0] == w[1]) {
            return None;
        }
        // A grid saved before the tree held every display, so one that lacks a display is not one to keep.
        if header == OLD_HEADER && all != (0..names.len()).collect::<Vec<_>>() {
            return None;
        }
        tree.hold_minimums()?;
        Some(tree)
    }

    /// Move every share off a side that would leave a Panel under its minimum in the reference window. None when
    /// the Panels cannot all fit it.
    fn hold_minimums(&mut self) -> Option<()> {
        let area = [0.0, 0.0, REFERENCE_AREA[0], REFERENCE_AREA[1]];
        for _ in 0..16 {
            let mut fixes: Vec<(Vec<bool>, f32)> = Vec::new();
            self.root.walk(
                area,
                REFERENCE_GAP,
                &mut Vec::new(),
                &mut |node, rect, path| {
                    if let Node::Split { dir, ratio, a, b } = node {
                        let na = a.need(REFERENCE_GAP, MIN_PANEL);
                        let nb = b.need(REFERENCE_GAP, MIN_PANEL);
                        let (total, na, nb) = match dir {
                            Dir::Row => (rect[2], na[0], nb[0]),
                            Dir::Col => (rect[3], na[1], nb[1]),
                        };
                        let avail = total - REFERENCE_GAP;
                        if avail > 0.0 && na + nb <= avail {
                            let fixed = ratio.clamp(na / avail, 1.0 - nb / avail);
                            if (fixed - ratio).abs() > 1e-6 {
                                fixes.push((path.to_vec(), fixed));
                            }
                        }
                    }
                },
            );
            if fixes.is_empty() {
                break;
            }
            for (path, fixed) in fixes {
                if let Some(Node::Split { ratio, .. }) = self.root.at_mut(&path) {
                    *ratio = fixed;
                }
            }
        }
        let need = self.min_area(REFERENCE_GAP, MIN_PANEL);
        (need[0] <= REFERENCE_AREA[0] + 0.01 && need[1] <= REFERENCE_AREA[1] + 0.01).then_some(())
    }
}

fn parse_node<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    known: &impl Fn(&str) -> usize,
    depth: usize,
    leaves: &mut usize,
) -> Option<Node> {
    if depth > MAX_DEPTH {
        return None;
    }
    let line = lines.next()?;
    if line == "empty" {
        return (depth == 0).then_some(Node::EMPTY);
    }
    if let Some(rest) = line.strip_prefix("leaf ") {
        *leaves += 1;
        if *leaves > MAX_LEAVES {
            return None;
        }
        let (active, list) = rest.split_once(' ')?;
        let active: usize = active.parse().ok()?;
        let tabs: Vec<usize> = list.split('|').map(known).collect();
        return (active < tabs.len()).then_some(Node::Leaf { tabs, active });
    }
    let rest = line.strip_prefix("split ")?;
    let (dir, ratio) = rest.split_once(' ')?;
    let dir = match dir {
        "row" => Dir::Row,
        "col" => Dir::Col,
        _ => return None,
    };
    let ratio: f32 = ratio.parse().ok()?;
    if !ratio.is_finite() || !(MIN_RATIO..=1.0 - MIN_RATIO).contains(&ratio) {
        return None;
    }
    let a = parse_node(lines, known, depth + 1, leaves)?;
    let b = parse_node(lines, known, depth + 1, leaves)?;
    Some(Node::Split {
        dir,
        ratio,
        a: Box::new(a),
        b: Box::new(b),
    })
}

/// A grid saved as "layout 1": column and row weights, then each Panel's place.
fn parse_old<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    known: &impl Fn(&str) -> usize,
) -> Option<Node> {
    let weights = |line: &str, key: &str| -> Option<Vec<f32>> {
        let values: Vec<f32> = line
            .strip_prefix(key)?
            .split_whitespace()
            .map(|v| v.parse().ok())
            .collect::<Option<_>>()?;
        let sum: f32 = values.iter().sum();
        let sane = !values.is_empty()
            && values.len() <= MAX_LEAVES
            && values.iter().all(|v| v.is_finite() && *v > 0.0)
            && (sum - 1.0).abs() < 0.01;
        sane.then_some(values)
    };
    let columns = weights(lines.next()?, "columns ")?;
    let rows = weights(lines.next()?, "rows ")?;
    let mut cells = Vec::new();
    for line in lines {
        let mut parts = line.strip_prefix("panel ")?.splitn(5, ' ');
        let mut number = || parts.next()?.parse::<usize>().ok();
        let (column, row, across, down) = (number()?, number()?, number()?, number()?);
        cells.push(Cell {
            panel: known(parts.next()?),
            column,
            row,
            columns: across,
            rows: down,
        });
        if cells.len() > MAX_LEAVES {
            return None;
        }
    }
    tree_from_grid(&columns, &rows, &cells)
}
