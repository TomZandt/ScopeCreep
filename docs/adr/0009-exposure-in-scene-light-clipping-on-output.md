# Exposure in stops is measured in scene light; clipping and legal checks on the Output picture

A colourist asks two different exposure questions, and they need two different signals. Think of a song: the volume at the mixing desk versus the volume from the speakers after the limiter. The limiter squashes the loud parts, so "how far above normal" gives a different answer at each end.

- **How many stops from middle grey is this?** This is measured in **scene light**: the Feed's Timeline Colour Space frames, after the grade (the Sender sits at the end of the node tree), decoded to linear light with that space's own curve. Middle grey is 0.18 linear in every scene-referred space. Only the code value it is stored as differs (about 0.414 in ACEScct, about 0.336 in DaVinci Intermediate), so every Timeline Colour Space is decoded with its own curve. This is what camera monitors do for stop-based false colour: they decode the camera's log signal, never the Rec.709 picture on screen.
- **Is this clipped, crushed, illegal or out of gamut in what we deliver?** This is measured on the **Output picture**, the Feed after the Output Conversion: what the client sees.

## Decision

- The stops false colour view measures scene light decoded from the Timeline Colour Space. It ignores the Measured Space switch, because it always needs scene light.
- When the Timeline Colour Space is display-referred (such as Rec.709 Gamma 2.4), there is no scene light to decode. The view inverts the display curve and its label says it is approximated (ADR 0007).
- The gamut/legal warning and the ARRI-style IRE bands measure the Output picture, using the project's Levels.

## Considered and rejected

- **Measuring everything on the Output picture,** as display-referred tools such as MonoNodes' DCTLs do. Those tools say plainly that their bands are not true stops. The Output Colour Space's tone mapping squashes the highlights, so a stop scale measured there is not a stop scale.
- **Measuring everything in scene light.** Clipping and legal range only exist in the delivered signal.

## Consequences

- The stops view shows the exposure of the grade, not of the camera original.
- The stops view needs a decoding curve for every supported Timeline Colour Space. One the app cannot decode must be labelled, never guessed.
