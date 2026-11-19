# The Skin Tone Panel paints solid flat colours on the natural picture

ADR 0021 made the Skin Tone Panel a grey picture with tints that faded into each other and back to grey. Compared with MonoNodes' skin tone indicator (#101) the tints were too weak to read at a glance, and the grey picture lost the shape and colour of the scene.

## Decision

- The picture is drawn in its own (Output) colours, not grey.
- Within `t` of the skin-tone line a pixel is solid yellow; from `t` out to 6 t it is solid green (hue above the line) or solid magenta (below it). No blending and no fade. Beyond 6 t, and where Cb/Cr length is under 0.02, it keeps its own colour.
- The tolerance choices, the bands (6 t is 30 degrees at the default) and the shared 123 degree line are unchanged. This replaces the tint and fade rules of ADR 0021; its tolerance is now a slider from 1 to 12 degrees, default 5 (#88).
- No lightness floor: the colours are flat, so dark skin reads exactly as light skin does.

## Considered and rejected

- **Stronger tints mixed with grey, with a lightness floor:** the first version of #101; the product owner chose the MonoNodes look instead.

## Consequences

- The edge between yellow and green or magenta is a hard step, and a noisy hue flickers at an edge.
- A scene's own yellow, green or magenta near the bands' colours is not told apart from painted pixels by colour alone.
