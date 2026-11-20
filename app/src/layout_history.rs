//! Undo for layout changes (ADR 0038, #207): the last [`DEPTH`] arrangements of one [`LayoutTree`]. Session only,
//! never saved. It holds the arrangement and nothing else, so undo never touches a Panel's settings or state. Each
//! tree has its own history (a value the owner of the tree keeps, not a global), so a Page can own one later.
use std::collections::VecDeque;

use crate::layout_tree::LayoutTree;

/// How many layout changes can be undone.
pub const DEPTH: usize = 20;

/// The arrangements a tree had before its last changes, newest last.
#[derive(Clone, Debug, Default)]
pub struct LayoutHistory {
    steps: VecDeque<LayoutTree>,
    /// The arrangement when a divider drag began: the drag is one step, taken at its end.
    drag: Option<LayoutTree>,
}

impl LayoutHistory {
    fn push(&mut self, before: LayoutTree) {
        if self.steps.len() == DEPTH {
            self.steps.pop_front();
        }
        self.steps.push_back(before);
    }

    /// Run `change` on `tree`; when it changed the arrangement, that is one step. The focus alone is not a change.
    pub fn change<R>(
        &mut self,
        tree: &mut LayoutTree,
        change: impl FnOnce(&mut LayoutTree) -> R,
    ) -> R {
        let before = tree.clone();
        let result = change(tree);
        if *tree != before {
            self.push(before);
        }
        result
    }

    /// A divider drag begins: remember the arrangement.
    pub fn begin_drag(&mut self, tree: &LayoutTree) {
        self.drag = Some(tree.clone());
    }

    /// The drag is let go: when it moved the divider, the whole drag is one step.
    pub fn end_drag(&mut self, tree: &LayoutTree) {
        if let Some(before) = self.drag.take()
            && before != *tree
        {
            self.push(before);
        }
    }

    /// The display of index `removed` is gone for good (a closed duplicate, ADR 0039): drop its Panel from every
    /// arrangement kept and move the Panels after it up one index, so each still names the Panel it did.
    pub fn forget_index(&mut self, removed: usize) {
        let forget = |tree: &mut LayoutTree| {
            tree.close(removed);
            tree.forget_index(removed);
        };
        self.steps.iter_mut().for_each(forget);
        self.drag.iter_mut().for_each(forget);
    }

    /// Whether an arrangement kept for undo holds Panel `panel`: undoing to it would put the Panel back, so the
    /// Panel must not be removed yet.
    pub fn names(&self, panel: usize) -> bool {
        self.steps
            .iter()
            .chain(self.drag.iter())
            .any(|t| t.contains(panel))
    }

    /// Whether there is a change to undo.
    pub fn can_undo(&self) -> bool {
        !self.steps.is_empty()
    }

    /// Put back the arrangement before the last change. False when there is nothing to undo.
    pub fn undo(&mut self, tree: &mut LayoutTree) -> bool {
        match self.steps.pop_back() {
            Some(before) => {
                *tree = before;
                true
            }
            None => false,
        }
    }
}
