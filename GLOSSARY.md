# scopecreep

An external monitoring app for DaVinci Resolve: it receives the picture Resolve is playing and shows it alongside professional-grade scopes and picture views on any screen.

## Language

**Feed**:
The single stream of frames scopecreep receives from Resolve, equivalent to what Resolve's Video Clean Feed shows. Every Scope and Picture View is derived from the one Feed; nothing takes its own copy from Resolve.
_Avoid_: Input, stream, signal

**Feed loop**:
The app's one step per poll of the Feed: take a new frame, convert it once, hand it to every display, measure the frame on screen again when a setting or the Colour Setup changed, and remake the labels. The app's Feed thread and the display tests run the same code.
_Avoid_: Feed thread (that is only where it runs)

**Feed Source**:
Where the Feed comes from: either the Sender or a Capture.
_Avoid_: Input, source type, tap

**Sender**:
The scopecreep OFX plugin placed in a Resolve project that hands each frame Resolve renders to scopecreep. When several Senders render the same frame, the last one in the frame (in practice the last timeline node) drives the Feed, and the app warns that more than one is active (ADR 0024).
_Avoid_: Connector, link, OFX node, tap

**Viewer Frame**:
A frame Resolve renders because it is the frame on screen, as opposed to thumbnail renders, background cache renders and repeat renders of the same frame. Only Viewer Frames reach the Scopes. Resolve flags thumbnails (the Sender never feeds them) but not cache renders, so the app tells cache renders apart by the pacing of renders: a cache fill runs faster than real time, in numbered order, often two frames at once.
_Avoid_: Current frame, live frame, real frame

**Capture**:
A Feed Source that reads Resolve's video output from Blackmagic capture hardware.
_Avoid_: Ingest, input card, hardware feed

## Colour management

**Timeline Colour Space**:
The colour space a Resolve project grades in, such as DaVinci Wide Gamut / DaVinci Intermediate or ACEScct. Pixel values here are not what the viewer sees.
_Avoid_: Working space, project colour space, log space

**Output Colour Space**:
The colour space Resolve delivers to the monitor and the render, such as Rec.709 Gamma 2.4 or Rec.2100 PQ. This is what the client sees.
_Avoid_: Display space, ODT (except when talking specifically about ACES), delivery space

**Colour Setup**:
Everything about a Resolve project's colour pipeline that the Scopes depend on: colour science mode, Timeline Colour Space, Output Colour Space, tone and gamut mapping, ACES version and output transform, and Levels. A Colour Setup always says where it came from: the project (read by script) or set manually (on the Sender node). Set manually wins.
_Avoid_: Project settings, colour config, colour pipeline

**Measured Space**:
The colour space the Scopes measure: Output Colour Space by default, or Timeline Colour Space when the user switches.
_Avoid_: Scope space, analysis space

**Output Conversion**:
Turning a Sender's Timeline Colour Space frames into Output Colour Space. Every Output Conversion is labelled either exact (matches Resolve bit for bit) or approximated (and says why).
_Avoid_: CST, transform, ODT

**Levels**:
Whether code values use the full range (data levels) or the legal range (video levels), as set in Resolve (Project Settings, Video Monitoring, Data levels). On the Scopes' 10-bit scale, video levels put black at code 64 and white at 940; data levels put them at 0 and 1023. Inside Resolve, and in the Feed, values are the same whichever Levels the project uses: Levels only say which codes they are delivered as.
_Avoid_: Range, data range, legal/full (on its own)

## Setup

**Setup Checklist**:
The short list of exact fixes that appears only while something scopecreep needs is missing, and disappears once everything works.
_Avoid_: Onboarding, wizard, warnings panel, status page

## Displays

**Window**:
The whole scopecreep app on screen: every Panel, the labels and the status line.
_Avoid_: App (for the screen area), screen, workspace

**Chrome**:
Everything the Window draws that is not a measurement: the menu, labels, scale numbers, status line, borders and gaps. It is neutral grey and draws only through the style tokens.
_Avoid_: UI, theme

**Top bar**:
The one line across the top of the Window that says the Colour Setup: Timeline → Output, Levels, where it came from, and whether Output Conversion is exact or approximated. The Panels do not repeat it.
_Avoid_: header, toolbar

**Title bar tag**:
A short word in a Panel's title bar for state that changes how to read that Panel ("approximated", "unconverted", "Rec.709 assumed", a zoom, a non-default setting). Defaults are not tagged. The Panel's full label is its hover.
_Avoid_: badge, subtitle

**Status line**:
The line along the bottom of the Window with playback and performance only: the rate, dropped frames and why, and the timeline feeding the Window. What it leaves out (Sender instance, process, versions) is its hover.
_Avoid_: footer

**Style token**:
A named colour or size by role (`TEXT_SECONDARY`, `SPACE_MARGIN`), kept in one module; chrome never uses a raw value.
_Avoid_: Theme colour, constant

**Measurement colour**:
A colour that is data, not style: a false colour band, the heatmap palette, a skin tint, a trace colour. It lives with its display and is exempt from the style check.
_Avoid_: Theme colour

**Component**:
A part of the chrome every surface shares: chip, segmented control, slider with a number, popover, tooltip, Panel frame and title bar, top bar, status bar, legend strip. Each is a pure size and hit-area description with a drawing in every state (Normal, Hover, Pressed, Disabled).
_Avoid_: Widget, control (for the whole set)

**Chip**:
A small toggle on a title bar: a letter or short word, grey when off, the accent (or its channel's colour for R, G, B, Y) when on. A row of chips behaves in one of four ways: single (one always on), multiple (any combination, at least one on), toggle (free to be off) or one or none (at most one on; clicking the lit one turns it off).

**Toolbar**:
The settings a display marks to show as chips on its Panel's title bar (the Channel view's Y R G B, one at a time (out of the Window); the luma waveform's Y R G B, any combination with at least one on; the clean picture's Y R G B, one or none, none being colour, the default, with the channel named as a tag when lit). The chips of one setting are a group, with a wider gap between groups than within one (the Vectorscope: plots, mono, tonal range). The popover holds the settings that are not on the toolbar. On a title bar too narrow for every chip, whole settings leave the bar from the right (only the chips give way, ADR 0040) and the popover offers them first; the settings button stays fixed at the right edge and nothing runs past the bar.

**Legend strip**:
The colour scale of a display, always horizontal: under the picture, full width, read left to right.
_Avoid_: Vertical legend

**Scale bar**:
The legend strip of ARRI False Colour and the saturation heatmap: a bar proportional to a range (0 to 100 % of Output Luma, or 0 to the heatmap's scale), tick numbers along its top and marks under it at the meaningful points. Ticks thin out and marks drop out on a small Panel; text never overlaps.

**Panel**:
One area of the Window that shows one Scope or Picture View. Every display in the Window has its own Panel, so everything can be seen at once, unless it is closed (Close in the Panel's popover or the Layout menu; Add panel brings it back) (7 Panels; the gamut/legal warning, Highlight Neutrals, CIE plot, Black and white, Colour Waveform and Channel view are out of the Window but their code and saved settings are kept); the Panels sit in an ordered grid whose borders the user can drag.
_Avoid_: Pane, view, tile, window (for one display)

**Display**:
The shape every Scope and Picture View shares in the code: created for the target format, measures or takes the Viewer Frame already on the GPU, draws into its Panel, takes its settings and the Colour Setup, reports its label, and (Scopes) can be read back for tests. A new Scope or Picture View is one module with this shape, registered once.
_Avoid_: Widget, plugin, module (for a display)

**Picture stage**:
The one part of the code every Picture View draws through. It places the frame in its Panel (fit, centre, whole device pixels), flips it, keeps the GPU binding and the legend strip; a Picture View supplies only its colour rule, its values and optionally a legend. The Loupe is a shared piece on top of it, built from the Panel's own colour rule (`loupe.rs`).
_Avoid_: Renderer, canvas

**Zoom**:
How much of a picture Panel's frame is shown, in percent of source pixels: 100 % is one picture pixel to one device pixel. It runs from Fit to 800 %, free below 100 % and in whole steps (200, 300 ... 800 %) above, so every picture pixel is an exact square. The scroll wheel zooms toward the pointer; a read-only zoom tag on the title bar says it, and Fit in the view strip takes every picture Panel back to Fit. It is not a setting: it is not saved and every session starts at Fit (ADR 0035). On every picture Panel in the Window, each on its own, or all together while Link is on.
_Avoid_: Magnify (that is the Loupe), scale

**Pan**:
Moving a zoomed picture inside its Panel by dragging with the middle mouse button, as in Resolve's Viewer. The picture moves exactly as far as the pointer; its edge stops at the Panel's edge, so the Panel is never left empty, and an axis on which the picture is smaller than the Panel stays centred. Nothing to pan at Fit. Not saved.

**Fit**:
The zoom that shows the whole frame in the Panel, as a picture Panel always did. The zoom tag says how much that scales the frame: "Fit 23 %". A frame already larger than 100 % at Fit (a small frame in a big Panel) starts at Fit and zooms on in whole steps. Z (a Hotkey) toggles the picture Panel under the pointer between Fit and the zoom and spot it last left, like Resolve's Shift+Z.

**Link**:
One Window-wide switch, in the view strip (lit while on). It is on in a first session; a saved choice wins. With Link on, every picture Panel shares one zoom and one centre point in picture coordinates, so Panels of different sizes look at the same pixels and each fits its own Panel at Fit; scroll, middle-drag and the view strip's Fit on any of them move all of them. Turning it on brings every Panel to the zoom and spot of the first Panel zoomed in (else the first picture Panel); turning it off leaves each where it is. Link is remembered between sessions; zoom and pan are not (ADR 0035, 0015).

**Matte**:
A black bar around the picture: whole rows at the top and bottom, or whole columns at the sides, in which every pixel is exactly black. The Picture stage finds them once per frame and draws them black on every Picture View. A black pixel inside the picture is not a matte.
_Avoid_: Letterbox (that is one kind of matte), border

**Panel grid**:
The columns and rows the Panels sit in, each Panel at a cell with a span of columns and rows, every cell used. By default the clean picture spans 3 × 2 at the top left, the luma waveform and the Vectorscope (two columns wide) beside it, and the four other Picture Views fill two rows below (ARRI False Colour 3 wide with Stops False Colour, the Saturation heatmap 3 wide with Skin tone), so all 7 Panels use every cell. Column and row sizes are weights, so border positions the user dragged stay valid at any window size. Each Panel has a thin border and a title bar with its label at its top.
_Avoid_: Layout for the grid (a Layout is a saved arrangement of it), dock

Since spec #196 (ADR 0038) the grid is replaced by the **Layout tree** below, built in #204; this entry is kept for the old saved layouts, which convert when they are a sequence of cuts.

**Layout** (named):
An arrangement of the Panel grid saved in one of nine slots (Ctrl+Shift+1 to 9 by default), shown again with Ctrl+1 to 9 or from the Layout menu on the Menu bar, which also saves, renames layouts and restores the default layout. A layout names its Panels, so one still opens after Panels are added or removed.
_Avoid_: Preset, workspace, profile

Since spec #196 (ADR 0041) named layouts become **Pages**; this entry stays until they are built.

**Layout tree**:
The arrangement of one Page: a binary tree whose leaves are Tab groups and whose inner nodes are splits, each side by side or stacked with the share of its space (net of the gap) that its first child takes. It always fills the Panel area, so no cell is empty, and a Panel can never be smaller than the minimum Panel size (ADR 0038). Decided in spec #196 and built in #204: the Window draws from it, one Panel per tab group for now, and a saved layout reads "layout 2" (an old grid converts when it is a sequence of cuts, otherwise the default layout is used).
_Avoid_: Grid (for the new model), dock, split view

**Tab group**:
A leaf of the Layout tree: one or more Panels sharing one area, one shown (its title bar holds a tab for each when there is more than one). Only the shown Panel measures and draws. A tab group with one Panel looks as a Panel does today.
_Avoid_: Pane, stack, container

**Panel instance**:
A Panel is one instance of a display with a stable id, so a display can be on the Window more than once. The first instance keeps the display's name as its settings key; further ones are "Name 2", "Name 3", numbered when created and never by position (ADR 0039).
_Avoid_: Copy (for the id), clone

**Add panel**:
A Command (Layout menu, reassignable in the Hotkeys panel, no default hotkey) that opens the list of displays; picking one splits the focused tab group along its longer side (wider than tall: the new Panel on the right; otherwise below) at 50/50 and focuses it. The focused Panel is the one last clicked or added, and has a quiet outline. A page with no Panels shows one large Add panel button. Built in #206: an add with no room is refused with a notice that names the minimum Panel size, and a display already on the page is greyed until Panels can be copied (#208). A **+** on each edge of a tab group (hover) does the same on a chosen side. Once tab groups arrive (#211), a new Panel that has no room joins the focused tab group as a tab instead, with a notice that says so and names the minimum.
_Avoid_: Insert, new window

**Dock**:
Dragging a Panel's title or tab onto another tab group: the outer quarter nearest an edge splits that edge, the middle joins the tab group as a tab, and Shift on drop swaps the two tab groups. The overlay shows the result before release; Escape cancels.
_Avoid_: Snap, drop zone (for the overlay)

**Page**:
A saved layout shown as a tab in the Page bar under the Colour Setup line: its own Layout tree, focused tab group and undo. The pages and the current page are remembered; only the current page's Panels measure and draw. New pages come from a template (All 7 Panels, Picture views, Scopes, Picture only, Empty page) or as a copy of the current one; Ctrl+1 to 9 go to pages 1 to 9, and the first nine tabs show their number, "[1] Colour" (ADR 0041).
_Avoid_: Workspace (Resolve's word for its page arrangement), slot, preset

**Auto layout**:
A button on the Page bar that re-arranges every Panel on the current Page once, for the Feed's aspect and the Window: it keeps the Panels in reading order and picks the cutting of them into splits that gives the most useful area, with the clean picture weighted most. It never runs by itself, and Undo brings the old layout back (ADR 0041).
_Avoid_: Auto-arrange, tidy, auto mode (it is not a mode)

**Auto resize**:
A button beside Auto layout that only resizes splits on the current Page so picture cells match the Feed's aspect; no Panel changes place and the nesting is unchanged (ADR 0041).
_Avoid_: Fit layout, auto fit

**View strip**:
Fit, the shared zoom readout and Link, at the right end of the menu bar row: the Window-wide view controls, always in the same place. A picture Panel keeps a read-only zoom tag (ADR 0040).
_Avoid_: View toolbar, zoom bar

**Command**:
Something the app does that a menu entry offers and a Hotkey can trigger, such as showing a layout or saving one into a slot. Every command is one entry in one table, with a label and a default Hotkey, and works from its menu with no Hotkey (ADR 0036).
_Avoid_: Action, shortcut

**Hotkey**:
A key, by its position on the keyboard, with Ctrl, Shift or Alt held, that triggers a Command while the app's window has focus. Each Command has at most one and no two share one; the defaults are Ctrl+1 to 9 (show a layout), Ctrl+Shift+1 to 9 (save one), Z (the Fit toggle) and Ctrl+F (fill the Window). Assigned, cleared and reset in the Hotkeys panel (Settings, then Hotkeys…) by pressing the combination. Changes are kept in the config folder beside the layouts. Escape, and plain Enter, Backspace and Delete are reserved.
_Avoid_: Shortcut, keybinding, accelerator

Since spec #196 (ADR 0041) Ctrl+1 to 9 go to pages 1 to 9 and Ctrl+Shift+1 to 9 (save into a slot) are removed; Ctrl+Z (Undo layout change) is added.

**Menu bar**:
The thin bar across the very top of the Window, above the Colour Setup line, drawn by the app in the style system's quiet chrome. It holds the app's own Commands in menus (Layout, View, Settings), each entry with its Hotkey beside it. A menu opens on a click on its title and closes on a second click, a click elsewhere or Escape. Panel settings stay in each Panel's popover.
_Avoid_: Toolbar (that is a Panel's title bar chips), main menu

**View menu**:
The menu on the Menu bar between Layout and Settings for display choices that cover the whole Window. Its first entry, "Show legends", shows or hides the legend strip of every Panel that has one, by setting each Panel's own Legend setting (so a Panel can still differ afterwards); it is marked while every legend is on. It runs a Command with no default Hotkey.
_Avoid_: Display menu, Window menu

**Filling the Window**:
Ctrl+F (a Hotkey) makes the Panel under the pointer take the whole Panel area, with the menu bar, Colour Setup line and status line still there; Ctrl+F again or Escape brings the Panel grid back, and so does showing a Layout. Display state, never saved; while a Panel fills the Window, borders and title bars can't be dragged.
_Avoid_: Fullscreen (the window itself never changes), maximise, solo

**Panel settings**:
What a display offers in its Panel's right-click menu (and chips on its title bar). Each is either a short list of choices or an Amount set with a Slider, so a display can't be set up to mislead. Kept per Panel between sessions. A setting that changes what is measured is named in the Panel's label.
_Avoid_: Preferences, options, properties

**Amount**:
A Panel setting that is a number rather than a list: a range, a step, a default, a unit and how many digits to show (trace brightness, soft spread, graticule brightness, a tolerance, a threshold, the heatmap scale). Trace brightness runs 20 to 300 % on the waveforms and 20 to 400 % on the Vectorscope and CIE plot, whose gain scale is wider. It is set with a Slider, kept as a plain number, always read back inside its range, and an older list value maps to the number it produced.
_Avoid_: Free number, value

**Slider**:
The row in a Panel's menu for a setting that is an amount (trace brightness, soft spread, graticule brightness, a tolerance, a threshold, the heatmap scale): a track with a handle, the current value as a number with its unit beside it, dragged with the mouse while the menu stays open, with a row to put it back to its default. Its range is limited to sensible values and the value is kept per Panel.
_Avoid_: Dial, knob, spinner

**Scope**:
A display that measures the Feed's pixel values, such as a waveform, RGB parade or vectorscope.
_Avoid_: Monitor, meter, graph

**Vectorscope**:
A Scope that plots each pixel's colour as Cb across and Cr up, so hue is the angle and saturation the distance from the middle. One Panel shows it twice, joined as one scope: at 100 % (left) the whole picture, at 200 % (right) the same data at twice the radius. The two equal square plots touch with no gap, share one background with a faint divider at the join (graticule tone), and are sized and centred together as one 2:1 pair, so spare room in the Panel is outside the pair; a single plot (100 % only or 200 % only) fills the Panel. One label, one right-click menu and one set of settings serve both. Cb/Cr use the Measured Space's luma weights.
_Avoid_: Chroma scope, colour wheel

**Tonal range**:
A chip on the Vectorscope's title bar (All by default, remembered per Panel, tagged when not All) that plots only the pixels whose Luma is in one third of the signal: Shadows below 30 %, Mids from 30 % to below 70 %, Highlights 70 % and above (Resolve's own Low and High boundaries, of Y with the Measured Space's weights; the Levels do not move them). It changes what is measured: the plot and the ring-clipping counts only include the plotted pixels. Resolve and Tektronix call it a luma-qualified vectorscope.
_Avoid_: Luma key, qualifier

**Saturation heatmap**:
A Picture View that draws the Output picture as its Luma in grey with saturation laid over it in a colour that strengthens with saturation, cool to hot: neutral areas show the grey picture, saturated areas a strong colour. Saturation is the distance from neutral on the vectorscope's scale (100 % is the vectorscope's outer ring).
_Avoid_: Chroma map, saturation false colour

**Skin-tone line**:
A line on a vectorscope from the middle at a fixed angle, where skin tones sit. Its angle is held in one shared constant (123° in BT.709 Cb/Cr, the angle Resolve 21.2's own vectorscope draws its line at, #60).
_Avoid_: I-line (that is the composite-video name), flesh line

**Skin Tone Panel**:
A Picture View of the Output picture in its own colours in which pixels whose hue is near the Skin-tone line are painted solid flat yellow (on the line), green or magenta (either side, out to the outer band); other and near-neutral pixels keep their own colour. Only hue matters, not brightness.
_Avoid_: Skin tone indicator (Resolve's and others' name for their own tools)

**Hue vectors**:
A choice of graticule on the Vectorscope, next to Standard (the default). It adds a quiet neutral spoke from the middle to each of the six Colour targets, at the angle of the target's boxes, so a colour that has drifted off its hue line is easy to see. A drawing aid only: it follows the graticule brightness and never changes the trace, the clipping marks or the Skin-tone line.
_Avoid_: Hue lines, vector lines (Resolve's name for the setting is Scale Style)

**Colour targets**:
The six boxes on a vectorscope where red, yellow, green, cyan, magenta and blue colour bars land, for 75 % bars, 100 % bars or both at once (the user's choice; both is the default, with a hue's 75 % box inside its 100 % box in the same colour).
_Avoid_: Graticule (that is all the scale marks), boxes

**Luma**:
The single brightness value a Scope plots per pixel: a weighted sum of the encoded red, green and blue values, with weights set by the Measured Space's primaries and white point. A **Match Resolve** mode instead uses the weights Resolve's own waveform uses (Rec.709 weights for Rec.709 and P3 outputs, Rec.2020 weights for Rec.2020 and Rec.2100).
_Avoid_: Luminance (that is linear light, not this), brightness, Y (on its own)

**Trace density**:
How many pixels land in one spot of a waveform, measured against what that Panel and frame size lead us to expect (an even spread over all codes). The trace's brightness follows it like a phosphor scope, so the same frame looks the same brightness in a large and a small Panel.
_Avoid_: Intensity, heat

**Scope shader library**:
The one piece of shader code the waveforms and the vectorscope share: the phosphor response, the soft spread step and the plot, grid and trace greys. The trace brightness and soft spread settings change every scope through it.
_Avoid_: Common shader, shader utils

**Graticule brightness**:
The slider on every trace display (the waveforms, the vectorscope, the CIE plot) for how bright the grid, rings, targets and scale lines are, 20 % to 300 %, default 100 %. Drawing only, never in a tag, and the trace, clipping marks and measured shapes are never touched.

**Soft spread**:
A blur of the trace on the waveforms and the Vectorscope, a Slider in the Panel's settings from 0 to 3 device pixels (default 1; 0 is off). Drawing only: it never changes what is measured.
_Avoid_: Glow, blur, smoothing

**Picture View**:
A display that shows the Feed as an image, either untouched (the clean picture) or remapped for inspection (false colour, black and white).
_Avoid_: Scope (for false colour), filter, LUT view

**Highlight Neutrals**:
A Picture View that shows the Output picture in Luma grey and pixels whose red, green and blue are nearly equal (within a tolerance, a share of the full signal range, 3 % by default) in green, for checking that whites and greys are balanced.
_Avoid_: White balance check, neutral finder
**Black and white**:
The Picture View that shows the Output picture as greys equal to Luma, with the same weights as the waveform (including its luma weights setting), so a grey on screen lines up with its trace. Not in the Window: the clean picture's Y chip does its job (code and tests kept).
_Avoid_: Greyscale, desaturated view, monochrome

**Colour Waveform**:
A waveform whose trace is tinted by the colour of the pixels plotted at each point, so a green scene gives a green trace. Out of the Window since #145 (the luma waveform's Colorise chip does its job); its code, tests and saved settings are kept.
Its spots are at the luma waveform's positions; each one's tint is the average colour of the pixels counted there, brought to the neutral trace's strength so grey draws as the luma trace does.
_Avoid_: Colourised waveform, RGB waveform, parade (that shows channels side by side)

**Colorise**:
A chip on the luma waveform's title bar (default off, remembered per Panel, never a tag) that works on whichever of Y, R, G and B are showing: Y takes the average colour of the pixels counted at each spot (the Colour Waveform's rule), R, G and B their channel colours; off, every trace is neutral. A drawing choice: counts and edge rows are the same either way. Resolve's own switch is spelt Colorize.
_Avoid_: Colourise (as a tag or setting that changes the measurement)

**Scale units**:
What the luma and Colour Waveforms' scale is written in: 10-bit codes (default), percent of the Levels' range (black 0 %, white 100 %), or nits (only for a PQ or HLG Output Colour Space; HLG means a 1000 nit display). A tag names the unit when it is not 10-bit codes. It renames positions only: the counts, the edge rows and where a trace lands never change.
_Avoid_: Scale mode, IRE (a unit of analogue video)

**Reference level lines**:
Two dashed lines across the luma and Colour Waveforms' plot, Low and High, that mark a floor and a peak (a delivery's minimum black and maximum light). Two sliders in the Panel's settings, in the Panel's scale unit, off by default (Low at the bottom of its range, High at the top), Low never above High, drawn with the graticule brightness. One pair per Panel, kept as a signal level and shown in the Panel's scale unit, so a 1000 nit line stays on 1000 nits whichever unit reads it. Drawing only: they never change a count. Resolve's own waveform calls them Show Reference Levels.
_Avoid_: Limits, peak lines (a peak is one of the two), safe levels

**False Colour**:
A Picture View that paints each pixel by the band its value falls in, leaving the rest of the picture in grey.
_Avoid_: Heatmap (for exposure), exposure overlay

**ARRI False Colour**:
False Colour with the Rec.709 percentage bands third parties publish as ARRI's (green near middle grey, pink near one stop over), measured on the Output picture. Modelled on ARRI's scheme; not ARRI's own specification.
_Avoid_: ARRI spec, LogC false colour

**Band**:
One colour of ARRI False Colour: a range of Output Luma in percent of the full-range signal. A band includes its lower edge and excludes its upper edge; the lowest is open below and the highest open above (ADR 0016).
_Avoid_: Zone (ARRI's and EL Zone's word for their own schemes)

**Stops False Colour**:
False Colour by stops above and below middle grey, in scene light decoded from the Timeline Colour Space. When that space is not known or cannot be decoded it measures the Output picture through its display curve instead, labelled approximated (and assumed). Its legend is a horizontal bar of swatches under the picture, with the stop numbers under them and captions over them.
_Avoid_: EL Zone (a trademark scopecreep does not use), zone system

**Full zones**:
Stops False Colour's default mode: 17 zones, each in its own colour: black beyond 6 1/2 stops under middle grey, then 15 zones centred on the marks -6 to +6 (whole stops, with -1/2, 18 % and +1/2 beside middle grey), and white beyond 6 1/2 stops over. A zone covers the stops up to the midpoint with its neighbour.

**Grey and +1 stop**:
Stops False Colour's chart mode: a black and white picture where pixels within a sixth of a stop of middle grey are green and within a sixth of a stop of one stop over are pink. The ColorChecker routine (neutral 5 green, neutral 6.5 pink).

**Timeline Colour Space frame**:
The Feed's frame as Resolve sends it, before Output Conversion. Stops False Colour measures this one; every other display sees the Output picture.

**Loupe**:
A movable square on a Picture View that shows the picture under it magnified, so single pixels (and noise) can be seen. Its zoom is of source pixels, in device pixels of the screen: at 200 % each picture pixel is 2 × 2 device pixels, whatever the Panel's size. It is one shared piece any Picture View can use; it is built from the Panel's colour rule, so it shows what the Panel shows. It is on the clean picture (and on the Channel view, which is out of the Window) as a toolbar chip (off by default): the clean picture's Loupe magnifies whatever its chips show (colour or one channel), is dragged anywhere in the Panel, zooms 100 to 800 % with the scroll wheel, takes its size (small, medium, large) from the popover, and is remembered per Panel. While it is on, "Loupe N %" is a tag; while it is off the clean picture takes no pointer events and the wheel zooms the picture (Zoom).

**Channel view**:
The Panel showing one channel of the Output picture (blue by default, or red, green or Luma) as grey, with the Loupe on top, for inspecting noise. Out of the Window since #162 (the clean picture's Y R G B chips and Loupe do its job, ADR 0034); its code, tests and saved settings are kept.
_Avoid_: PiP, magnifier, zoom window

**ColorChecker frame**:
A test picture of a ColorChecker Classic chart (six patches across, four down), built in code in a named Timeline Colour Space and fed through the synthetic Feed writer. Every Patch carries the values it should read, so every display can be checked against it. Never stored as an image file.
_Avoid_: Test image, chart file, colour-nuke EXR

**Patch**:
One of the 24 coloured squares of a ColorChecker frame, numbered 1 to 24 left to right and top to bottom. The six greys are 19 to 24; neutral 5 (22) is 19.06 % Y, not 18 %.
_Avoid_: Swatch, chip, tile

**Gamut/legal warning**:
A Picture View showing the Output picture's Luma in black and white, with problem pixels coloured: red from the upper threshold up to legal white (solid above it), blue from the lower threshold down to legal black (solid below it). Every channel is checked as well as Luma, so a colour outside the Output Colour Space's gamut is caught. Percentages are of the Levels' range.
_Avoid_: Zebras, clipping indicator, safe colour

**EBU R103 tolerance**:
An option on the gamut/legal warning that widens the limits from 0 % and 100 % to -5 % and 105 %, the excursion the EBU's R103 recommendation allows.
_Avoid_: Broadcast safe

**CIE plot**:
A Panel placing each pixel's chromaticity on the CIE 1931 xy diagram, with the spectral locus and the gamut triangles of the Output and Timeline Colour Spaces.
_Avoid_: Gamut scope, horseshoe, chromaticity scope

**Gamut triangle**:
The triangle of a colour space's red, green and blue primaries on the CIE plot. A pixel outside the Output Colour Space's triangle is outside what the deliverable can show.
_Avoid_: Gamut box, colour space outline
