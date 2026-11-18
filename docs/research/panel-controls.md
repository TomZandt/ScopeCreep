# Panel controls: what colourists change, what they set once, what is noise

Research for spec #113 (style system and Panel toolbar), with #94 and #83 as context. It covers the displays scopecreep ships today, with no new displays.

**Placement key**
- **Toolbar**: a chip or toggle on the Panel's title bar, for things changed mid-session.
- **Popover**: the settings popover (from the gear button or a right-click), for things set once per room, job or taste.
- **Drop**: don't offer it.

**How sure the placements are.** No source says outright "colourists change X mid-session". The placements below are inferred from two kinds of evidence:
1. Where the vendors put each control. Tektronix states its three levels of control explicitly. Resolve keeps every per-scope option in one popover. Leader puts each option behind its own soft key.
2. What the option is for. A diagnostic you use per shot is different from a preference you set for the room.

Each row says which of the two it rests on.

**Tektronix's three levels, an existing toolbar/popover/settings split.** The WFM5200 manual (p. 13) splits controls into three levels:
- **Frequently changed settings** sit on the front-panel buttons and knobs.
- **Tile-specific settings** are in a pop-up menu, held down for about a second, "for less frequently changed parameters such as the waveform display mode".
- **Instrument-wide settings** are in the CONFIG menu, "changed only occasionally, such as changing waveform colour".

scopecreep's toolbar, popover and (future) app-wide settings map onto these one to one. Think of it as a car: the steering wheel and indicators (toolbar), the seat and mirror adjustments (popover), and the settings you set once at the dealer (global).

## Sources

| Id | Source |
|---|---|
| R20 | Blackmagic, *DaVinci Resolve 20 Reference Manual*, ch. 127 "Viewers, Monitoring, and Video Scopes", pp. 2985–2986 (Viewer options) and pp. 3007–3022 (Video Scopes). https://documents.blackmagicdesign.com/UserManuals/DaVinci_Resolve_20_Reference_Manual.pdf |
| R21 | Blackmagic, *DaVinci Resolve 21 New Features Guide*, p. 94 "Customizable HDR and Nit Scopes Ranges". https://documents.blackmagicdesign.com/SupportNotes/DaVinci_Resolve_21_New_Features_Guide.pdf (the 20.1 and 20.2 guides have no scope changes) |
| TEK | Tektronix, *WFM5200 Waveform Monitor User Manual* (077-0531-03): "Three Levels of Control" p. 13; Waveform display p. 37; Vector display pp. 38–40. https://www.avbroadcast.fr/media/productfile/t/e/tektronix-wfm5200-manuel.pdf |
| LV | Leader, *LV5600/LV7600 Waveform Monitor Instruction Manual*: §10.3.5–10.3.11 (waveform intensity, colour, scale intensity, scale unit, 75 % scale, scale display), §10.4 (gain, filter), §11.2–11.11 (vector), §13.2.1 (picture mono/colour), §13.8.1 (picture FIT/REAL/X2). https://www.leader.co.jp/uploads/2021/06/lv5600_lv7600_e_11_57.pdf |
| SB-W / SB-V | Hedge, *ScopeBox* docs, Waveform and Vectorscope palettes. https://docs.hedge.video/scopebox/palette-types/waveform and …/vectorscope |
| SHD-W / SHD-V / SHD-FC | SmallHD User Guide: Waveform https://guide.smallhd.com/a/808524-waveform, Vectorscope https://guide.smallhd.com/a/814239-vectorscope, Exposure Assist (False Color) https://guide.smallhd.com/a/808517-exposure-assist-false-color |
| ARRI | ARRI, *LogC False Color: Exposure Zones and Key*, specification, 2025-02-04, §2.2 and Table 1. https://www.arri.com/resource/blob/390448/2d469ae1e98110562fe347cea50a284b/arri-logc-false-color-specification-data.pdf |
| MN | MonoNodes, Utility DCTL product page. https://mononodes.com/utility-dctl/ |
| POM | Pomfort, "How to use Livegrade's False Color mode". https://pomfort.com/article/how-to-use-livegrades-false-color-mode/ |
| OMNI | Toolfarm product page for Nobe OmniScope (vendor text: 13 preset false colour schemes, editable ranges). https://toolfarm.com/buy/timeinpixels_nobe_omniscope |
| MON | Hardware monitor manuals, seen as search snippets only, not read in full (weak): the JVC DT-G17E and ProHD manuals cycle an "R/G/B/Mono" function key; Sony describes "Blue Only" as showing the blue signal as monochrome; the FSI AM250 manual describes a positionable, resizable "Pixel Zoom" rectangle (manualshive.com). RED's Tools page says its false colour shows "most of the tonal range … in monochrome" (docs.red.com). |
| FORUM | Supporting evidence only. Blackmagic Forum "Vectorscope Brightness" (brightness is relative to the whole image, so small areas read faint) https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=189551 ; Blackmagic Forum "Waveform Monitor Scales" (graders split between 10 % and 12.5 % divisions) https://forum.blackmagicdesign.com/viewtopic.php?f=33&t=162413 ; Lift Gamma Gain "Ability to view color gamut violation in vectorscope" (the vectorscope is the wrong tool for gamut; use the parade) https://www.liftgammagain.com/forum/index.php?threads/ability-to-view-color-gamut-violation-in-vectoscrope.9647/ ; Mixing Light "HDR Essentials: Reading the Scopes in DaVinci Resolve" (nit scale) https://mixinglight.com/color-grading-tutorials/hdr-essentials-reading-the-scopes-in-davinci-resolve/ |
| REPO | scopecreep `GLOSSARY.md`, issues #113, #94, #83, `app/src/*.rs` settings (current defaults). |

I couldn't get these: Omniscope's own docs (timeinpixels.com returned 404), a full Telestream PRISM manual (behind Telestream's resources page), and Phabrix manuals. PRISM shows up only through Telestream marketing text: Luma Qualified CIE, a Stop display in nits or stops, and HDR false colour above 203 nits.

---

## 1. Clean picture

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| B/W (show the picture as Luma grey) | **Toolbar** toggle | Off (colour) | Judging contrast without colour is a per-shot check. Leader puts MONO/COLOR on the picture display's own menu, beside brightness and contrast. Once this toggle exists, the separate Black and white Panel adds nothing (the open question in #113). | LV §13.2.1; REPO #113 |
| Single channel (R, G, B) | Drop here | – | It belongs on the Channel view, which has the Loupe for it. Resolve does offer it on its main Viewer ("Show Viewer Channels: RGB, R, G, B"), so if the Channel view is ever merged into the clean picture, this becomes a toolbar chip. | R20 p. 2986 |
| Picture zoom (FIT / 1:1 / 2×) | Drop for now | Fit | Leader offers FIT/REAL/X2 on the picture display. scopecreep's Loupe on the Channel view does this job, and Resolve's Viewer already zooms. | LV §13.8.1 |
| Safe-area / aspect markers | Drop | – | Every hardware scope offers them (Leader §13.4, TEK p. 83), but they are framing tools, not grading tools, and Resolve's Viewer draws them. A colourist won't miss them on a second monitor. | LV; TEK |
| Brightness / contrast of the picture | Drop | – | Leader has them (§13.2.3–4), but on a grading monitor they would mislead. ADR 0013 says a display can't be set up to mislead. | LV; REPO ADR 0013 |

## 2. Luma waveform

What real scopes offer:
- **Resolve's Waveform:** Y / CbCr / RGB modes; in RGB, R, G and B buttons "individually toggled on and off to see any combination of graphs"; Colorize; Extents; a Waveform (trace) brightness slider; a Graticule brightness slider; Show Reference Levels with Low and High sliders (R20 pp. 3019–3020).
- **Resolve, global:** Low Pass Filter; Quality (Auto/High/Medium/Low); Waveform Scale Style (10-bit default, 12-bit, Percentage, mV, HDR ST.2084/HLG); Video Level Scopes (R20 pp. 3009–3012). Resolve 21 adds a Scale Display Range to zoom the HDR scale (R21 p. 94).
- **Leader:** waveform intensity −128…127 (default 0); waveform colour; scale intensity −8…7 (default 4); scale colour; scale unit (V, %, CV DEC 64–940, CV HEX); scale on/off; variable gain and ×1/×5/×10; filter FLAT/LOWPASS (LV §10.3–10.4).
- **ScopeBox:** Weighted / Mono / Colorize; Zoom Blacks; Luma/Chroma; instantaneous and peak envelopes; Scale IRE / 8-bit / 10-bit / mV / ST2084 (SB-W).

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Y R G B channel chips** (any combination, at least one on) | **Toolbar** | Y only | This is Resolve's own model: in RGB mode each of R, G and B toggles on and off independently. Checking channel balance and per-channel clipping is a per-shot task. It also covers the parade's main use without a side-by-side layout. | R20 p. 3020; REPO #113 |
| Colour trace (Colorize) | **Drop on this Panel** | – | Resolve's Colorize on Y "draws areas with colour taken from the image". scopecreep already has that as its own Colour Waveform Panel (ADR 0011, one display per Panel). A toggle here would duplicate it. If the two Panels are ever merged, it becomes a toolbar toggle (default off, white trace). | R20 p. 3020; REPO GLOSSARY "Colour Waveform" |
| Trace brightness | **Popover** slider (#83) | Today's "Normal" | Set to suit the room and the screen. Resolve keeps it in the per-scope options popover, Leader behind a menu with a reset-to-default, and Tektronix keeps display intensity under the Display button, not on the main controls. | R20 p. 3020; LV §10.3.5; TEK p. 13 |
| Graticule (scale) brightness | **Popover** slider | Visible but quiet: one step below the label text, as Leader's default of 4 on a −8…7 range suggests | Every vendor has it as its own control. Resolve says it makes the scale "more or less visible (or distracting) relative to the graph". It is set for the room, not per shot. It is missing from scopecreep today, and #83 doesn't list it, so it is a gap. | R20 p. 3020; LV §10.3.7 |
| Scale units | **Popover** choice: **10-bit codes / %**, plus **nits** when the Output is PQ or HLG | 10-bit (what Resolve and scopecreep show today) | Resolve made 10-bit the default and moved the unit to a simple submenu. Graders do differ (Leader offers CV, % and V; forum posters disagree on 10 % or 12.5 % divisions), so it is a once-per-person choice. Nits is the HDR standard. Drop 12-bit, mV, hex and IRE: these are composite or broadcast legacy units, and IRE is just % under another name. The unit changes what the numbers mean, so it shows as a tag. | R20 p. 3009; LV §10.3.9; SB-W; FORUM (scales, Mixing Light) |
| Soft spread | **Popover** slider (#83) | 1 px | Drawing only. It is scopecreep's answer to the readability that the low-pass filter buys elsewhere. | REPO #83 |
| Low-pass filter | **Drop** | – | Resolve warns that it "can make the highlight and shadow excursions … seem a bit shy of where they actually are". That is a misleading reading, which ADR 0013 rules out. Soft spread already gives the smoother look without moving the peaks. | R20 p. 3012; LV §10.4.3; TEK p. 37 |
| Extents (outline of the true max and min) | **Popover** toggle, maybe later | Off | Offered by Resolve and ScopeBox. scopecreep's edge rows already show out-of-range pixels, so extents add little unless low-pass is ever added. | R20 p. 3012; SB-W |
| Reference levels (adjustable low and high lines) | **Popover**, later | Off | Resolve offers them for HDR peak limits ("working within a specific peak luminance threshold"). They are set once per delivery spec. | R20 p. 3019 |
| Luma weights (primaries / match Resolve) | **Popover** (as today) | From primaries | Set once. It changes the measurement, so it shows as a tag. | REPO ADR 0004 |
| Gain / zoom blacks / variable gain | Drop for now | – | Leader ×5/×10 and ScopeBox Zoom Blacks are real, but are used for engineering QC and noise-floor checks. A colourist can resize the Panel instead. A candidate for later, not v1. | LV §10.4.1; SB-W |
| Quality / trace colour choice (white, green, …) / CbCr mode / video-level scaling | Drop | – | Quality exists because of Resolve's GPU budget, and scopecreep has one fixed quality (ADR 0003). Trace colour is an instrument-wide preference (Tektronix CONFIG) that a tool with one style system doesn't need. CbCr is an engineering mode. Levels come from the Colour Setup, not a scope toggle. | R20 pp. 3010–3011; TEK p. 13 |

## 3. Colour Waveform

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| Trace brightness | **Popover** slider | Today's Normal | Same as the luma waveform. | R20 p. 3020 |
| Soft spread | **Popover** slider | 1 px | Same as the luma waveform. | REPO #83 |
| Graticule brightness | **Popover** (shared value, see conventions) | as luma | Same as the luma waveform. | R20 |
| Scale units | Follows the luma waveform's choice | 10-bit | The two waveforms side by side must read on the same scale. | inference |
| Toolbar | **None** | – | The Panel *is* the colour trace. An empty toolbar is fine and keeps the title bar quiet. | REPO #113 story 20 |

## 4. Vectorscope (one Panel, 100 % and 200 % plots)

What real scopes offer:
- **Resolve:** ALL / Low / Mid / High tonal range, with adjustable Low (0.30) and High (0.70) boundaries; Colorize ("with colorize turned off, graphs only appear white"); Extents and Combine; a Vectorscope (trace) brightness slider; a Graticule slider; Show 2X Zoom; Show Skin Tone Indicator; Show Graticule. Globally, a Vectorscope Scale Style (Off / Standard / Simplified / Hue Vectors), targets at 75 %, 100 % or both, and hue labels on or off (R20 pp. 3010, 3020–3021).
- **Leader:** vector intensity −128…127; vector colour; scale intensity and colour; scale BT.601/709/DCI/2020/Auto; gain ×1/×5/IQ-MAG plus variable; a 75 %/100 % bar scale; a measuring marker giving d % and degrees; I/Q axis (LV §11).
- **Tektronix:** gain 1X/2X/5X/10X/variable; 75 %/100 % targets in the tile's pop-up; I/Q axis or compass rose in CONFIG; Luma Qualified Vector with luma high and low (TEK pp. 38–40).
- **ScopeBox:** Weighted / Mono / Colorize; envelopes; Flesh Line; Standard or Hue Vectors graticule; targets (SB-V).
- **SmallHD:** Show Skintone (SHD-V).

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Colour / mono trace** | **Toolbar** toggle | Colour (today's tinted trace) | Resolve calls Colorize a way to see "which part of the scope graph corresponds to which part of the image". That is a per-shot diagnostic: which spike is the wall, which is the jacket. Mono (white) is the classic phosphor look and is easier to read for density and for neutrals around the centre. Unlike the waveforms, there is no second vectorscope Panel to carry the other look, so the toggle has to live here. | R20 p. 3020; SB-V |
| **Plots shown: 100 % · 200 % · both** | **Toolbar** segmented chip | Both | Every vendor treats zoom as a front-panel-level control (Tektronix GAIN and MAG buttons, Leader GAIN MAG, Resolve Show 2X Zoom). The 2× plot matters for low-saturation and neutral work, the 1× plot for the whole picture. On a small Panel, one plot at double the size beats two cramped ones. The choice is shown as a tag ("200 %"). | TEK p. 39; LV §11.6.2; R20 p. 3021 |
| Tonal range: All / Shadows / Mids / Highlights (luma-qualified) | **Toolbar** chip, a candidate for v2 | All | This is the one option colourists would miss. Resolve (Low/Mid/High plus Combine) and Tektronix (LQV) both offer it, and it is the usual way to check whether a cast is in the shadows or the highlights. It changes what is measured, so it shows as a tag. It needs new measurement code, so file it as an issue rather than part of #113. | R20 p. 3020; TEK p. 40 |
| Skin-tone line | **Popover** toggle | On | Set once. Resolve, ScopeBox, SmallHD and Leader/Tektronix (as the I/Q axis) all put it in settings or CONFIG, not on the front panel. | R20 p. 3021; SB-V; SHD-V; TEK p. 40 |
| Colour targets 75 % / 100 % | **Popover** choice | 75 % | Set once per job or bars standard. Resolve's default graticule shows 75 % targets. Tektronix keeps it in the tile's pop-up. Resolve's "both" option is a nice-to-have. | R20 p. 3015; TEK p. 39; LV §11.11.2 |
| Graticule style (standard / hue vectors / off) | **Popover**, maybe later | Standard | Resolve and ScopeBox offer the Van Hurkman hue-vector graticule. It is a matter of taste and set once. | R20 p. 3010; SB-V |
| Trace brightness | **Popover** slider | Today's Normal | Same reasoning as the waveform. The Blackmagic forum shows users hunting for it when small areas read faint. | R20 p. 3021; FORUM |
| Graticule brightness | **Popover** slider | quiet | Same as the waveform. scopecreep has nothing for this yet. | R20 p. 3021; LV §11.3.1 |
| Soft spread | **Popover** slider | 1 px | #83. | REPO |
| Hue labels on targets | Drop or popover | Off | Resolve offers it. scopecreep's hue-wheel ring already says which hue is where. | R20 p. 3010 |
| Extents / peak envelopes | Drop for now | – | They show a frame's maximum chroma. scopecreep's ring-clip mark already flags colours off the chart. | R20; SB-V |
| Variable gain, IQ-MAG, measuring marker, scale standard (601/2020/DCI) | Drop | – | Engineering QC. Which standard applies comes from the Measured Space automatically (Leader's own default is AUTO). | LV §11.3.5, §11.6.2, §11.9 |

## 5. Channel view with Loupe

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Y R G B channel chips**, one at a time | **Toolbar** | B (today's default; blue is the noisiest channel) | Resolve: "Show Viewer Channels … RGB, R, G, or B … useful when evaluating a single channel for noise or other artifacts". Monitors put R/G/B/Mono and Blue Only on a function key, so it is a one-press, mid-session action. | R20 p. 2986; MON; REPO #113 |
| Loupe zoom (100 / 200 / 400 %) | **On the Loupe itself** (scroll wheel or a small badge), not the title bar | 200 % | Zoom is direct manipulation of the Loupe. FSI's Pixel Zoom is a positionable, resizable rectangle on a function key, and Leader's picture X2 is its own soft key. It is remembered as display state (ADR 0015). | MON (FSI); LV §13.8.1 |
| Loupe size (small / medium / large) | **Popover** | Medium | Set once for the Panel size. | REPO channel_view.rs |
| Loupe on/off | **Toolbar** toggle, optional | On | Lets the colourist see the whole channel unobstructed. Cheap to add. | inference |

## 6. Black and white

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| Luma weights | Follows the luma waveform (no own control) | – | The glossary says its greys equal the waveform's Luma. | REPO GLOSSARY |
| Whole Panel | **Candidate to retire** | – | Hardware treats B/W as a MONO/COLOR toggle on the picture display, not as a separate display (Leader §13.2.1). With the B/W toggle on the clean picture (#113), this Panel duplicates it. | LV §13.2.1; REPO #113 open question |

## 7. ARRI-style False Colour

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Picture under the bands: B/W / colour** | **Toolbar** toggle | **B/W** | ARRI's spec defines the output outside every zone as the achromatic signal (A′, A′, A′), so grey is the canonical look. RED's false colour also shows most of the range in monochrome. Colour underneath is a scopecreep convenience: useful for locating things, but not the reference look. | ARRI §2.2; MON (RED); REPO #113 |
| Legend | **Popover** toggle | On | Every implementation offers a key and treats it as set-once: SmallHD "Show IRE Guide" with a guide location, scopecreep today. | SHD-FC; REPO |
| Band scheme editing / presets | Drop | – | ARRI's colours are "hard-coded" and "purely categorical". Editable schemes (OmniScope, SmallHD MAP, Pomfort's generator) are a different product, and ADR 0010 already decides naming and licensing. | ARRI Table 1; OMNI; SHD-FC; REPO ADR 0010 |
| Overlay intensity / opacity | Drop | – | SmallHD has "Intensity". A half-strength band gets mistaken for another band, which misleads. | SHD-FC |

## 8. Stops False Colour

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Mode: Full zones / Grey and +1 stop** | **Toolbar** segmented chip | Full zones | The chart mode is switched on when a ColorChecker shot comes up and off again: per shot, not per job. It changes what is shown, so it is tagged. | REPO GLOSSARY |
| Picture under: B/W / colour | **Toolbar** toggle, **only in Grey and +1 stop mode** | B/W | In Full zones every pixel gets a zone colour, so the toggle would do nothing and should be hidden or disabled. In chart mode, B/W is the defined look (glossary). | REPO GLOSSARY |
| Legend (horizontal scale, #93) | **Popover** toggle | On | Set once. | REPO #94/#93 |
| Zone count / custom stops / EI | Drop | – | Stops come from the decoded Timeline Colour Space (ADR 0019), so there is no EI to set. ARRI's EI-dependent zones are a camera concern. | ARRI §3.2; REPO ADR 0019 |

## 9. Skin Tone

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| **Picture under: colour / B/W** | **Toolbar** toggle | Colour (per #94's 2026-11-18 update, modelled on MonoNodes) | MonoNodes' own pack offers both looks (its feature list includes "Gray Out Non-Skin Areas" in the product description). Switching to B/W makes skin pop when the scene is busy, which is a per-shot need. | MN; REPO #94 |
| Tolerance (± degrees) | **Popover** slider (#83) | ±5° | Set once to taste. MonoNodes exposes no tolerance at all, which is a sign it isn't adjusted often. | MN; REPO #83 |
| Line angle | Drop | 123° (shared constant, ADR 0018) | An adjustable angle would turn it into a different tool for each user. | REPO ADR 0018 |

## 10. Saturation heatmap

| Control | Place | Default | Why | Source |
|---|---|---|---|---|
| Scale (warm end, % of the vectorscope ring) | **Popover** slider (#83) | 100 % | Set once per look or job. It changes the reading, so it is tagged. | REPO #83, ADR 0023 |
| Legend | **Popover** toggle | On | Set once. | REPO |
| Picture under: B/W / colour | **Drop** (always B/W) | B/W | #94 already defines the heatmap as "Luma in grey with saturation overlaid". A colour picture under a saturation map lets the picture's own saturation read as heat, which misleads. MonoNodes' saturation heatmap exposes no such option. This contradicts #113's proposal to put a B/W toggle on every overlay Panel, so the product owner should confirm. | REPO #94; MN |

## Hidden Panels (brief)

- **Gamut/legal warning:** blue and red thresholds as popover sliders (#83). It has no toolbar controls.
- **Highlight Neutrals:** tolerance as a popover slider. If it returns, a B/W-under toggle on the toolbar, as with Skin Tone (MonoNodes shows neutrals as green over the picture).
- **CIE plot:** in the popover, 1931 xy / 1976 u′v′, the additional gamut triangle, trace and graticule brightness, and 2× zoom (Resolve offers all of these, R20 p. 3022). A toolbar chip for the triangle only if it is switched per shot. Prism adds luma-qualified CIE.

---

## Cross-cutting conventions

1. **Three tiers, as Tektronix defines them.** The toolbar holds what is changed per shot. The popover holds what is set per room, job or taste. App-wide CONFIG holds only what is truly global. Nothing goes on the toolbar just because it exists. (TEK p. 13)
2. **Every trace display has the same popover trio:** trace brightness, graticule brightness and soft spread, as sliders with the same names, ranges and defaults. Resolve gives every scope exactly a "[scope] slider" plus a "Graticule slider". Graticule brightness is the gap: it is missing today, and #83 should list it. Optionally, graticule brightness could be one app-wide value, since it is about the room, not the Panel. (R20 pp. 3019–3022; LV)
3. **Colour or mono trace is a choice on every scope, but not always a toggle.** The vectorscope has a toolbar toggle (it has no sibling Panel). The waveforms get it by having two Panels: the Luma waveform is white and the Colour Waveform is coloured. Don't add Colorize to the luma waveform as well. (R20 p. 3020)
4. **One "picture under" toggle with one meaning,** labelled "B/W" with the same icon everywhere. It appears only where it changes something: the clean picture, ARRI False Colour, Stops False Colour in Grey and +1 stop mode, and Skin Tone. It is absent on the heatmap, where the picture is always grey, and in Stops Full zones, where every pixel is coloured. Defaults follow each display's reference look: ARRI and Stops grey, Skin Tone and the clean picture in colour. (ARRI §2.2; LV §13.2.1)
5. **Channel chips look the same everywhere.** Y R G B, coloured when on and grey when off. Single choice on the Channel view, any combination with at least one on for the waveform. This matches Resolve's own two uses. (R20 pp. 2986, 3020)
6. **Anything that changes the measurement is tagged:** plots shown, tonal range, scale unit, channels, Stops mode, tolerances, the heatmap scale. Drawing-only settings (brightness, spread, graticule, legend, B/W under) are never tagged. (REPO ADR 0007, #83)
7. **No control that can make a reading look better than it is:** no low-pass, no overlay intensity, no picture brightness or contrast, no adjustable skin angle. (R20 p. 3012; REPO ADR 0013)
8. **Units:** 10-bit codes by default, % as the alternative, nits only for PQ/HLG outputs. The legacy units (mV, IRE, hex, 12-bit) are left out. (R20 p. 3009)
9. **Legends are popover toggles, on by default,** on every false colour and heatmap Panel.
10. **Gaps worth filing as issues, not built in #113:**
    - a vectorscope tonal range (luma-qualified) chip;
    - waveform reference-level lines for HDR;
    - a 75 % + 100 % "both" targets option;
    - the hue-vectors graticule;
    - whether to retire the Black and white Panel.
