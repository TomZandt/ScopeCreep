# Spec #196: the integrated Window at the real app (UHD monitor)

Seen by the product owner on 2026-11-20 with the integrated build, and what was found and fixed. There is no ticket for this round; #217 (the empty Colour Setup line) is covered here.

## #217: the Colour Setup line was empty

Cause: the Window draws in layers, the base text first, then the flat rectangles, then the overlay text. The Colour Setup line's text was in the base text layer, and the bar's own fill (a flat rectangle) was added to the rectangles after it, so the fill covered the text. Nothing was clipped or missing, which is why brightening the pixels found no hidden text. It was the same at every scale and state, and with no Resolve.

Fix: the bar is one list of drawing parts (`window_text::top_bar_prims`: fill, line under it, text) drawn in the overlay layer, text after fill. While a menu is open over the bar the line is left out, because overlay text draws above every fill and would show through the menu. Seam test: the text comes after the fill in the same list, at 1x, 1.5x and 2x. What the layer order puts on screen is not reachable without a window: confirm the line reads at the app.

## Menus opened from a button dropped at the top left

The Page bar's **+** and Add panel button already gave the menu an anchor (the button's rectangle as last drawn). The edge **+** and the empty page's Add panel button gave none, so their list fell back to dropping under the Layout title at the top left: that is the list in the owner's screenshot. The Page bar's own buttons are now anchored from the bar as it is at the click (`PageBar::anchor`), not from where it was last drawn, and every button that opens a list anchors it (`DropMenu::rect_near`: under the button, left edge on the button's, kept inside the Window, above the button when it does not fit below). Seam tests at 1x, 1.5x, 2x and a 3840 wide Window.

## The freeze: not reproduced; one real stall removed

Looked at first, with evidence:
- A stress test (`display_lock_stress`) runs the Feed thread (frames, measuring) against a Window thread making, hiding, drawing and removing Panels, with the settings lock held while Panels are made or removed as the Window does. It ran 4 seconds, three times, with the old `set_shown` (a write lock on every draw) and with the fix, and neither thread ever stopped. So lock ordering between the shared display set, the settings and the zoom Link does not deadlock under this load.
- `set_shown` did bump nothing when nothing changed, so the Feed thread was not re-measuring every frame; but it took the display set's write lock on every draw, so the Window thread waited behind the Feed thread's read lock (held for all of a frame's measuring) at every frame. That is a stall, not a deadlock, and more Panels measuring (strays) make it longer. Fixed: look under the read lock first, write only when the shown Panels changed.
- The auto layout search is fast (9 Panels well under a second, existing tests) and sweeping and instance creation are bounded.

What is not known: the freeze the owner saw. No stack was available. Next time it freezes, run `eu-stack -p $(pgrep scopecreep)` (or `gdb -p ... -batch -ex "thread apply all bt"`) before closing it, and attach it to a new issue with what was clicked just before.

## Panel instance numbers and strays

The migration from named layouts left Main and the layouts on the same first instances, so "Default layout" on Main (which takes a new instance for any first instance another page holds) gave Main instances 2 and 3. Now, at start-up, every page is given Panels of its own in page order (`Pages::own_instances`): Main on the first instances (where its saved settings are kept), the next page the next instances, and anything no page holds is swept. A title adds an instance number only where the page holds two or more of that display (`pages::title_names`); keys and ids are unchanged. The Add list shows each display once (`pages::add_offers`) and adding one takes an instance no page holds, else makes a new one (`pages::instance_to_add`). A clean profile gives exactly one page, and only saved layouts become pages (`Pages::migrate`): the two extra pages in the owner's screenshot were two slots the owner had saved ("Layout 1", "Layout 2"), not empty or default ones.

## Owner decisions made in the same round

The view strip readout, the default layout, and the display names are in the changelog, the glossary's Displays table and ADRs 0038, 0040 and 0041. The new default layout's least Window size is 759 × 667.
