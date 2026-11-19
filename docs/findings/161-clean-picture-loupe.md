# #161: Clean picture Loupe toggle

What ticket #161 showed on 2026-11-19, at the Feed seam (`app/tests/clean_loupe_seam.rs`, headless GPU), the pure toolbar seam (`app/tests/toolbar.rs`) and the settings seam (`app/tests/settings.rs`). Tests only; nothing was checked in Resolve.

- With the Loupe on it magnifies colour and each of Y, R, G and B exactly as the Panel shows them, in exact whole-pixel blocks (200 % is 2 x 2, 400 % is 4 x 4, 100 % is 1 x 1), zooms 100 to 800 %, drags, stays inside the Panel and reports what it drew.
- With it off nothing is drawn and no pointer, drag or wheel event is used.
- The Loupe chip is off by default, toggles, is remembered per Panel and tags "Loupe N %" only while on; the size is the popover's only row. Rubbish saved values fall back to off and medium.
- Not checked: the look beside Resolve 21 (a person does that, #75 moves to the clean picture).
- The Loupe chip is a compact "L" (tooltip "Loupe") so the five chips and the settings button fit a Panel about 190 px wide. At the 120 px minimum Panel width they do not all fit (the same is already true of four chips plus the button); the Panel grid's default layout gives the clean picture far more.
