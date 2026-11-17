# #60 Where does Resolve draw its vectorscope skin-tone line?

Measured on 2026-11-17 from a screenshot of DaVinci Resolve 21.2 Studio's own vectorscope with "Show Skin Tone Indicator" on, taken by the product owner on real footage (a face-and-skin clip); the vectorscope was close to circular (374 × 365 px).

## Method

The picture's pixels were read from the screenshot. The middle of the plot was found from the two crosshair lines. Along rays from the middle, at radii 70 to 140 px in steps of 10, the angle (0 at the right, counter-clockwise, as `atan2(Cr, Cb)`) with the strongest thin bright line was taken.

## Result

The line is drawn at **123.3° ± 0.5°** on screen, the same at every radius (122.5° to 124.0°). That is the traditional skin-tone I-line angle (123°), read on the vectorscope's own screen axes. It is not the 134.5° that the composite U/V angle gives when rescaled to BT.709 Cb/Cr (the first assumption, ADR 0018).

scopecreep draws the line at 123°. Against it the ColorChecker's dark skin sits −2.3° and light skin −5.2° (Davinci Intermediate chart, Rec.709 output), so skin lands on the line, as it should.

## Limits

- One screenshot, one project. The plot was 2.5 % wider than tall, which moves the angle by under half a degree.
- Measured by reading the picture, not by scripting Resolve: its scripting API has no scope controls.
