# The skin-tone line's angle is assumed, held in one shared constant, and never labelled as matching Resolve

Spec #49 asks for a skin-tone line "at the same angle as Resolve's vectorscope". That angle has not been measured: the ticket that will (#60) needs a person at Resolve, and it blocked the vectorscope.

## Decision

- The vectorscopes draw the line at **134.5°** in BT.709 Cb/Cr (`atan2(Cr, Cb)`, 0 at +Cb, counter-clockwise): the traditional skin-tone I-line (123° in the composite U/V plane) rescaled to Cb/Cr, from the ColorChecker research.
- The angle lives in **one** public constant, `SKIN_TONE_LINE_DEGREES` in `app/src/skin_tone.rs`, marked `TODO(#60)`. The Skin Tone Panel (#63) uses the same constant, so the displays cannot disagree.
- The label states the angle and that it is **assumed** ("skin-tone line 134.5° (assumed)"), and nothing in the app or docs says it matches Resolve (ADR 0007). When #60 is verified, the constant changes and the label may then say what was proven.

## Considered and rejected

- **Wait for #60:** the rest of the vectorscope does not depend on it, and the angle is one number.
- **Label it "Resolve's angle":** claims something unproven.

## Consequences

- #60 stays open until a person measures Resolve's line; if it differs, only the constant (and its label wording) changes.
- The 100 % and 200 % vectorscopes draw their rings and the skin-tone line in the same place on screen; the 200 % Panel's data is simply twice as far out.
