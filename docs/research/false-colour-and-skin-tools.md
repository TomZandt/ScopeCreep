# False colour, exposure and skin tone tools: what exists (November 2026)

Desk research for the milestone 2 design, done on 2026-11-16 from public sources. Claims marked *inferred* were not stated by a source. None of this has been tested against scopecreep. Decisions drawn from it: ADR 0009 and ADR 0010.

## ARRI false colour

- **ARRI's own specification** is *ARRI LogC False Color Exposure Zones and Key Specification* (Sean Cooper, ARRI, v2025-02-04): <https://www.arri.com/resource/blob/390448/2d469ae1e98110562fe347cea50a284b/arri-logc-false-color-specification-data.pdf>.
  - It defines six zones on the **LogC signal**, using luma A′ = 0.2126 R′ + 0.7152 G′ + 0.0722 B′ of the normalised LogC values. A pixel outside every zone is shown as grey (A′, A′, A′).

    | Zone | Meaning |
    |---|---|
    | red | 1/3 stop below clipping |
    | yellow | 2/3 stop below clipping |
    | pink | 1 stop above 18 % grey |
    | green | 18 % grey |
    | blue | edge of shadow detail, SNR ≤ 3 |
    | purple | noise floor, SNR ≤ 1 |

  - Green and pink are fixed (worked out at EI 400). Red, yellow, blue and purple move with the camera's EI.
  - Tables are given for every EI step, LogC3 and LogC4 (ALEXA 35), legal and full range. LogC4 also comes with its derivation from sensor-linear values.
  - Example, LogC3 at EI 800 in 10-bit legal range: red 878–940, yellow 857–878, pink 461–480, green 397–415, blue 152–172, purple 64–152.
- **Others may implement it.** ARRI writes that the specification would "allow a user to implement the algorithm into their software or hardware". Developer contact: arriraw-dev@arri.de. No licence terms are stated.
- **The familiar percentage table** is published as "ARRI" by third parties, not as text by ARRI: SmallHD's ARRI COLOR SPECTRUM (<https://guide.smallhd.com/a/1668559-exposure-assist-false-color>) and the Sigma fp manual.

  | Colour | Band |
  |---|---|
  | purple | 0–2.5 % |
  | blue | 2.5–4 % |
  | green | 38–42 % |
  | pink | 52–56 % |
  | yellow | 97–99 % |
  | red | 99–100 % |

  It describes the older Rec.709 display version (*inferred*: in LogC3 code values, pink sits nearer 45–47 %).
- **Shipped by** SmallHD (ARRI COLOR SPECTRUM), Atomos (log or RAW input only) and Driftwood's "ARRI EI Zone" Resolve OFX plugin.

## EL Zone System

- **Owner:** a trademark of Cinecam, Inc. (Ed Lachman, ASC); <https://www.elzonesystem.com>. The Sigma fp manual states "EL ZONE™ and EL ZONE SYSTEM™ are trademarks of Cinecam, Inc."
- **Licensing:** case by case, with no public terms. Contact is ELZoneSystem@gmail.com or the site's contact form.
- **Who ships it:**
  - Shipping: SmallHD, Atomos, Accsoon and Sony monitors, plus Sony Venice 2, RED, Panasonic and Sigma fp cameras.
  - "In discussions": ARRI, Blackmagic, Canon, Fujifilm and others.
- **Definition, from third-party sources:**
  - Exposure in stops from 18 % grey, from −6 to +6 stops: 15 zones, with half-stop zones either side of grey.
  - Grey means middle grey; white and black mean far over and far under.
  - It needs a camera log signal decoded with that camera's curve. SmallHD only offers it with a log input.
  - Sources: <https://www.newsshooter.com/2023/03/06/el-zone-exposure-system-how-does-it-work-and-how-do-you-use-it/>, <https://guide.smallhd.com/a/808517-exposure-assist-false-color>.
- **Not found:** the full stop-to-colour table as text. It only appears in images.

## MonoNodes Utility DCTLs

<https://mononodes.com/utility-dctl/>. €99, an encrypted DCTL for Resolve Studio, © Stefan Ringelschwandtner. It publishes no maths.

- **Input:** "Place the DCTLs after your Log to Rec.709 conversion"; they are made for the Rec.709 display colour space.
- **Skin Tone Indicator ("Balance")**
  - Hues inside a selected range around the skin-tone line turn yellow. Hues drifting off it turn green or magenta.
  - It checks hue, not brightness, because "melanin mainly changes luminance, not chroma", so it works for all skin tones.
- **Exposure Heatmap:** bands of Rec.709 display luminance, warm for over and cool for under. The page says they "are not true stop spacing around 18 % grey".
- **Saturation Heatmap:** cool for low saturation, warm for high.
- **Highlight Neutrals:** areas where R, G and B are almost equal turn green.
- **Clipping:** white and black thresholds, plus an "Enhanced" per-channel mode.
- **Palette:** "muted colors with even luminance instead of bright neon", shared by all the heatmaps.

## Other skin tone tools

- **Resolve's vectorscope:** the "Show Skin Tone Indicator" option draws a static line at the traditional in-phase (I) angle.
- **Nobe OmniScope:** a skin tone indicator scope and a skin overlay in its viewer.
- **ModernWndr "Skin Tone Indicator" DCTL:** HSV-based, colour-coding skin that falls outside a range.
- **Leo Labs "Hue / Skintone Highlighter" DCTL:** highlights a target hue range.
- **Common pattern** (*inferred*): either a static line on the vectorscope, or an overlay that classifies hue as on, warm-side or cool-side of a target, almost always on the display-referred picture.

## Resolve's built-in false colour

- **Form:** a ResolveFX plugin, "False Color", Studio only (Resolve 18.5 Reference Manual, ResolveFX Color). It is not a viewer toggle.
- **Modes:**
  - **Specific Camera Model:** Blackmagic camera, dynamic range and ISO.
  - **Creative:** colour band presets.
- **Legend:** can label black clip, white clip and middle grey.
- **Blackmagic cameras' false colour:** "pink represents optimum exposure for lighter skin tones, while green is a good match to darker skin tones … yellow to red … over exposed". Exact IRE edges were not found.
