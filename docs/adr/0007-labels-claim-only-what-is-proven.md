# Labels claim "exact" only for what has been proven

Every label in scopecreep says what is measured and how far it can be trusted. "Output Conversion exact" is only shown for a Colour Setup whose conversion has been measured against Resolve's own renders (#17) and whose every relevant setting is known.

## Decision

- A Colour Setup read from the project is labelled "exact" when it matches a measured conversion and tone mapping and gamut mapping are known to be off.
- A **manual Colour Setup** on the Sender node is never labelled "exact". The node can't express tone mapping or gamut mapping, so the label reads "Output Conversion approximated: assumes tone and gamut mapping off" (code review of Milestone 1, commit 53e2c9d).
- When the Feed is shown as sent, with no Output Conversion, the waveform label says so too ("measured as sent, unconverted"), not only the Picture View label.

## Consequences

- Users of the free version of Resolve, who must use the manual Colour Setup, never see "exact", even when their project has no tone or gamut mapping. Adding those two settings to the Sender node would let a manual setup be exact; until then the label errs on the side of honesty.
- Any new label must be checked against this rule: claim the weaker statement when something is assumed.
