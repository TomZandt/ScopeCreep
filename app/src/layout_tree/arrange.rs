//! The search behind Auto layout (#214, ADR 0041). The page's tab groups, in reading order, are the items; every
//! way of cutting that ordered list into splits is a candidate (all of them up to [`FULL_SEARCH_MAX`] items, then
//! only the first cuts near the middle, up to [`SEARCH_MAX`]); above that a greedy halving replaces the search.
//! Each candidate gets its shares from the items' weights, is fitted as Auto resize fits (picture cells get the
//! footage's aspect), is dropped when a Panel would go under its least size, and the best score wins. Ties go to
//! the candidate generated first, so the result is deterministic.

use super::{Dir, Fit, NoFit, Node, halves};
use crate::auto_layout::{self, FULL_SEARCH_MAX, Item, SEARCH_MAX};
use crate::auto_resize::Shape;
use crate::panel_grid::min_panel_size;

struct Search<'a> {
    /// The tab groups in reading order.
    leaves: Vec<Node>,
    /// Each tab group's weight and display item (its shown Panel's).
    weights: Vec<f32>,
    shown: Vec<Item>,
    shapes: &'a [Shape],
    aspect: f32,
    scale: f32,
    area: [f32; 4],
    gap: f32,
    min: [f32; 2],
}

/// The best arrangement of `root`'s tab groups, `Ok(None)` when there is nothing to arrange (one tab group or
/// none), `Err` when no arrangement keeps every Panel at its least size.
pub fn arrange(
    root: &Node,
    items: &[Item],
    aspect: f32,
    area: [f32; 4],
    gap: f32,
    scale: f32,
) -> Result<Option<Node>, NoFit> {
    let mut leaves = Vec::new();
    collect(root, &mut leaves);
    if leaves.len() < 2 {
        return Ok(None);
    }
    let fallback = Item {
        shape: Shape::Other,
        weight: auto_layout::WEIGHT_OTHER,
        round: false,
    };
    let shown: Vec<Item> = leaves
        .iter()
        .map(|leaf| match leaf {
            Node::Leaf { tabs, active } => tabs
                .get(*active)
                .and_then(|p| items.get(*p))
                .copied()
                .unwrap_or(fallback),
            Node::Split { .. } => fallback,
        })
        .collect();
    let shapes: Vec<Shape> = items.iter().map(|i| i.shape).collect();
    let search = Search {
        weights: shown.iter().map(|i| i.weight).collect(),
        shown,
        leaves,
        shapes: &shapes,
        aspect,
        scale,
        area,
        gap,
        min: min_panel_size(scale),
    };
    search.run().map(Some).ok_or(NoFit)
}

fn collect(node: &Node, out: &mut Vec<Node>) {
    match node {
        Node::Leaf { .. } => out.push(node.clone()),
        Node::Split { a, b, .. } => {
            collect(a, out);
            collect(b, out);
        }
    }
}

impl Search<'_> {
    fn run(&self) -> Option<Node> {
        let n = self.leaves.len();
        if n > SEARCH_MAX {
            return self.finish(self.greedy(0, n, self.area[2], self.area[3]));
        }
        let mut best: Option<(f32, Node)> = None;
        for candidate in self.generate(0, n) {
            if let Some((score, tree)) = self.evaluate(candidate)
                && best.as_ref().is_none_or(|(b, _)| score > *b)
            {
                best = Some((score, tree));
            }
        }
        best.map(|(_, tree)| tree)
    }

    fn weight(&self, lo: usize, hi: usize) -> f32 {
        self.weights[lo..hi].iter().sum()
    }

    fn split(&self, dir: Dir, lo: usize, mid: usize, hi: usize, a: Node, b: Node) -> Node {
        let (wa, wb) = (self.weight(lo, mid), self.weight(mid, hi));
        Node::Split {
            dir,
            ratio: wa / (wa + wb),
            a: Box::new(a),
            b: Box::new(b),
        }
    }

    /// Every cutting of items `lo..hi` into splits: where to cut, which way each split goes.
    fn generate(&self, lo: usize, hi: usize) -> Vec<Node> {
        let n = hi - lo;
        if n == 1 {
            return vec![self.leaves[lo].clone()];
        }
        let cuts: Vec<usize> = if n > FULL_SEARCH_MAX {
            let mut cuts = vec![n / 2];
            if n % 2 == 1 {
                cuts.push(n.div_ceil(2));
            }
            cuts
        } else {
            (1..n).collect()
        };
        let mut out = Vec::new();
        for k in cuts {
            let (firsts, seconds) = (self.generate(lo, lo + k), self.generate(lo + k, hi));
            for a in &firsts {
                for b in &seconds {
                    for dir in [Dir::Row, Dir::Col] {
                        out.push(self.split(dir, lo, lo + k, hi, a.clone(), b.clone()));
                    }
                }
            }
        }
        out
    }

    /// The fallback above [`SEARCH_MAX`]: halve the ordered list, cutting across the longer side of the room.
    fn greedy(&self, lo: usize, hi: usize, w: f32, h: f32) -> Node {
        if hi - lo == 1 {
            return self.leaves[lo].clone();
        }
        let mid = lo + (hi - lo).div_ceil(2);
        let share = self.weight(lo, mid) / self.weight(lo, hi);
        let dir = if w >= h { Dir::Row } else { Dir::Col };
        let ([wa, ha], [wb, hb]) = match dir {
            Dir::Row => ([w * share, h], [w * (1.0 - share), h]),
            Dir::Col => ([w, h * share], [w, h * (1.0 - share)]),
        };
        let (a, b) = (self.greedy(lo, mid, wa, ha), self.greedy(mid, hi, wb, hb));
        self.split(dir, lo, mid, hi, a, b)
    }

    /// Fit a candidate and score it; None when it cannot keep every Panel at its least size.
    fn evaluate(&self, tree: Node) -> Option<(f32, Node)> {
        let tree = self.finish(tree)?;
        let score = self.score(&tree, self.area, &mut 0);
        Some((score, tree))
    }

    fn finish(&self, mut tree: Node) -> Option<Node> {
        let need = tree.floor(self.gap, self.min);
        if need[0] > self.area[2] || need[1] > self.area[3] {
            return None;
        }
        let fit = Fit {
            shapes: self.shapes,
            aspect: self.aspect,
            scale: self.scale,
            min: Some(self.min),
        };
        tree.fit(self.area, self.gap, &fit);
        Some(tree)
    }

    /// The score of `tree` in `rect`. The tab groups come in reading order, so `next` counts them off.
    fn score(&self, tree: &Node, rect: [f32; 4], next: &mut usize) -> f32 {
        match tree {
            Node::Leaf { .. } => {
                let item = self.shown[*next];
                *next += 1;
                auto_layout::score(item, rect, self.aspect, self.scale)
            }
            Node::Split { dir, ratio, a, b } => {
                let [ra, rb] = halves(*dir, rect, self.gap, *ratio);
                let first = self.score(a, ra, next);
                first + self.score(b, rb, next)
            }
        }
    }
}
