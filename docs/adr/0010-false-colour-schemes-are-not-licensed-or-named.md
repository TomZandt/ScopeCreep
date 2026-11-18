# Third-party false colour schemes are modelled, not licensed, and never claimed

Colourists know two exposure schemes by name: ARRI's false colour and Ed Lachman's EL Zone System. scopecreep offers views modelled on both without licensing either. Its labels never claim to be the original.

## Decision

- **ARRI-style bands.** The view uses the Rec.709 percentage bands third parties publish as "ARRI" (SmallHD, Sigma): purple 0–2.5 %, blue 2.5–4 %, green 38–42 %, pink 52–56 %, yellow 97–99 %, red 99–100 %, everything else in grey. They are applied to the Luma of the Output picture (ADR 0009). The label says where the bands come from and that scopecreep is not affiliated with ARRI. It never calls them ARRI's specification.
- **Stops false colour.** A stop-based scheme modelled on the EL Zone System's look, shipped under scopecreep's own name. The name "EL Zone" never appears in the app.

## Why

- ARRI's own published specification (*ARRI LogC False Color Exposure Zones and Key Specification*, 2025) is defined on the LogC signal, with bounds that move with the camera's EI. On a graded, delivered Rec.709 picture, the only scheme colourists recognise is the percentage table, which ARRI does not publish as text. Claiming it is ARRI's would break ADR 0007.
- EL Zone is a trademark of Cinecam, Inc., licensed case by case, with no public terms and no published colour table. A licence would only buy the name, so the product owner chose to model the scheme and leave the name out.

## Considered and rejected

- **Implementing ARRI's LogC specification:** it needs ARRI footage and an EI, and it does not describe the delivered picture.
- **Licensing EL Zone before shipping:** this would block the view on a negotiation with no known terms.

## Consequences

- The EL Zone name is a clear trademark. A near-identical colour scheme could still draw a complaint from Cinecam. The product owner accepted that risk. This is not legal advice.
- If the bands are ever measured against an ARRI camera's own Rec.709 output, the label can say more.
- The research behind this decision is in [`docs/research/false-colour-and-skin-tools.md`](../research/false-colour-and-skin-tools.md).

## Update 2026-11-18 (#93)

The stops scale now has 17 zones centred on the stop marks (-6 to +6, with -1/2, 18 % and +1/2 beside middle grey, black and white beyond 6 1/2 stops) and a horizontal legend, following a reference scale the product owner supplied as a model for the colours and layout only. The colours are scopecreep's own values, chosen by eye (black, purple, lilac, blue, sky blue, green, mid green, light green, grey, yellow, pale yellow, orange, light orange, red, pink, light pink, white). The product owner accepted the risk that a scheme this close to a third party's can draw a complaint; the third party's name still appears nowhere in the app or its docs.
