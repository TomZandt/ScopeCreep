# #160: Clean picture Y R G B channel chips

What ticket #160 showed on 2026-11-19, at the Feed seam (`app/tests/bw_seam.rs`, headless GPU), the pure toolbar seam (`app/tests/toolbar.rs`, `app/tests/components.rs`) and the settings seam (`app/tests/settings.rs`). Tests only; nothing was checked in Resolve.

- The clean picture draws colour by default; Y is Luma grey with the waveform's weights, R, G and B are that channel as grey; mattes stay black on every choice.
- The one-or-none chip mode switches between chips, turns off on a second click, is remembered per Panel and tags the channel only when it is not colour. The four chips stay on a small Panel.
- A saved clean picture B/W "on" reads as Y and "off" as colour; ARRI, Stops and Skin Tone B/W are unchanged.
- Not checked: the look beside Resolve 21 (a person does that, #75 moves to the clean picture).
