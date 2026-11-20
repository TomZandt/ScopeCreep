# Tiling layout prototype (throwaway, never merged)

A single self-contained HTML page: open `index.html` in a browser. It is a stand-in for scopecreep's Window, built to answer one question before the spec was written.

**The question.** Can the Window behave like a tiling window manager: one panel fills it, each new panel splits the focused one along its longer side, panels can be added from any edge or dragged where the user wants them, the same display can appear several times, one press re-arranges the page for the footage's shape, and several pages each keep their own layout?

**What it answers.** The layout rules and the feel of them: dwindle auto-insert, edge "+" with a display list, drag-to-dock (edge splits, middle tabs), duplicate displays, Auto layout (one press, per page), Auto resize (split sizes only, per page), pages as tabs, the minimum-size refusal, and tab groups that stop measuring while hidden. Six guided walkthroughs plus two for Auto layout and Pages.

**What it does not answer.** Anything about GPU cost, a real Feed, the app's own drawing or input code, HiDPI, or the Window view strip (decided after the last version of this page).

**The logic.** The code between the `<tiler>` markers is a pure module (a plain tree and functions over it) written to be lifted into the app. The page around it is throwaway.

**Verdict.** Judged "really nice and user friendly" by the product owner on 2026-11-20. The decisions are in ADRs 0038 to 0041 and spec #196; this branch is kept only as the primary source. Related: #194 (help page and layout tutorial, which could grow from these walkthroughs), #195 (readability), #197 (cost of copies of a display).
