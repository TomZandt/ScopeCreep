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
The scopecreep OFX plugin placed in a Resolve project that hands each frame Resolve renders to scopecreep.
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

**Panel**:
One area of the Window that shows one Scope or Picture View. Every display has its own Panel, so everything can be seen at once; the Panels sit in an ordered grid whose borders the user can drag.
_Avoid_: Pane, view, tile, window (for one display)

**Display**:
The shape every Scope and Picture View shares in the code: created for the target format, measures or takes the Viewer Frame already on the GPU, draws into its Panel, takes its settings and the Colour Setup, reports its label, and (Scopes) can be read back for tests. A new Scope or Picture View is one module with this shape, registered once.
_Avoid_: Widget, plugin, module (for a display)

**Panel grid**:
The ordered grid the Panels sit in. Its shape depends only on how many displays there are (rows as equal as possible, every row filling the width), so border positions the user dragged stay valid at any window size.
_Avoid_: Layout (on its own), dock

**Panel settings**:
The choices a display offers in its Panel's right-click menu, each from a short list so a display can't be set up to mislead. Kept per Panel between sessions. A setting that changes what is measured is named in the Panel's label.
_Avoid_: Preferences, options, properties

**Scope**:
A display that measures the Feed's pixel values, such as a waveform, RGB parade or vectorscope.
_Avoid_: Monitor, meter, graph

**Luma**:
The single brightness value a Scope plots per pixel: a weighted sum of the encoded red, green and blue values, with weights set by the Measured Space's primaries and white point. A **Match Resolve** mode instead uses the weights Resolve's own waveform uses (Rec.709 weights for Rec.709 and P3 outputs, Rec.2020 weights for Rec.2020 and Rec.2100).
_Avoid_: Luminance (that is linear light, not this), brightness, Y (on its own)

**Picture View**:
A display that shows the Feed as an image, either untouched (the clean picture) or remapped for inspection (false colour, black and white).
_Avoid_: Scope (for false colour), filter, LUT view

**Highlight Neutrals**:
A Picture View that shows the Output picture in Luma grey and pixels whose red, green and blue are nearly equal (within a tolerance, a share of the full signal range, 3 % by default) in green, for checking that whites and greys are balanced.
_Avoid_: White balance check, neutral finder

**Colour Waveform**:
A waveform whose trace is tinted by the colour of the pixels plotted at each point, so a green scene gives a green trace. A Scope in its own Panel, beside the plain Luma waveform.
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
False Colour by stops above and below middle grey, in scene light decoded from the Timeline Colour Space.
_Avoid_: EL Zone (a trademark scopecreep does not use), zone system

**Loupe**:
A movable square on a Picture View that shows the picture under it magnified, so single pixels (and noise) can be seen. Its zoom is of source pixels, in device pixels of the screen: at 200 % each picture pixel is 2 × 2 device pixels, whatever the Panel's size. It lives on the Channel view.

**Channel view**:
The Panel showing one channel of the Output picture (blue by default, or red or green) as grey, with the Loupe on top, for inspecting noise.
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
