# Colorise on the waveform: why one chip instead of a Colour Waveform Panel

Research of 2026-11-18 for the luma waveform and Colour Waveform (spec #113). Quick pass, public sources only.

## What Resolve does

- The waveform scope has Y, CbCr and RGB modes (Resolve 20 manual, the repo's own earlier reading, `panel-controls.md` section 2, R20 pp. 3019 to 3020). In RGB, the R, G and B buttons toggle individually "to see any combination of graphs".
- **Colorize** is one checkbox on the waveform. Resolve 18.6 manual wording, as quoted by a third-party mirror: it lets you view the overlaid graphs "in monochrome or false colour"; on the Y option it shows "where the image's colours sit on the graph". With it off, graphs are white.
- A Resolve 21 guide (davinciresolve21.com, 2026) says Colorize renders the trace "in false red, green and blue instead of monochrome" and applies to the waveform and parade.
- So Colorize is one switch for the whole scope: on a Y trace it takes the colour of the image, on R, G and B traces it takes the channel colour, off is white.

## How sure this is

- The Resolve 20 and 21 manuals were not read directly this time. The 18.6 manual pages could not be fetched (404); the wording came from search snippets of a mirror and from the repo's earlier reading of R20.
- The two sources describe different colours for Colorize (image colour on Y, false RGB on channels). That fits one switch with a different colour source per trace, but it is an inference. **Checked at Resolve 21 (#140, 2026-11-20):** it does not hold. Resolve's waveform is in one of three exclusive modes (Y, CbCr or RGB); Colorize is one switch inside whichever mode is chosen, and it stays set when the mode changes. scopecreep keeps its Colorise chip as a separate remembered switch and its Y R G B combinations, a superset of Resolve's (ADR 0032, #202).

## What follows for scopecreep

- The Colour Waveform Panel is the luma waveform's counts with an image-colour tint (ADR 0022). With Y R G B chips on the luma waveform (#117) it is the Colorise switch on that one waveform: a second Panel is redundant.
- This reverses the "Drop on this Panel" row in `panel-controls.md` section 2, which kept the two apart; that row said a merge would become a toolbar toggle, default off.
