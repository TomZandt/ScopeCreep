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
Everything about a Resolve project's colour pipeline that the Scopes depend on: colour science mode, Timeline Colour Space, Output Colour Space, tone and gamut mapping, ACES version and output transform, and Levels. A Colour Setup always says where it came from: the host, the project, or set manually.
_Avoid_: Project settings, colour config, colour pipeline

**Measured Space**:
The colour space the Scopes measure: Output Colour Space by default, or Timeline Colour Space when the user switches.
_Avoid_: Scope space, analysis space

**Output Conversion**:
Turning a Sender's Timeline Colour Space frames into Output Colour Space. Every Output Conversion is labelled either exact (matches Resolve bit for bit) or approximated (and says why).
_Avoid_: CST, transform, ODT

**Levels**:
Whether code values use the full range (data levels) or the legal range (video levels), as set in Resolve.
_Avoid_: Range, data range, legal/full (on its own)

## Setup

**Setup Checklist**:
The short list of exact fixes that appears only while something scopecreep needs is missing, and disappears once everything works.
_Avoid_: Onboarding, wizard, warnings panel, status page

## Displays

**Scope**:
A display that measures the Feed's pixel values, such as a waveform, RGB parade or vectorscope.
_Avoid_: Monitor, meter, graph

**Luma**:
The single brightness value a Scope plots per pixel: a weighted sum of the encoded red, green and blue values, with weights set by the Measured Space's primaries and white point.
_Avoid_: Luminance (that is linear light, not this), brightness, Y (on its own)

**Picture View**:
A display that shows the Feed as an image, either untouched (the clean picture) or remapped for inspection (false colour, black and white).
_Avoid_: Scope (for false colour), filter, LUT view
