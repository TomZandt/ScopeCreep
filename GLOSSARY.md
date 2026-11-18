# scopecreep

An external monitoring app for DaVinci Resolve: it receives the picture Resolve is playing and shows it alongside professional-grade scopes and picture views on any screen.

## Language

**Feed**:
The single stream of frames scopecreep receives from Resolve, equivalent to what Resolve's Video Clean Feed shows. Every Scope and Picture View is derived from the one Feed; nothing takes its own copy from Resolve.
_Avoid_: Input, stream, signal

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
A small toggle on a title bar: a letter or short word, grey when off, the accent (or its channel's colour for R, G, B, Y) when on.

**Toolbar**:
The settings a display marks to show as chips on its Panel's title bar (the Channel view's Y R G B, one at a time; the luma waveform's Y R G B, any combination with at least one on). The popover holds the settings that are not on the toolbar.

**Legend strip**:
The colour scale of a display, always horizontal: under the picture, full width, read left to right.
_Avoid_: Vertical legend

**Panel**:
One area of the Window that shows one Scope or Picture View. Every display in the Window has its own Panel, so everything can be seen at once (9 Panels; the gamut/legal warning, Highlight Neutrals, CIE plot and Black and white are out of the Window but their code and saved settings are kept); the Panels sit in an ordered grid whose borders the user can drag.
_Avoid_: Pane, view, tile, window (for one display)

**Display**:
The shape every Scope and Picture View shares in the code: created for the target format, measures or takes the Viewer Frame already on the GPU, draws into its Panel, takes its settings and the Colour Setup, reports its label, and (Scopes) can be read back for tests. A new Scope or Picture View is one module with this shape, registered once.
_Avoid_: Widget, plugin, module (for a display)

**Picture stage**:
The one part of the code every Picture View draws through. It places the frame in its Panel (fit, centre, whole device pixels), flips it, keeps the GPU binding and the legend strip; a Picture View supplies only its colour rule, its values and optionally a legend. The Channel view's Loupe is its own draw on top of it.
_Avoid_: Renderer, canvas

**Matte**:
A black bar around the picture: whole rows at the top and bottom, or whole columns at the sides, in which every pixel is exactly black. The Picture stage finds them once per frame and draws them black on every Picture View. A black pixel inside the picture is not a matte.
_Avoid_: Letterbox (that is one kind of matte), border

**Panel grid** (the layout):
The columns and rows the Panels sit in, each Panel at a cell with a span of columns and rows, every cell used. By default the clean picture spans 3 × 2 at the top left. Column and row sizes are weights, so border positions the user dragged stay valid at any window size. Each Panel has a thin border and a title bar with its label at its top.
_Avoid_: Layout (on its own), dock

**Layout** (named):
An arrangement of the Panel grid saved in one of nine slots (Ctrl+Shift+1 to 9), shown again with Ctrl+1 to 9 or from the layout menu (right-click the status line), which also renames layouts and restores the default layout. A layout names its Panels, so one still opens after Panels are added or removed.
_Avoid_: Preset, workspace, profile

**Panel settings**:
The choices a display offers in its Panel's right-click menu, each from a short list so a display can't be set up to mislead. Kept per Panel between sessions. A setting that changes what is measured is named in the Panel's label.
_Avoid_: Preferences, options, properties

**Scope**:
A display that measures the Feed's pixel values, such as a waveform, RGB parade or vectorscope.
_Avoid_: Monitor, meter, graph

**Vectorscope**:
A Scope that plots each pixel's colour as Cb across and Cr up, so hue is the angle and saturation the distance from the middle. One Panel shows it twice, side by side: at 100 % (left) the whole picture, at 200 % (right) the same data at twice the radius. One label, one right-click menu and one set of settings serve both. Cb/Cr use the Measured Space's luma weights.
_Avoid_: Chroma scope, colour wheel

**Saturation heatmap**:
A Picture View that draws the Output picture as its Luma in grey with saturation laid over it in a colour that strengthens with saturation, cool to hot: neutral areas show the grey picture, saturated areas a strong colour. Saturation is the distance from neutral on the vectorscope's scale (100 % is the vectorscope's outer ring).
_Avoid_: Chroma map, saturation false colour

**Skin-tone line**:
A line on a vectorscope from the middle at a fixed angle, where skin tones sit. Its angle is held in one shared constant (123° in BT.709 Cb/Cr, the angle Resolve 21.2's own vectorscope draws its line at, #60).
_Avoid_: I-line (that is the composite-video name), flesh line

**Skin Tone Panel**:
A Picture View of the Output picture in its own colours in which pixels whose hue is near the Skin-tone line are painted solid flat yellow (on the line), green or magenta (either side, out to the outer band); other and near-neutral pixels keep their own colour. Only hue matters, not brightness.
_Avoid_: Skin tone indicator (Resolve's and others' name for their own tools)

**Colour targets**:
The six boxes on a vectorscope where red, yellow, green, cyan, magenta and blue colour bars land, for 75 % or 100 % bars (the user's choice).
_Avoid_: Graticule (that is all the scale marks), boxes

**Luma**:
The single brightness value a Scope plots per pixel: a weighted sum of the encoded red, green and blue values, with weights set by the Measured Space's primaries and white point. A **Match Resolve** mode instead uses the weights Resolve's own waveform uses (Rec.709 weights for Rec.709 and P3 outputs, Rec.2020 weights for Rec.2020 and Rec.2100).
_Avoid_: Luminance (that is linear light, not this), brightness, Y (on its own)

**Trace density**:
How many pixels land in one spot of a waveform, measured against what that Panel and frame size lead us to expect (an even spread over all codes). The trace's brightness follows it like a phosphor scope, so the same frame looks the same brightness in a large and a small Panel.
_Avoid_: Intensity, heat

**Soft spread**:
A blur of the waveform trace of about 1 device pixel, on by default and switchable off in the Panel's settings. Drawing only: it never changes what is measured.
_Avoid_: Glow, blur, smoothing

**Picture View**:
A display that shows the Feed as an image, either untouched (the clean picture) or remapped for inspection (false colour, black and white).
_Avoid_: Scope (for false colour), filter, LUT view

**Highlight Neutrals**:
A Picture View that shows the Output picture in Luma grey and pixels whose red, green and blue are nearly equal (within a tolerance, a share of the full signal range, 3 % by default) in green, for checking that whites and greys are balanced.
_Avoid_: White balance check, neutral finder
**Black and white**:
The Picture View that shows the Output picture as greys equal to Luma, with the same weights as the waveform (including its luma weights setting), so a grey on screen lines up with its trace. Out of the Window since #118: the clean picture's B/W toggle does its job (code and tests kept).
_Avoid_: Greyscale, desaturated view, monochrome

**Colour Waveform**:
A waveform whose trace is tinted by the colour of the pixels plotted at each point, so a green scene gives a green trace. A Scope in its own Panel, beside the plain Luma waveform.
Its spots are at the luma waveform's positions; each one's tint is the average colour of the pixels counted there, brought to the neutral trace's strength so grey draws as the luma trace does.
_Avoid_: Colourised waveform, RGB waveform, parade (that shows channels side by side)

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
A movable square on a Picture View that shows the picture under it magnified, so single pixels (and noise) can be seen. Its zoom is of source pixels, in device pixels of the screen: at 200 % each picture pixel is 2 × 2 device pixels, whatever the Panel's size. It lives on the Channel view.

**Channel view**:
The Panel showing one channel of the Output picture (blue by default, or red, green or Luma) as grey, with the Loupe on top, for inspecting noise.
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
