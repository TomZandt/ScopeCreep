# #215: Page templates

What ticket #215 showed on 2026-11-20, at the pure layout seam (`app/tests/page_templates.rs`) and the pure UI seam (`app/tests/menu_bar_seam.rs`), headless. Nothing was checked at the app: the menu on a real window and the look of each template page with real footage are still to be tried by a person.

- Templates are `page_templates::PageTemplate` (All 7 Panels, Picture views, Scopes, Picture only), each naming its displays in reading order. `PageTemplate::tree(panels, items, aspect, area, gap, scale)` builds `LayoutTree::automatic` over those Panels and runs `LayoutTree::auto_layout` once. With no fit (a Window too small) the plain halving is kept, so the page always holds every Panel.
- At 16:9, 2.39:1 and 9:16, at 1x and 2x in a 1256 x 716 Window, every template Panel is at least the least Panel size; the Scopes page gives the clean picture the most room and differs between 16:9 and 9:16.
- A page from a template is named after it (`Pages::new_named`), is renamable, has its own tree and undo, and is saved like any page. The Window picks each display's first instance, or makes a further instance when another page holds the first, so a page owns its Panels (as Default layout does); the settings of a new instance are the display's defaults. The template's aspect is the Feed frame's on screen, 16:9 with none.
- The new-page menu lists Empty page, Default layout (7 Panels), Copy of this page, then the four templates, then Back; template entries have no Command or hotkey. It fits under the bar of the smallest Window at 1x and 2x.
- All 7 Panels differs from Default layout only in being arranged by Auto layout instead of the fixed grid.
