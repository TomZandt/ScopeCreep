# The last Sender in the frame drives the Feed, and more than one active Sender is a warning

A project can hold more than one Sender: one left on a clip node, another on the timeline node, or two in step on a grade. Each frame Resolve renders then reaches several Senders, and the app needs one picture. Milestone 1 settled this with a rule colourists cannot see (the instance Resolve created first in its process, #19), and every Sender copied every frame, so two Senders in step could drop frames as "Sender busy" (#38).

## Decision

- When several Senders render the same frame, the one that renders **last in the frame** drives the Feed. In practice that is the last timeline node: the picture after every grade, which is what the colourist means by "the output".
- Only the driving Sender copies frames. The others still register and record their renders, so the driver can be handed over within one frame period when it stops rendering.
- Whenever more than one Sender is active, the app shows a warning naming them, so the colourist can remove the extra one.

## Considered and rejected

- **The first instance Resolve created** (milestone 1): invisible to the colourist and unrelated to the grade.
- **A "Drive the Feed" checkbox on the Sender:** explicit, but one more control to set on every project, and two ticked boxes still need a rule.

## Consequences

- The glossary and the README's placement guidance say "the last Sender in the frame drives".
- If telling "last in the frame" apart needs more in the Feed protocol's registry and that is a breaking change, the protocol and product versions bump (#38).
- Decided by the product owner on 2026-11-18 (#39); built by #38.
