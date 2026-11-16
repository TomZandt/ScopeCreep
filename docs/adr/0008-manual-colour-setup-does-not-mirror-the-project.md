# The Sender's manual Colour Setup controls don't mirror the project

The Sender node has a manual Colour Setup (Timeline Colour Space, Output Colour Space, Levels, ACES version) behind a "Set Colour Setup manually" switch. Ticket #46 asked for these controls to show the project's own values while the switch is off, so the node never displays settings that aren't in effect.

## Decision

With the switch off, the controls are greyed out (`kOfxParamPropEnabled`) and are **not** filled in with the project's values; scopecreep simply uses the project's Colour Setup, read by the app (ADR 0006). Values a person sets by hand stay: switching off keeps them, switching on brings them back, and they are saved with the project. The product owner made this call on 2026-11-16 after testing showed the mirroring could not be done safely.

## Why

- OFX lets a plugin set parameter values only outside its render call, and Resolve calls the Sender outside render only when an instance is created. No action follows a change in Project Settings, so the values could only be set at project load, before the app has read the new project: they would show the previous project's setup, or go stale.
- Values the plugin set were sometimes saved into the project, depending on what else Resolve saved, so mirroring could quietly change users' projects.

Evidence: `docs/findings/46-sender-colour-setup-controls.md`.

## Consequences

- In automatic mode the node shows greyed-out controls whose values may differ from the project's. The app's label is the place that states the Colour Setup in use and where it came from.
- Revisit only if Resolve adds a way for a plugin to learn about project-setting changes outside render.
