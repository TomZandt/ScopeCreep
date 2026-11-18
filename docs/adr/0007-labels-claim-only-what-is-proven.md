# Labels claim "exact" only for what has been proven

Every label in scopecreep says what is measured and how far it can be trusted. "Output Conversion exact" is only shown for a Colour Setup whose conversion has been measured against Resolve's own renders (#17) and whose every relevant setting is known.

## Decision

- A Colour Setup read from the project is labelled "exact" when it matches a measured conversion and tone mapping and gamut mapping are known to be off.
- A **manual Colour Setup** on the Sender node is never labelled "exact". The node can't express tone mapping or gamut mapping, so the label reads "Output Conversion approximated: assumes tone and gamut mapping off" (code review of Milestone 1, commit 53e2c9d).
- When the Feed is shown as sent, with no Output Conversion, the waveform label says so too ("measured as sent, unconverted"), not only the Picture View label.

## Consequences

- Users of the free version of Resolve, who must use the manual Colour Setup, never see "exact", even when their project has no tone or gamut mapping. Adding those two settings to the Sender node would let a manual setup be exact; until then the label errs on the side of honesty.
- Any new label must be checked against this rule: claim the weaker statement when something is assumed.

## Update 2026-11-18 (ticket #99): the claim is split across the top bar and the title bar

The long label every Panel carried is now said in two places, and the claim is made by both together:

- The **top bar** says the Colour Setup once: Timeline → Output, Levels ("Video levels assumed" when the Colour Setup does not say), where it came from, and "Output Conversion exact", "exact for ACES 2.0", "approximated: why" or "as sent, no Output Conversion", in the same words as before. It says nothing stronger than the weakest of those.
- Each Panel's **title bar** adds only the state that changes how to read that Panel: "approximated" when its picture's conversion is, "unconverted" when it is shown as sent, "Rec.709 assumed" when its measured space is an assumption, and non-default settings that change what is measured. A Panel never drops a tag its measurement needs because the top bar says something similar; when tags do not fit beside the name the most important go first (reading state, then approximated, assumed or unconverted, then settings) and the hover shows all of them.
- The **hover** over a title bar shows the full label as it read before, so nothing is lost and nothing is overstated.

A Panel's title bar can never make a claim the top bar does not, and the top bar's claim is never stronger than what the least-proven Panel measures.
