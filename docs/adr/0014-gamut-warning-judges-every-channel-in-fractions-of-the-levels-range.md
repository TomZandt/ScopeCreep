# The gamut/legal warning judges every channel, in fractions of the Levels' range

## Decision

- A pixel is solid red or blue when **any channel**, not only Luma, is past the upper or lower limit of the Output picture. Luma alone fades: from the threshold to the limit a pixel is tinted in proportion, and past the limit it is solid.
- Thresholds are fractions of the Levels' range, and the maths is the same for video and data Levels: inside Resolve and in the Feed 0.0 is black and 1.0 is white whichever Levels are set (see Levels in the glossary). The Levels decide only the codes the label names (90 % is code 852 at video levels, 921 at data levels).
- EBU R103 tolerance moves both limits (not the thresholds) to -5 % and 105 %, for Luma and channels alike.
- Luma weights come from the Output Colour Space's primaries (ADR 0004) and are not a setting here.
- The thresholds are lists of choices (10, 5, 15, 20 % and 90, 95, 85, 80 %), per ADR 0013, not free numbers.

## Considered and rejected

- **Luma only:** misses a saturated colour that clips one channel while Luma looks fine, which is what out-of-gamut means after Output Conversion.
- **Fading channels too:** a channel inside its limits is not a problem however close it is, so only crossing a limit is marked.
- **Free numeric thresholds:** a typed number can be nonsense (ADR 0013).

## Consequences

- The picture is sampled nearest-neighbour when a Panel is smaller than the frame, so a single offending pixel can fall between samples at a reduced size.
- A pixel with one channel above its limit and another below shows red.
