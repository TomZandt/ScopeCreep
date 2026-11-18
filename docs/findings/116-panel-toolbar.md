# #116: the Panel toolbar

What ticket #116 showed on 2026-11-18, at the pure UI seam (`app/tests/toolbar.rs`, no GPU).

- The Channel view has Y R G B chips on its title bar, blue on by default. A click switches the channel one at a time, Y is Luma, and the choice is remembered with the other settings.
- Chips are coloured when on and grey when off; hovering one names it. The channel shows in the tags.
- Settings the toolbar does not show are in the popover, opened from the settings button; the popover stays inside the Window.
- On a small Panel the chips stay, the name is cut short and the tags move to the hover.

Not verified (a person at the app): how the chips and popover look and feel at real sizes.
