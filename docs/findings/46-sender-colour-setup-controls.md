# #46 Greying out the Sender's manual Colour Setup controls, and why they don't show the project's values

What ticket #46 learned by testing DaVinci Resolve 21.2 Studio on Linux with the Sender (probe-alias build) on the timeline node. The experiments used a scratch project imported from `scopecreep-test-dwg-p3d65.drp` (three Sender instances). Every Resolve step ran under `flock -o ~/scopecreep-testdata/resolve.lock`. An experimental build, never committed, logged every OFX action Resolve called and set the disabled controls in `kOfxActionCreateInstance` from a scratch file.

## What shipped

- With **Set Colour Setup manually** off, Timeline Colour Space, Output Colour Space, Levels and ACES version are greyed out (`kOfxParamPropEnabled` = 0). The Sender sets this when an instance is created (so a loaded project opens greyed) and in `kOfxActionInstanceChanged` when the switch changes. It never touches the render call.
- In automatic mode scopecreep uses the project's Colour Setup, read by the app (ADR 0006), as before.
- Values set by hand stay: switching off keeps them (greyed), switching on brings them back, and nothing in the Sender resets them when a project is opened. Checked with the fake OFX host (`make -C sender test`).
- The product owner looked at the greyed-out controls in Resolve and said they look right. No agent screenshot of the Settings panel was taken: opening it needs clicks (Color page, node editor set to Timeline, Sender node selected, Settings tab), and Resolve ignores agents' clicks.

## Showing the project's values in the greyed controls: dropped

The product owner dropped this from the ticket. What testing found before that:

- **Where a plugin may set values.** OFX allows `paramSetValue` only outside render: create instance, instance changed (and its begin/end) and sync private data. Over a project load, idle, a render and a project-setting change by script, Resolve called the Sender only with `CreateInstance`, `GetClipPreferences` (from the render thread), `Begin/EndSequenceRender`, `Render` and `DestroyInstance`. `BeginInstanceEdit` was never called. The panel was not open; opening it needs clicks.
- **Setting values at create instance works.** Resolve accepted `paramSetValue` in `kOfxActionCreateInstance` (status OK) and at once called `BeginInstanceChanged`, `InstanceChanged` (reason `OfxChangePluginEdited`) and `EndInstanceChanged` on the same thread.
- **No action follows a project setting.** Changing `colorSpaceOutput` twice by script (no app attached, so the Sender was an identity) caused no OFX action at all. The only point where the Sender could fill in the values is project load, before the app has read the newly loaded project. So the values would show the previous project's setup, or be stale after any change in Project Settings.
- **Undo.** Ctrl+Z by keystroke reliably undid a CDL change made by script (Gain red 1.50 back to 1.00, screenshots `08`/`09`). A second Ctrl+Z called nothing on the Sender, so the values set at create instance were not an undo step, or at least not the next one.
- **Edited state and saving.** This Resolve has Project Live Save on (`IsProjectLiveSaveEnabled` in `config.user.xml`), so there is no unsaved state to see. Quitting with Ctrl+Q gave no save prompt, with or without values set by the plugin. Values set by the plugin were **sometimes saved into the project**. In the first session they survived a restart (two instances read back Output = P3-D65 at the next create). In later sessions, with nothing else edited, they were not saved, even after a CDL edit and its undo. So plugin-set values can end up in the saved project, depending on what else Resolve saves.
- **Keyframes.** The controls do not animate (`kOfxParamPropAnimates` = 0), so setting them cannot make keyframes. Not looked at in the keyframe editor (that needs clicks).
- **Render timing.** Not measured. No reflection code ever ran in the render call, and the shipped change does not touch it.
- **Saved values survive a reopen.** The values the Sender's controls hold were saved with the project and given back at `CreateInstance` after Resolve restarted (seen in the first session above). A value set by a person, then the project closed and reopened, was not checked in Resolve: scripts cannot set OFX parameters and agents' clicks don't register.

Logs and screenshots (not in git): `~/scopecreep-testdata/scratch-46/actions.log` (last session only), `~/scopecreep-testdata/screenshots/46/`.
